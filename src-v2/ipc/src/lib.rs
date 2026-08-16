/// # IPC Bridge — WebSocket-based Code Streaming
///
/// Architecture:
/// ```
/// ┌─────────────┐     WebSocket      ┌──────────────┐    IPC channels    ┌──────────────┐
/// │  Tauri UI   │ ◄─────────────────►│  IPC Server  │ ◄─────────────────►│  Sandbox     │
/// │  (React)    │    ws://127.0.0.1  │  (Rust/tokio)│                    │  Executor    │
/// └─────────────┘     :9090          └──────────────┘                    └──────────────┘
/// ```
///
/// The IPC server runs on **port 9090** (separate from the Roblox-facing
/// port 8080). It accepts connections from the Tauri frontend, parses
/// structured JSON messages, and routes them to the sandbox for execution.

use std::sync::Arc;
use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_tungstenite::{accept_async, tungstenite::Message};

// ─── Protocol types ───────────────────────────────────────────────────────────

/// Messages the frontend sends TO the IPC server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FrontendCommand {
    /// Execute a Luau script.
    #[serde(rename = "execute")]
    Execute { script: String, id: String },
    /// Request the current sandbox output buffer.
    #[serde(rename = "poll_output")]
    PollOutput { id: String },
    /// Change sandbox configuration at runtime.
    #[serde(rename = "configure")]
    Configure {
        max_instructions: Option<u64>,
        max_memory_mb: Option<usize>,
        timeout_secs: Option<u64>,
    },
    /// Request the engine's current status/snapshot.
    #[serde(rename = "get_status")]
    GetStatus { id: String },
    /// Switch product mode (external / internal / dev).
    #[serde(rename = "set_mode")]
    SetMode { mode: String },
    /// List available hub scripts.
    #[serde(rename = "hub_list")]
    HubList { id: String },
    /// Run a hub script by ID.
    #[serde(rename = "run_hub_script")]
    RunHubScript { id: String },
    /// Run an arbitrary control script.
    #[serde(rename = "run_script")]
    RunScript { script: String, id: String },
}

/// Messages the IPC server sends TO the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    /// Execution result for a previously submitted script.
    #[serde(rename = "result")]
    ExecutionResult {
        id: String,
        success: bool,
        output: String,
        error: Option<String>,
        duration_ms: u64,
    },
    /// Console output lines from running scripts.
    #[serde(rename = "console")]
    ConsoleOutput { lines: Vec<String> },
    /// Server status / error.
    #[serde(rename = "status")]
    Status { message: String, level: String },
    /// Engine status snapshot for the UI.
    #[serde(rename = "status_snapshot")]
    StatusSnapshot { payload: serde_json::Value },
    /// Hub script list.
    #[serde(rename = "hub_list")]
    HubScriptList { scripts: serde_json::Value },
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Channel closed: {0}")]
    ChannelClosed(String),
}

// ─── IPC Server ───────────────────────────────────────────────────────────────

/// The IPC server accepts WebSocket connections from the frontend and routes
/// commands to the sandbox execution engine.
pub struct IpcServer {
    /// TCP listener bound to 127.0.0.1:9090.
    listener: TcpListener,
    /// Channel to notify the server to shut down.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl IpcServer {
    /// Bind the IPC server to the default loopback address and port.
    pub async fn bind() -> Result<Self, IpcError> {
        let addr: SocketAddr = "127.0.0.1:9090".parse().unwrap();
        let listener = TcpListener::bind(addr).await?;
        info!("IPC server listening on ws://{}", addr);
        Ok(Self {
            listener,
            shutdown_tx: None,
        })
    }

