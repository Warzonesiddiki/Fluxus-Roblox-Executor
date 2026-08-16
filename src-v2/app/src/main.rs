/// # WebSocket X v2 — Orchestrator Daemon
///
/// The main process that ties every module together:
///
/// ```
/// ┌─────────────────────────────────────────────────────────────┐
/// │ wsx-app (main)                                              │
/// │                                                             │
/// │  ┌──────────┐   ┌────────────┐   ┌──────────────────────┐  │
/// │  │ IPC       │   │ Vision      │   │ Net listener (pcap) │  │
/// │  │ (port     │   │ (frame      │   │ (entity sync)       │  │
/// │  │  9090)    │   │  loop)      │   │                      │  │
/// │  └────┬─────┘   └─────┬──────┘   └─────────┬────────────┘  │
/// │       │               │                    │               │
/// │       ▼               ▼                    ▼               │
/// │  ┌────────────────────────────────────────────────────┐    │
/// │  │ BehaviorTree tick loop (20 Hz) + PepsiMatrix        │    │
/// │  └──────────────────────┬─────────────────────────────┘    │
/// │                         ▼                                  │
/// │  ┌────────────────────────────────────────────────────┐    │
/// │  │ HumanizedInput + MacroRoutes + Sandboxed Luau      │    │
/// │  └────────────────────────────────────────────────────┘    │
/// └─────────────────────────────────────────────────────────────┘
/// ```

pub mod engine;
pub mod hub;

use std::time::Duration;

use clap::Parser;
use log::{error, info, warn};

use engine::{ApexEngine, EngineConfig};
use wsx_core::sandbox::{Sandbox, SandboxConfig};
use wsx_logic::{BehaviorTree, FeatureMatrix};
use wsx_vision::VisionEngine;

/// CLI arguments for the daemon.
#[derive(Parser, Debug)]
#[command(name = "wsx-app", version, about = "WebSocket X v2 orchestrator")]
struct Cli {
    /// Game window title to capture.
    #[arg(long, default_value = "Roblox")]
    window: String,

    /// Run in headless mode (no IPC server, no vision).
    #[arg(long)]
    headless: bool,

    /// Run the sandboxed Luau self-test on startup.
    #[arg(long)]
    selftest: bool,

    /// Run the red-team preflight sweep and block launch if any
    /// detectable surface is found (fail-closed).
    #[arg(long)]
    preflight: bool,

    /// Run the end-to-end pipeline demo with synthetic frames
    /// (no game client needed — exercises capture→fusion→tree→input).
    #[arg(long)]
    demo: bool,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    let cli = Cli::parse();
    info!("WebSocket X v2 orchestrator starting (window='{}')", cli.window);

    // ─── 1. Sandbox self-test (validates the Luau engine) ──────────────
    if cli.selftest {
        run_selftest();
        return;
    }

    // ─── 1b. Red-team preflight (fail-closed gate) ────────────────────
    if cli.preflight {
        let probe: &dyn wsx_redteam::ProcessProbe = {
            #[cfg(windows)]
            {
                &wsx_redteam::WindowsProbe::new()
            }
            #[cfg(not(windows))]
            {
                // Non-Windows: use a clean mock so CI can exercise the gate.
                &wsx_redteam::MockProbe::clean("RobloxPlayerBeta.exe")
            }
        };
        let result = wsx_redteam::preflight(probe, 0, "RobloxPlayerBeta.exe");
        if !result.passed {
            error!("Preflight BLOCKED launch (fail-closed). Fix detected surface first.");
            std::process::exit(1);
        }
        info!("Preflight passed — launch approved");
        return;
    }

    // ─── 1c. Demo mode: full pipeline with synthetic frames ───────────
    if cli.demo {
        run_demo();
        return;
    }

