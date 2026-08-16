/// # Passive Network Listening (pcap)
///
/// Intercepts unencrypted game packets to synchronise player position,
/// entity spawns, and world events — **without touching the game client
/// process**. This module is entirely passive (read-only taps).
///
/// ## Architecture
/// ```
/// Network Interface ──▶ libpcap/npcap capture loop
///                              │
///                              ▼
///                    Packet Parser (Rust)
///                              │
///                              ▼
///                    EntitySyncState (JSON)
///                              │
///                              ▼
///                    Behavior Tree / Frontend
/// ```
///
/// ## Safety
/// - Pure passive listening: no packet injection, no ARP spoofing.
/// - Requires admin/root privileges for raw capture (documented).
/// - All parsing is fuzz-safe: every field is bounds-checked.

pub mod protocol;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::{info, trace, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("Capture device not found: {0}")]
    DeviceNotFound(String),
    #[error("Permission denied — raw sockets require admin/root: {0}")]
    PermissionDenied(String),
    #[error("Capture loop terminated: {0}")]
    CaptureTerminated(String),
    #[error("Malformed packet (offset {offset}): {reason}")]
    MalformedPacket { offset: usize, reason: String },
}

/// Synchronised world state extracted from network traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySyncState {
    /// Player position (in-game coordinates, source: movement packets).
    pub player_position: (f64, f64, f64),
    /// Player rotation (yaw, pitch).
    pub player_rotation: (f32, f32),
    /// Other players/entities currently visible.
    pub entities: Vec<EntityInfo>,
    /// Timestamp of last packet.
    pub last_packet_at: f64,
    /// Current server region / map ID if present in packets.
    pub map_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: u64,
    pub position: (f64, f64, f64),
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntityKind {
    Player,
    Bee,
    Monster,
    Token,
    Npc,
    Other,
}

impl Default for EntitySyncState {
    fn default() -> Self {
        Self {
            player_position: (0.0, 0.0, 0.0),
            player_rotation: (0.0, 0.0),
            entities: Vec::new(),
            last_packet_at: 0.0,
            map_id: None,
        }
    }
}

/// Configuration for the packet capture loop.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Network interface to listen on (empty = auto-detect).
    pub interface: String,
    /// BPF filter string (e.g., "udp port 6969").
    pub filter: String,
    /// Snap length for captured packets.
    pub snaplen: i32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interface: String::new(),
            filter: "udp".to_string(),
            snaplen: 65535,
        }
    }
}

/// Passive packet capture engine. Clone shares the same state (Arc-based).
#[derive(Clone)]
pub struct PacketListener {
    config: CaptureConfig,
    /// Latest synchronised world state (shared with the logic thread).
    pub state: Arc<std::sync::Mutex<EntitySyncState>>,
    running: Arc<AtomicBool>,
    packet_count: Arc<std::sync::atomic::AtomicU64>,
}

impl PacketListener {
    pub fn new(config: CaptureConfig) -> Self {
        Self {
            config,
            state: Arc::new(std::sync::Mutex::new(EntitySyncState::default())),
            running: Arc::new(AtomicBool::new(true)),
            packet_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Start the capture loop (blocking). Spawn on its own thread.
    pub fn run(&self) -> Result<(), NetError> {
        info!("[NET] Starting capture on '{}' filter '{}'",
              if self.config.interface.is_empty() { "auto" } else { &self.config.interface },
              self.config.filter);

        // In production with the `real` feature:
        //   1. pcap::Device::lookup() → device
        //   2. device.open(Promiscuous, snaplen) → capture
        //   3. capture.filter(&config.filter, true)
        //   4. loop { match capture.next_packet() { ... } }
        //
        // For the sandbox/research build, we run a simulated
        // loop that occasionally emits mock entity sync state.

        let mut tick = 0u64;
        while self.running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(250));

            // Simulated packet → state update
            let mut state = self.state.lock().unwrap();
            state.player_position = (
                (tick as f64 * 0.5) % 100.0,
                (tick as f64 * 0.25) % 50.0,
                0.0,
            );
            state.last_packet_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            state.map_id = Some("BSS_PINE_TREE_FOREST".into());

            let count = self.packet_count.fetch_add(1, Ordering::SeqCst) + 1;
            if count % 40 == 0 {
                trace!("[NET] {} packets parsed", count);
            }
            tick += 1;
        }

        info!("[NET] Capture loop stopped");
        Ok(())
    }

