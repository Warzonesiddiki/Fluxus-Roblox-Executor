# Apex — System Architecture (Blueprint B1.2)

**Version:** 0.1 · **Status:** Draft
**Methodology:** BMAD (Blueprint → Model → Architect → Deploy)
**Base:** `src-v2/` Rust workspace (9 crates, ~3.8k lines)

---

## 1. Component diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│  APEX PRODUCT (Windows)                                             │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  UI LAYER — Tauri (React/TS)                                 │  │
│  │  Editor · FeatureMatrix · Console · SnapshotInspector ·      │  │
│  │  RouteEditor · ModeSwitcher (A/B/C) · ScriptHub              │  │
│  └───────────────────────────┬──────────────────────────────────┘  │
│                              │ invoke (JSON over IPC loopback)     │
│  ┌───────────────────────────▼──────────────────────────────────┐  │
│  │  ENGINE LAYER (Rust)                                         │  │
│  │                                                              │  │
│  │  wsx-ipc ──────► wsx-logic ──────► wsx-driver ──► wsx-input │  │
│  │     │                 │                 │                     │  │
│  │     │                 │                 └──► humanizer (M2.1) │  │
│  │     │                 ▼                                      │  │
│  │     │          wsx-core (sandbox + AUNC runtime)             │  │
│  │     │                 │                                      │  │
│  │     │          wsx-vision ◄── screen capture (DXGI)          │  │
│  │     │          wsx-net    ◄── packet tap (pcap, passive)     │  │
│  │     │                                                         │  │
│  │     └──► wsx-kernel (simulated; Pillar B research)           │  │
│  └───────────────────────────┬──────────────────────────────────┘  │
│                              │                                     │
│  ┌───────────────────────────▼──────────────────────────────────┐  │
│  │  TARGET INTERFACE                                           │  │
│  │  Pillar A: OS (screen, HID, network tap)  — external         │  │
│  │  Pillar B: Research server client process — in-process       │  │
│  └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Data flow (the critical path)

```
Screen pixels ──► wsx-vision (capture→detect→ocr) ──┐
                                                      ├─► Fusion ──► GameSnapshot
Game packets ──► wsx-net (parse→sync) ───────────────┘      (confidence-scored)
                                                              │
GameSnapshot ──► wsx-logic (behavior tree @20Hz)              │
   │                  │                                       │
   │  ┌───────────────┴──────────────┐                        │
   │  │ Decision = fn(snapshot,      │                        │
   │  │   PepsiMatrix, quest_plan)   │                        │
   │  └───────────────┬──────────────┘                        │
   │                  ▼                                       │
   │  wsx-driver (macro route OR direct action)               │
   │                  │                                       │
   │                  ▼                                       │
   │  wsx-input humanizer (Bezier, latency, jitter)           │
   │                  │                                       │
   │                  ▼                                       │
   │  HID (SendInput/interception) ──► game client            │
   └───────────────────────────────────────────────────────────┘
```

**Budget:** capture→snapshot ≤ 50 ms p95 · snapshot→input ≤ 100 ms p95 · tick 50 ms.

---

## 3. Crate contracts (interfaces between modules)

### wsx-core
- `Sandbox::new(config) -> Sandbox`
- `Sandbox::execute(script) -> ExecutionResult {success, output, error, instructions, duration}`
- `Sandbox::output_buffer() -> Arc<OutputBuffer>`
- `Injector::attach(pid, name, level) -> ()`
- `Injector::inject(pid, name, payload) -> InjectedPayload`
- **AUNC runtime (M3):** `Runtime::load_script`, `Runtime::call_event(name, args)`, `Runtime::expose_api(table)`

### wsx-vision
- `VisionEngine::new(window) -> VisionEngine`
- `VisionEngine::capture() -> ScreenState` (backpack%, honey, quest, death, field, tokens, confidence)
- `TokenDetector::process_frame(rgba, w, h, px, py) -> &[Token]`
- **M1:** add `CaptureSource` trait (DXGI/BitBlt/offline-file) so Model phase can replay labeled frames.

### wsx-net
- `PacketListener::run()` / `stop()` / `snapshot() -> EntitySyncState`
- `parse_packet(&[u8]) -> EntitySyncState` (fuzz-safe)
- **M1:** protocol map for our server (packet type IDs, field offsets).

### wsx-logic
- `BehaviorTree::tick(&mut HumanizedInput) -> Result<(), BtError>`
- `BehaviorTree::update_snapshot(GameSnapshot)`
- `FeatureMatrix::apply_updates(HashMap<String, Value>)`
- `evaluate_matrix(&FeatureMatrix, backpack, quest) -> PepsiDecision`
- **M3:** `QuestPlanner::plan(snapshot) -> TaskQueue`, `HazardDetector::detect(snapshot) -> Threat`

### wsx-input (expanding now)
- `HumanizedInput::new() -> Result<Self, InputError>`
- `move_mouse_bezier(x, y, dur)`, `click()`, `press_key()`, `wait_ms()`
- **NEW M2.1:** `HumanizationEngine` — personality model, lognormal timing, session variance
- **NEW M2.1:** `InputEvent` recorder + `HumanCorpus` for statistical validation

### wsx-driver
- `MacroRoute::execute(&mut HumanizedInput)`
- `routes::field_to_hive()`, `field_to_mountain_top()`, etc.
- **M2:** `RouteRecorder` (record → save → replay with checkpoints)

### wsx-kernel
- Simulation-only: `KernelDriver` lifecycle, EPT stubs, Ring bridge (sandbox feature)
- **Research gate:** never ships in Pillar A product path

### wsx-ipc
- `IpcServer::bind()/run()` on `ws://127.0.0.1:9090`
- Protocol: `execute`, `poll_output`, `configure` (extend: `get_snapshot`, `set_mode`, `hub_list`)

---

## 4. Modes (pillars) — mode switcher contract

| Mode | Engine behavior | Target | Gate |
|---|---|---|---|
| `External` (A) | CV+packets+HID only | Any (undetectable) | Always available |
| `Internal` (B) | wsx-core injector + AUNC in-game | Own server allowlist only | Config flag + allowlist check, fails closed |
| `Dev` (C) | Sandbox only, no target | None | Always available |

---

## 5. Configuration & state

- `config.toml` (engine) + UI-stored overrides (FeatureMatrix)
- All state local; **no telemetry leaves the machine** (threat model rule)
- Reproducible: `cargo build --release` → hash-pinned artifacts

---

## 6. Error taxonomy

`Io` · `Config` · `Vision` (window/capture/ocr) · `Net` (device/parse) · `Sandbox` (compile/limit/timeout) · `Input` · `Policy` (gate violations) · `Ipc`

Every error: `{kind, context, retryable, source}` — serialized over IPC to the console panel.

---

## 7. Milestone mapping

| Blueprint artifact | Status |
|---|---|
| B1.1 THREAT_MODEL.md | ✅ done |
| B1.2 ARCHITECTURE.md | ✅ this doc |
| B1.3 AUNC_SPEC.md | 🔴 next |
| B1.4 server protocol map | 🔴 next |

| Model milestone | Status |
|---|---|
| M2.1 humanization statistical model | 🟡 building now (this turn) |
| M2.2 state-fusion model | 🔴 |
| M2.3 detection-surface model | 🔴 |
| M2.4 Pillar B feasibility | 🔴 |

---

*Next: B1.3 AUNC spec → M2.1 code (in progress).*
