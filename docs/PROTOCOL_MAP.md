# Apex — Research Server Protocol Map (Blueprint B1.4)

**Version:** 0.1 · **Status:** Template — fill from server source
**Context:** We own the research server, so this is documentation, not reverse engineering. The `wsx-net` crate implements the typed model in `protocol.rs`.

---

## 1. Transport

| Property | Value (default) | Notes |
|---|---|---|
| Transport | UDP | Roblox-style unreliable messaging for position/state |
| Default port | 49152–50000 (server-assigned) | Confirm from server config |
| Framing | 4-byte little-endian length prefix | `[len:u32][payload]` |
| Encryption | None (research server) | Document here if TLS is enabled |
| Header | 12-byte fixed | `[magic:u16][type_id:u16][seq:u32][timestamp_ms:u32]` |

## 2. Packet types (type_id → struct)

| type_id | Name | Payload | Direction |
|---|---|---|---|
| 0x0001 | `PositionSync` | `[x:f32][y:f32][z:f32][yaw:f32][pitch:f32]` | S→C |
| 0x0002 | `QuestState` | `[quest_id:u16][progress:u16][goal:u16][field_id:u8]` | S→C |
| 0x0003 | `EntitySpawn` | `[entity_id:u32][kind:u8][x:f32][y:f32][z:f32]` | S→C |
| 0x0004 | `EntityDespawn` | `[entity_id:u32]` | S→C |
| 0x0005 | `InventoryUpdate` | `[item_id:u16][count:u32]` | S→C |
| 0x0006 | `EventTrigger` | `[event_id:u8][x:f32][y:f32]` | S→C |
| 0x0007 | `ChatMessage` | `[len:u16][utf8]` | S→C |
| 0x0008 | `BackpackUpdate` | `[pollen_permille:u16]` | S→C |

## 3. Field ID map

| field_id | Field |
|---|---|
| 0 | Pine Tree Forest |
| 1 | Sunflower Field |
| 2 | Cactus Canyon |
| 3 | Bamboo Field |
| 4 | Pumpkin Patch |
| 5 | Mushroom Field |
| 6 | Clover Field |
| 7 | Strawberry Field |
| 8 | Pepper Patch |
| 9 | Rose Field |
| 10 | Blue Flower Field |
| 11 | Mountain Top Field |

## 4. Entity kinds

| kind | Meaning |
|---|---|
| 0 | Player |
| 1 | Bee |
| 2 | Monster |
| 3 | Token (collectible) |
| 4 | NPC |
| 5 | Sprout |

## 5. Mapping to GameSnapshot (fusion inputs)

| GameSnapshot field | Packet source | Vision source |
|---|---|---|
| `position` | 0x0001 PositionSync | token centroid / player template |
| `backpack_fill` | 0x0008 BackpackUpdate (pollen_permille/1000) | pollen bar green-fill ratio |
| `quest` | 0x0002 QuestState | quest dialog OCR |
| `field` | 0x0001/0x0002 field_id | minimap OCR |
| `is_dead` | — (no death packet) | respawn overlay template |
| `tokens` | 0x0003 EntitySpawn (kind=3) | HSV token detector |

**Fusion priority:** packet data wins on `position` (lower latency, higher precision); vision wins on `backpack_fill` and `is_dead` (packets don't carry them); quest/field use packet-if-available, vision-fallback.

## 6. TODO (server-side confirmation)

- [ ] Confirm exact port + encryption
- [ ] Confirm field_id list (add any custom fields)
- [ ] Confirm quest_id registry (for quest planner)
- [ ] Confirm event_id registry (honeystorm/meteor/sprout)

---

*Next: implement `wsx-net/protocol.rs` (done in parallel), then M2.3 red-team harness.*