    /// Stop the capture loop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Read the latest synchronised state.
    pub fn snapshot(&self) -> EntitySyncState {
        self.state.lock().unwrap().clone()
    }

    /// Parse a raw UDP payload into entity state (fuzz-safe).
    pub fn parse_packet(payload: &[u8]) -> Result<EntitySyncState, NetError> {
        // Very defensive parsing: every read is bounds-checked.
        // In production this matches the game's protocol structure.
        let mut state = EntitySyncState::default();

        if payload.len() < 4 {
            return Err(NetError::MalformedPacket {
                offset: 0,
                reason: "packet too short".into(),
            });
        }

        // Example: byte 0 = packet type, bytes 1..4 = entity count
        let _packet_type = payload[0];
        let entity_count = u16::from_le_bytes([payload[1], payload[2]]) as usize;

        // Guard against absurd counts
        let entity_count = entity_count.min(64);

        let mut offset = 3usize;
        for i in 0..entity_count {
            if offset + 13 > payload.len() {
                warn!("[NET] Truncated entity packet at index {}", i);
                break;
            }
            // Each entity: id(4) + x(4) + y(4) + kind(1)
            let id = u32::from_le_bytes([payload[offset], payload[offset+1], payload[offset+2], payload[offset+3]]);
            let x = f32::from_le_bytes([payload[offset+4], payload[offset+5], payload[offset+6], payload[offset+7]]);
            let y = f32::from_le_bytes([payload[offset+8], payload[offset+9], payload[offset+10], payload[offset+11]]);
            let kind_byte = payload[offset+12];
            offset += 13;

            state.entities.push(EntityInfo {
                id: id as u64,
                position: (x as f64, y as f64, 0.0),
                kind: match kind_byte {
                    0 => EntityKind::Player,
                    1 => EntityKind::Bee,
                    2 => EntityKind::Monster,
                    3 => EntityKind::Token,
                    4 => EntityKind::Npc,
                    _ => EntityKind::Other,
                },
            });
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_parse_valid() {
        // 2 entities: id=1,(10.0,20.0,0),kind=0  and  id=2,(30.0,40.0,0),kind=3
        let mut payload = vec![0u8; 3 + 13 * 2];
        payload[0] = 1; // type
        payload[1] = 2; payload[2] = 0; // count = 2

        // entity 1
        payload[3..7].copy_from_slice(&1u32.to_le_bytes());
        payload[7..11].copy_from_slice(&10.0f32.to_le_bytes());
        payload[11..15].copy_from_slice(&20.0f32.to_le_bytes());
        payload[15] = 0; // Player

        // entity 2
        payload[16..20].copy_from_slice(&2u32.to_le_bytes());
        payload[20..24].copy_from_slice(&30.0f32.to_le_bytes());
        payload[24..28].copy_from_slice(&40.0f32.to_le_bytes());
        payload[28] = 3; // Token

        let state = PacketListener::parse_packet(&payload).unwrap();
        assert_eq!(state.entities.len(), 2);
        assert_eq!(state.entities[0].id, 1);
        assert_eq!(state.entities[1].id, 2);
        assert!(matches!(state.entities[1].kind, EntityKind::Token));
    }

    #[test]
    fn test_packet_parse_truncated_does_not_panic() {
        let payload = vec![1u8, 50, 0, 1, 2, 3]; // claims 50 entities but is short
        let state = PacketListener::parse_packet(&payload).unwrap();
        assert!(state.entities.is_empty() || state.entities.len() < 50);
    }

    #[test]
    fn test_packet_parse_too_short() {
        let payload = vec![1u8, 2];
        let result = PacketListener::parse_packet(&payload);
        assert!(matches!(result, Err(NetError::MalformedPacket { .. })));
    }

    #[test]
    fn test_listener_snapshot_cycle() {
        let listener = PacketListener::new(CaptureConfig::default());
        let handle = std::thread::spawn(move || {
            let _ = listener.run();
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        let state = listener.snapshot();
        assert!(state.last_packet_at > 0.0);
        listener.stop();
        let _ = handle.join();
    }
}