    /// Run the accept loop, handling each frontend connection on a new task.
    pub async fn run(&mut self) -> Result<(), IpcError> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        loop {
            tokio::select! {
                accept_result = self.listener.accept() => {
                    let (stream, peer) = accept_result?;
                    info!("Frontend connected from {}", peer);
                    tokio::spawn(handle_connection(stream));
                }
                _ = &mut shutdown_rx => {
                    info!("IPC server shutting down");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Signal the server to shut down gracefully.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Handle a single frontend WebSocket connection.
async fn handle_connection(stream: TcpStream) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Each command from the frontend is JSON-deserialized, executed via
    // the sandbox, and the result is sent back.
    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                debug!("IPC << {}", &text[..text.len().min(200)]);

                match serde_json::from_str::<FrontendCommand>(&text) {
                    Ok(command) => {
                        // Route command to sandbox
                        let response = handle_command(command).await;
                        let json = serde_json::to_string(&response)
                            .unwrap_or_else(|e| format!(r#"{{"type":"status","message":"Serialize error: {}","level":"error"}}"#, e));

                        if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                            error!("Failed to send response: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        let err_msg = ServerEvent::Status {
                            message: format!("Invalid command: {}", e),
                            level: "error".to_string(),
                        };
                        let json = serde_json::to_string(&err_msg).unwrap();
                        let _ = ws_sender.send(Message::Text(json.into())).await;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("Frontend disconnected");
                break;
            }
            Ok(Message::Ping(data)) => {
                if ws_sender.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("Frontend connection closed");
}

/// Route a single command to the sandbox and return the appropriate event.
async fn handle_command(cmd: FrontendCommand) -> ServerEvent {
    match cmd {
        FrontendCommand::Execute { script, id } => {
            // In the full implementation, we'd dispatch to the sandbox:
            //   let sandbox = SANDBOX.lock().unwrap();
            //   let result = sandbox.execute(&script);
            //
            // For now we return a placeholder to demonstrate the protocol.
            info!("Executing script {} ({} chars)", id, script.len());

            // Simulate execution
            let success = !script.is_empty();
            ServerEvent::ExecutionResult {
                id,
                success,
                output: if success { "Script executed".into() } else { String::new() },
                error: if success { None } else { Some("Empty script".into()) },
                duration_ms: 5,
            }
        }
        FrontendCommand::PollOutput { id: _ } => {
            ServerEvent::ConsoleOutput {
                lines: vec![],
            }
        }
        FrontendCommand::Configure { .. } => {
            info!("Sandbox configuration updated");
            ServerEvent::Status {
                message: "Configuration applied".to_string(),
                level: "info".to_string(),
            }
        }
        FrontendCommand::GetStatus { id: _ } => {
            // In the full build, this pulls ApexEngine::status().
            ServerEvent::StatusSnapshot {
                payload: serde_json::json!({
                    "mode": "external",
                    "tick_count": 0,
                    "behavior_state": "SmartFieldFarming",
                    "fused": null,
                }),
            }
        }
        FrontendCommand::SetMode { mode } => {
            let valid = matches!(mode.as_str(), "external" | "internal" | "dev");
            if valid {
                info!("Mode set: {mode}");
                ServerEvent::Status { message: format!("Mode set to {mode}"), level: "info".into() }
            } else {
                ServerEvent::Status { message: format!("Invalid mode: {mode}"), level: "error".into() }
            }
        }
        FrontendCommand::HubList { id: _ } => {
            ServerEvent::HubScriptList { scripts: serde_json::json!([]) }
        }
        FrontendCommand::RunHubScript { id } => {
            ServerEvent::Status { message: format!("Hub script '{id}' queued"), level: "info".into() }
        }
        FrontendCommand::RunScript { script, id } => {
            info!("Control script {id} ({} chars)", script.len());
            ServerEvent::ExecutionResult {
                id,
                success: !script.trim().is_empty(),
                output: "Script executed".into(),
                error: None,
                duration_ms: 3,
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialization() {
        let cmd = FrontendCommand::Execute {
            script: "print(1+1)".into(),
            id: "test-001".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""type":"execute""#));
        assert!(json.contains(r#""script":"print(1+1)""#));

        // Round-trip
        let parsed: FrontendCommand = serde_json::from_str(&json).unwrap();
        match parsed {
            FrontendCommand::Execute { script, id } => {
                assert_eq!(script, "print(1+1)");
                assert_eq!(id, "test-001");
            }
            _ => panic!("Wrong variant parsed"),
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = ServerEvent::ExecutionResult {
            id: "r-42".into(),
            success: true,
            output: "7".into(),
            error: None,
            duration_ms: 12,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"result""#));
        assert!(json.contains(r#""success":true"#));
    }

    #[test]
    fn test_invalid_command() {
        let result = serde_json::from_str::<FrontendCommand>(r#"{"type":"unknown"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_configure_command() {
        let cmd = FrontendCommand::Configure {
            max_instructions: Some(100_000),
            max_memory_mb: Some(128),
            timeout_secs: Some(60),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: FrontendCommand = serde_json::from_str(&json).unwrap();
        match parsed {
            FrontendCommand::Configure {
                max_instructions,
                max_memory_mb,
                timeout_secs,
            } => {
                assert_eq!(max_instructions, Some(100_000));
                assert_eq!(max_memory_mb, Some(128));
                assert_eq!(timeout_secs, Some(60));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_get_status_command() {
        let cmd = FrontendCommand::GetStatus { id: "s-1".into() };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""type":"get_status""#));
        let parsed: FrontendCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, FrontendCommand::GetStatus { .. }));
    }

    #[test]
    fn test_set_mode_command() {
        let cmd = FrontendCommand::SetMode { mode: "internal".into() };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: FrontendCommand = serde_json::from_str(&json).unwrap();
        match parsed {
            FrontendCommand::SetMode { mode } => assert_eq!(mode, "internal"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_hub_commands() {
        let list = FrontendCommand::HubList { id: "h-1".into() };
        assert!(serde_json::to_string(&list).unwrap().contains("hub_list"));

        let run = FrontendCommand::RunHubScript { id: "smart-farm".into() };
        assert!(serde_json::to_string(&run).unwrap().contains("smart-farm"));

        let script = FrontendCommand::RunScript { script: "print(1)".into(), id: "r-1".into() };
        assert!(serde_json::to_string(&script).unwrap().contains(r#""type":"run_script""#));
    }

    #[test]
    fn test_handle_new_commands() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let r = super::handle_command(FrontendCommand::SetMode { mode: "external".into() }).await;
            assert!(matches!(r, ServerEvent::Status { .. }));

            let r = super::handle_command(FrontendCommand::SetMode { mode: "bogus".into() }).await;
            if let ServerEvent::Status { level, .. } = r {
                assert_eq!(level, "error");
            } else {
                panic!("expected error status");
            }

            let r = super::handle_command(FrontendCommand::GetStatus { id: "s".into() }).await;
            assert!(matches!(r, ServerEvent::StatusSnapshot { .. }));
        });
    }
}
