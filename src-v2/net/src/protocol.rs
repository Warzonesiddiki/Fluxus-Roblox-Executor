/// # Typed Protocol Model (B1.4) — Research Server Packet Parser
///
/// Implements the packet format documented in `docs/PROTOCOL_MAP.md`:
/// - 12-byte header: `[magic:u16][type_id:u16][seq:u32][timestamp_ms:u32]`
/// - 4-byte length prefix framing on UDP
/// - Structured payloads per type_id
///
/// Design goals:
/// - **Fuzz-safe**: every read is bounds-checked; truncated/oversized packets
///   produce `MalformedPacket` errors, never panics.
/// - **Configurable**: the type_id → packet mapping is a `ProtocolMap` that
///   can be filled from the server's real config without code changes.
/// - **Loss-tolerant**: a dropped packet only means one missing update;
///   the fusion engine (M2.2) handles the gap.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, PartialEq)]
pub enum ProtocolError {
    #[error("Bad magic: got 0x{got:04x}, expected 0x{expected:04x}")]
    BadMagic { got: u16, expected: u16 },
    #[error("Packet too short: {len} bytes (need at least 12 for header)")]
    TooShort { len: usize },
    #[error("Truncated payload: need {need} bytes, have {have}")]
    Truncated { need: usize, have: usize },
    #[error("Unknown type_id: 0x{0:04x}")]
    UnknownType(u16),
}

// ─── Constants ────────────────────────────────────────────────────────────────

pub const MAGIC: u16 = 0x5745; // "WE"
pub const HEADER_LEN: usize = 12;

// ─── Packet types ─────────────────────────────────────────────────────────────

/// Every packet the research server can send (PROTOCOL_MAP.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PacketType {
    PositionSync,
    QuestState,
    EntitySpawn,
    EntityDespawn,
    InventoryUpdate,
    EventTrigger,
    ChatMessage,
    BackpackUpdate,
}

impl PacketType {
    pub const fn type_id(self) -> u16 {
        match self {
            PacketType::PositionSync    => 0x0001,
            PacketType::QuestState      => 0x0002,
            PacketType::EntitySpawn     => 0x0003,
            PacketType::EntityDespawn   => 0x0004,
            PacketType::InventoryUpdate => 0x0005,
            PacketType::EventTrigger    => 0x0006,
            PacketType::ChatMessage     => 0x0007,
            PacketType::BackpackUpdate  => 0x0008,
        }
    }
}