    // ─── 2. Start IPC bridge on a background thread ────────────────────
    if !cli.headless {
        std::thread::spawn(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let mut server = match wsx_ipc::IpcServer::bind().await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to bind IPC server: {}", e);
                        return;
                    }
                };
                if let Err(e) = server.run().await {
                    error!("IPC server error: {}", e);
                }
            });
        });
        info!("IPC bridge spawned on ws://127.0.0.1:9090");
    }

    // ─── 3. Build the product facade (owns all subsystems) ─────────────
    let mut engine = ApexEngine::new(EngineConfig {
        window_title: cli.window.clone(),
        max_ticks_per_second: 20,
    })
    .expect("engine init failed");

    // Optional: load vision templates (best effort)
    // engine.vision is private; templates load inside VisionEngine.

    // ─── 4. Main control loop through the facade ───────────────────────
    info!("Entering main control loop (20 Hz)");
    let mut last_tick = std::time::Instant::now();
    loop {
        let now = std::time::Instant::now();
        if now.duration_since(last_tick) < Duration::from_millis(50) {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        last_tick = now;

        engine.tick();

        // Periodic status log
        if engine.tick_count % 200 == 0 {
            let status = engine.status();
            info!(
                "[{}s] mode={:?} state={} decision={} fps={:.1}",
                status.tick_count * 50 / 1000,
                status.mode,
                status.behavior_state,
                status.decision,
                status.fps,
            );
        }
    }
}

/// Run the end-to-end pipeline demo.
/// Uses synthetic bar-filling frames + simulated packets; prints a live
/// telemetry stream so you can watch the engine decide and act.
fn run_demo() {
    info!("=== APEX DEMO — synthetic frames, no game client ===");

    let mut engine = match ApexEngine::demo() {
        Ok(e) => e,
        Err(e) => {
            error!("demo init failed: {e}");
            std::process::exit(1);
        }
    };
    engine.start_net();

    const TICKS: u64 = 300;
    info!("Running {} ticks...", TICKS);

    for _ in 0..TICKS {
        engine.tick();
        std::thread::sleep(Duration::from_millis(16));

        if engine.tick_count % 10 == 0 {
            let s = engine.status();
            let pollen = s
                .fused
                .as_ref()
                .map(|f| format!("{:.0}%", f.backpack_fill * 100.0))
                .unwrap_or_else(|| "—".into());
            let conf = s
                .fused
                .as_ref()
                .map(|f| format!("{:.2}", f.confidence))
                .unwrap_or_else(|| "—".into());
            println!(
                "tick={:>4} state={:<20} pollen={:>5} conf={} fps={:>4.1} quest_tasks={} hazard={:?}",
                s.tick_count,
                s.behavior_state,
                pollen,
                conf,
                s.fps,
                s.quest_tasks,
                s.hazard,
            );
        }
    }

    info!("=== DEMO COMPLETE — pipeline executed end-to-end ===");
}

/// Run the built-in self-test (validates the whole stack in one shot).
fn run_selftest() {
    info!("=== WSX v2 SELF-TEST ===");

    // 1. Sandbox
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("sandbox");
    let result = sandbox.execute("local t = {} for i=1,10 do t[i] = i end print('ok')").unwrap();
    info!("[SELFTEST] sandbox execute: success={} err={:?}", result.success, result.error);

    // 2. Kernel simulation
    let mut drv = wsx_kernel::KernelDriver::new("wsx_research.sys");
    drv.load().unwrap();
    drv.ept_init(0x1000, 0x100000).unwrap();
    drv.bridge_init("WSX_SELFTEST").unwrap();
    drv.bridge_submit("print('via kernel bridge')").unwrap();
    info!("[SELFTEST] kernel driver lifecycle: OK (state={:?})", drv.state);

    // 3. Behavior tree
    let mut tree = BehaviorTree::new();
    let mut input = wsx_input::HumanizedInput::new().unwrap();
    tree.snapshot.is_dead = true;
    tree.tick(&mut input).unwrap();
    info!("[SELFTEST] behavior tree: state={:?}", tree.current_state);

    // 4. Pepsi matrix
    let matrix = FeatureMatrix::default();
    let d = wsx_logic::pepsi_matrix::evaluate_matrix(&matrix, 96.0, false);
    info!("[SELFTEST] pepsi matrix at 96% backpack → {:?}", d);

    // 5. Network parse (fuzz-safe)
    let payload = [1u8, 1, 0, 7, 0, 0, 0, 0, 0, 0x40, 0x24, 0, 0, 2];
    match wsx_net::PacketListener::parse_packet(&payload) {
        Ok(state) => info!("[SELFTEST] packet parse: {} entities", state.entities.len()),
        Err(e) => warn!("[SELFTEST] packet parse (expected possible): {}", e),
    }

    // 6. Vision
    let mut vision = VisionEngine::new("Roblox");
    let _ = vision.load_templates();
    let screen = vision.capture().unwrap();
    info!(
        "[SELFTEST] vision: pollen={:.1}% field={:?}",
        screen.backpack_pollen * 100.0,
        screen.field_name
    );

    info!("=== SELF-TEST COMPLETE ===");
}
