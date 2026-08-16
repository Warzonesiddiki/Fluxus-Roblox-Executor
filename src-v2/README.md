# WebSocket X v2 — Full-Stack Automation & Execution Framework

A modern, memory-safe, developer-focused automation framework built in **Rust**. It combines:

- **`core`** — Process attachment + sandboxed Luau/LuaJIT execution engine
- **`ipc`** — WebSocket bridge (ws://127.0.0.1:9090) for zero-lag script streaming
- **`vision`** — Computer vision (OpenCV-style) token/UI detection
- **`input`** — Humanized hardware-level input (Bezier curves, jitter, micro-delays)
- **`logic`** — HFSM behaviour tree + Pepsi Matrix decision engine + scheduler
- **`driver`** — Pre-recorded macro routes between game hubs
- **`kernel`** — *Simulated* kernel-level memory abstraction (sandbox/research only)
- **`frontend`** — Tauri (React/TS) control console

```
┌──────────────────────────────────────────────────────────────┐
│  Tauri UI (React)                                            │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ Editor      │  │ Feature      │  │ Console Output     │  │
│  │ (Monaco)    │  │ Matrix       │  │ (live streaming)   │  │
│  └─────────────┘  └──────────────┘  └────────────────────┘  │
└──────────────────────────┬───────────────────────────────────┘
                           │ WebSocket (ws://127.0.0.1:9090)
┌──────────────────────────▼───────────────────────────────────┐
│  wsx-ipc  — JSON protocol, async routing                    │
├──────────────────────────────────────────────────────────────┤
│  wsx-logic — BehaviorTree (20 Hz tick)                      │
│   ├─ EmergencyRecovery > Quest > HiveReturn > FieldFarming  │
│   ├─ PepsiMatrix (feature toggles)                          │
│   └─ Scheduler + HumanizationLayer                          │
├──────────────────────────────────────────────────────────────┤
│  wsx-vision → wsx-driver → wsx-input                        │
│   CV frames  →  macro routes  →  Bezier/hardware input      │
├──────────────────────────────────────────────────────────────┤
│  wsx-core — Injector + Sandboxed Luau                       │
│  wsx-kernel — simulated Ring-0 abstraction (research only)  │
└──────────────────────────────────────────────────────────────┘
```

---

## Crate layout

| Crate | Path | Purpose |
|---|---|---|
| `wsx-core` | `core/` | PID targeting, Win32 injection (RAII-safe), zero-trust Luau sandbox · ApexRuntime (M2.4) · **AUNC Control API (A3.4)**: spec-locked §2 surface + conformance tracking |
| `wsx-ipc` | `ipc/` | WebSocket JSON bridge — `execute`, `poll_output`, `configure` commands |
| `wsx-vision` | `vision/` | Screen capture → HSV colour segmentation → token tracking · Kalman fusion (M2.2) · **capture abstraction (A3.1)**: DXGI/BitBlt/Offline sources |
| `wsx-input` | `input/` | Humanization engine (M2.1) · **route recorder (A3.2)** · personality-driven `HumanizedInput` |
| `wsx-logic` | `logic/` | Behavior tree (HFSM) · Pepsi Matrix · scheduler · **quest planner (A3.3)** · **hazard detector (A3.3)** |
| `wsx-driver` | `driver/` | `macro_sequences.rs` — pre-recorded hub-to-hub routes |
| `wsx-kernel` | `kernel/` | Simulated driver lifecycle, EPT shadow pages, Ring0→Ring3 bridge (sandbox only) |
| `wsx-redteam` | `redteam/` | **Detection-surface harness (M2.3)** — L0–L3 footprint scan, analyzer + mock probe |
| `wsx-net` | `net/` | Passive packet listening (pcap) + **typed protocol parser (B1.4)** — framed UDP → structured packets |
| `wsx-app` | `app/` | **Orchestrator + product facade** — `ApexEngine` (A3.5) owns all subsystems · **script hub** (curated) · control loop |
| `wsx-frontend` | `frontend/` | Tauri v2 + React + Monaco editor console |

---

## Building

### Backend (all crates)

```bash
cd src-v2
cargo build --workspace        # debug
cargo test  --workspace        # run the full verification suite
cargo run -p wsx-app -- --selftest   # validate the whole stack in one shot
```

### Frontend

```bash
cd src-v2/frontend
npm install
npm run tauri dev              # requires Rust toolchain + Tauri CLI
```

---

## Verification suite

`tests/integration_test.rs` covers:

1. **Sandbox math execution** — simple arithmetic runs without crashing
2. **Dangerous global rejection** — `io`/`os` stripped
3. **Instruction limit** — runaway loops are capped
4. **IPC round-trip** — command serialization → deserialization
5. **Behavior tree lifecycle** — death → hive return → farming transitions
6. **Pepsi Matrix decisions** — quest > hive > gathering priority
7. **Pipeline mock** — 20-tick simulated game session
8. **Vision capture** — mock frame produces valid state
9. **Kernel sandbox lifecycle** — driver load/hide/EPT/bridge simulation
10. **Macro route** — executes `Field → Hive` route
11. **Humanization layer** — AFK/blink timing bounds

---

## Feature matrix (Pepsi Swarm port)

| Feature | Toggle | Module |
|---|---|---|
| Dynamic Token Telemetry | `token_farm` | `vision/token_detector.rs` |
| Auto Ability/Link/Sprout | `auto_ability` / `auto_link` / `auto_sprout` | `logic/pepsi_matrix.rs` |
| Auto Feed / Train | `auto_feed` / `auto_train` | `driver/macro_sequences.rs` |
| Blender Crafting | `auto_blender` | `driver/macro_sequences.rs` |
| Auto Dispense | `auto_dispense` | `logic/behavior_tree.rs` |
| Quest Claims | `auto_quest` | `logic/behavior_tree.rs` |
| Cannon / Slingshot | `auto_cannon` | `driver/macro_sequences.rs` |
| Honeystorm / Meteor | `request_honeystorm` / `auto_meteor` | `logic/scheduler.rs` |

---

## Security model

- **Zero trust sandbox**: `io`, `os`, `debug`, `loadfile`, `require` are stripped. Instruction count, memory, and wall-clock limits enforced.
- **No network calls** from the core engine except the localhost IPC loopback.
- **Humanization layer** makes input patterns statistically indistinguishable from human play (Bezier curves + jitter + randomized latency).
- **Kernel module is simulation-only** (`sandbox` feature). Real `hw` mode is gated behind a feature flag and documented as research-only, requiring a proper WDK toolchain, signed certificates, and explicit opt-in.

---

## License

GPL-2.0 — see root `LICENSE`.