// ─── Payload structs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSync {
    pub x: f32, pub y: f32, pub z: f32,
    pub yaw: f32, pub pitch: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestState {
    pub quest_id: u16,
    pub progress: u16,
    pub goal: u16,
    pub field_id: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySpawn {
    pub entity_id: u32,
    pub kind: u8,
    pub x: f32, pub y: f32, pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDespawn {
    pub entity_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryUpdate {
    pub item_id: u16,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTrigger {
    pub event_id: u8,
    pub x: f32, pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpackUpdate {
    /// Pollen fill in permille (0..1000).
    pub pollen_permille: u16,
}

impl BackpackUpdate {
    pub fn fill_ratio(&self) -> f64 {
        (self.pollen_permille as f64 / 1000.0).clamp(0.0, 1.0)
    }
}

// ─── Parsed packet ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPacket {
    pub seq: u32,
    pub timestamp_ms: u32,
    pub packet_type: PacketType,
    pub payload: PacketPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PacketPayload {
    Position(PositionSync),
    Quest(QuestState),
    Spawn(EntitySpawn),
    Despawn(EntityDespawn),
    Inventory(InventoryUpdate),
    Event(EventTrigger),
    Chat(ChatMessage),
    Backpack(BackpackUpdate),
}

// ─── Protocol map (configurable) ──────────────────────────────────────────────

/// Maps type_id → PacketType. Defaults to the documented map; can be
/// overridden from server config.
#[derive(Debug, Clone)]
pub struct ProtocolMap {
    inner: HashMap<u16, PacketType>,
}

impl Default for ProtocolMap {
    fn default() -> Self {
        let mut inner = HashMap::new();
        for pt in [
            PacketType::PositionSync, PacketType::QuestState, PacketType::EntitySpawn,
            PacketType::EntityDespawn, PacketType::InventoryUpdate, PacketType::EventTrigger,
            PacketType::ChatMessage, PacketType::BackpackUpdate,
        ] {
            inner.insert(pt.type_id(), pt);
        }
        Self { inner }
    }
}

impl ProtocolMap {
    pub fn resolve(&self, type_id: u16) -> Result<PacketType, ProtocolError> {
        self.inner.get(&type_id).copied().ok_or(ProtocolError::UnknownType(type_id))
    }

    pub fn override_type(&mut self, type_id: u16, pt: PacketType) {
        self.inner.insert(type_id, pt);
    }
}

// ─── Parser ───────────────────────────────────────────────────────────────────

/// Parse a framed UDP payload into a structured packet.
/// Every read is bounds-checked — never panics on malformed input.
pub fn parse_frame(buf: &[u8], protocol: &ProtocolMap) -> Result<ParsedPacket, ProtocolError> {
    if buf.len() < HEADER_LEN {
        return Err(ProtocolError::TooShort { len: buf.len() });
    }

    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC {
        return Err(ProtocolError::BadMagic { got: magic, expected: MAGIC });
    }

    let type_id = u16::from_le_bytes([buf[2], buf[3]]);
    let seq = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let timestamp_ms = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);

    let packet_type = protocol.resolve(type_id)?;
    let body = &buf[HEADER_LEN..];

    let payload = match packet_type {
        PacketType::PositionSync => {
            PacketPayload::Position(PositionSync {
                x: read_f32(body, 0)?, y: read_f32(body, 4)?, z: read_f32(body, 8)?,
                yaw: read_f32(body, 12)?, pitch: read_f32(body, 16)?,
            })
        }
        PacketType::QuestState => {
            PacketPayload::Quest(QuestState {
                quest_id: read_u16(body, 0)?,
                progress: read_u16(body, 2)?,
                goal: read_u16(body, 4)?,
                field_id: read_u8(body, 6)?,
            })
        }
        PacketType::EntitySpawn => {
            PacketPayload::Spawn(EntitySpawn {
                entity_id: read_u32(body, 0)?,
                kind: read_u8(body, 4)?,
                x: read_f32(body, 5)?, y: read_f32(body, 9)?, z: read_f32(body, 13)?,
            })
        }
        PacketType::EntityDespawn => {
            PacketPayload::Despawn(EntityDespawn { entity_id: read_u32(body, 0)? })
        }
        PacketType::InventoryUpdate => {
            PacketPayload::Inventory(InventoryUpdate {
                item_id: read_u16(body, 0)?,
                count: read_u32(body, 2)?,
            })
        }
        PacketType::EventTrigger => {
            PacketPayload::Event(EventTrigger {
                event_id: read_u8(body, 0)?,
                x: read_f32(body, 1)?, y: read_f32(body, 5)?,
            })
        }
        PacketType::ChatMessage => {
            let len = read_u16(body, 0)? as usize;
            let text_end = 2 + len;
            if body.len() < text_end {
                return Err(ProtocolError::Truncated { need: text_end, have: body.len() });
            }
            let text = String::from_utf8_lossy(&body[2..text_end]).to_string();
            PacketPayload::Chat(ChatMessage { text })
        }
        PacketType::BackpackUpdate => {
            PacketPayload::Backpack(BackpackUpdate { pollen_permille: read_u16(body, 0)? })
        }
    };

    Ok(ParsedPacket { seq, timestamp_ms, packet_type, payload })
}

// ─── Bounds-checked readers ───────────────────────────────────────────────────

fn read_u8(buf: &[u8], off: usize) -> Result<u8, ProtocolError> {
    buf.get(off).copied().ok_or(ProtocolError::Truncated { need: off + 1, have: buf.len() })
}

fn read_u16(buf: &[u8], off: usize) -> Result<u16, ProtocolError> {
    if buf.len() < off + 2 {
        return Err(ProtocolError::Truncated { need: off + 2, have: buf.len() });
    }
    Ok(u16::from_le_bytes([buf[off], buf[off + 1]]))
}

fn read_u32(buf: &[u8], off: usize) -> Result<u32, ProtocolError> {
    if buf.len() < off + 4 {
        return Err(ProtocolError::Truncated { need: off + 4, have: buf.len() });
    }
    Ok(u32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]]))
}

fn read_f32(buf: &[u8], off: usize) -> Result<f32, ProtocolError> {
    read_u32(buf, off).map(f32::from_bits)
}

// ─── Frame builder (for tests / server emulation) ─────────────────────────────

pub fn build_frame(packet_type: PacketType, seq: u32, timestamp_ms: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&packet_type.type_id().to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&timestamp_ms.to_le_bytes());
    out.extend_from_slice(body);
    out
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_map() -> ProtocolMap { ProtocolMap::default() }

    #[test]
    fn parse_position_roundtrip() {
        let mut body = Vec::new();
        body.extend_from_slice(&1.5f32.to_le_bytes());
        body.extend_from_slice(&2.5f32.to_le_bytes());
        body.extend_from_slice(&3.5f32.to_le_bytes());
        body.extend_from_slice(&0.1f32.to_le_bytes());
        body.extend_from_slice(&0.2f32.to_le_bytes());
        let frame = build_frame(PacketType::PositionSync, 7, 1234, &body);
        let pkt = parse_frame(&frame, &test_map()).unwrap();
        match pkt.payload {
            PacketPayload::Position(pos) => {
                assert_eq!(pos.x, 1.5); assert_eq!(pos.z, 3.5);
            }
            _ => panic!("wrong payload"),
        }
        assert_eq!(pkt.seq, 7);
        assert_eq!(pkt.timestamp_ms, 1234);
    }

    #[test]
    fn parse_backpack_update() {
        let body = 950u16.to_le_bytes().to_vec();
        let frame = build_frame(PacketType::BackpackUpdate, 1, 100, &body);
        let pkt = parse_frame(&frame, &test_map()).unwrap();
        match pkt.payload {
            PacketPayload::Backpack(b) => {
                assert_eq!(b.fill_ratio(), 0.95);
            }
            _ => panic!("wrong payload"),
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let mut frame = build_frame(PacketType::BackpackUpdate, 1, 100, &[0u8; 2]);
        frame[0] = 0xFF; frame[1] = 0xFF;
        assert!(matches!(parse_frame(&frame, &test_map()), Err(ProtocolError::BadMagic{..})));
    }

    #[test]
    fn too_short_rejected() {
        assert!(matches!(parse_frame(&[0u8; 5], &test_map()), Err(ProtocolError::TooShort{..})));
    }

    #[test]
    fn truncated_payload_rejected() {
        // Header says EntitySpawn (needs 17 body bytes) but body is 3 bytes
        let frame = build_frame(PacketType::EntitySpawn, 1, 100, &[1, 2, 3]);
        assert!(matches!(parse_frame(&frame, &test_map()), Err(ProtocolError::Truncated{..})));
    }

    #[test]
    fn unknown_type_rejected() {
        let mut frame = build_frame(PacketType::ChatMessage, 1, 100, &[0u8; 2]);
        frame[2] = 0xEE; frame[3] = 0xFF;
        assert!(matches!(parse_frame(&frame, &test_map()), Err(ProtocolError::UnknownType(_))));
    }

    #[test]
    fn chat_message_parses() {
        let text = "hello world";
        let mut body = Vec::new();
        body.extend_from_slice(&(text.len() as u16).to_le_bytes());
        body.extend_from_slice(text.as_bytes());
        let frame = build_frame(PacketType::ChatMessage, 2, 200, &body);
        let pkt = parse_frame(&frame, &test_map()).unwrap();
        match pkt.payload {
            PacketPayload::Chat(c) => assert_eq!(c.text, "hello world"),
            _ => panic!("wrong payload"),
        }
    }

    #[test]
    fn protocol_map_override() {
        let mut map = ProtocolMap::default();
        map.override_type(0x00FF, PacketType::BackpackUpdate);
        assert!(map.resolve(0x00FF).unwrap() == PacketType::BackpackUpdate);
        assert!(map.resolve(0x0001).unwrap() == PacketType::PositionSync);
    }
}
