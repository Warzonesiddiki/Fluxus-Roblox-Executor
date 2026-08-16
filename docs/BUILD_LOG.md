# Apex — First Build & Verification Log (Step-by-Step)

> **Purpose:** guide the first `cargo build` on a Windows machine so the project goes
> from "compiles" to "runs on the research server" with minimal friction.
> **Written:** 2026-08-16 · **Pre-verified:** import graph complete, no unused imports,
> brace balance clean — but **no compiler has ever run on this code**.

---

## 0.5 One-command verification

```powershell
# Windows
powershell -File scripts/verify.ps1
# or bash (WSL/Git Bash)
bash scripts/verify.sh
```
Runs: build → 162 tests → selftest → **demo pipeline** → preflight gate.
The **demo** (`cargo run -p wsx-app -- --demo`) runs the full engine loop on
synthetic frames — capture→fusion→quest/hazard→behavior tree→humanized input —
so you can see the pipeline work *before* implementing hardware capture.

---

## 0. Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust toolchain | 1.83.0 (pinned by `rust-toolchain.toml`) | [rustup.rs](https://rustup.rs) |
| Windows | 10/11 | — |
| Game client | Your research server's client | — |
| (Frontend, optional) | Node 18+, Tauri CLI | `npm i -g @tauri-apps/cli` |

```powershell
# 1. Install Rust
winget install Rustlang.Rustup
# or download from rustup.rs, then:
rustup toolchain install 1.83.0
```

---

## 1. First compile (expect errors — that's normal)

```powershell
cd src-v2
cargo build --workspace 2>&1 | Tee-Object -FilePath build-errors.txt
```

### Likely error classes (pre-analyzed)

| Class | Where | Fix pattern |
|---|---|---|
| `windows` crate API signature drift | `core/src/process/mod.rs` | The Win32 calls (`OpenProcess`, `VirtualAllocEx`, `CreateRemoteThread`, `WriteProcessMemory`) were written from the 0.58 API docs; if a signature differs, read the error, adjust args (usually `Some(x)`/`None` and handle types) |
| `luau-src` optional dep missing | `core/Cargo.toml` | `luau = { package = "luau-src", version = "0.3", optional = true }` — if the crate version is wrong, run `cargo search luau-src` and bump; or build Luau from source per its README |
| `enigo` commented out | `input/Cargo.toml` | Uncomment + add `enigo = "0.2"` when implementing real HID (Step 3) |
| Test-only symbols | `tests/tests/*.rs` | Cross-checks passed; any failure = fix the test's import/type |

**Workflow:** fix → `cargo build` → repeat. Target: `cargo build --workspace` clean.

---

## 2. Run the tests (161 expected)

```powershell
cargo test --workspace 2>&1 | Tee-Object -FilePath test-results.txt
```

| Suite | Count | Gate |
|---|---|---|
| `wsx-core` (process, sandbox, runtime, control_api) | ~30 | all pass |
| `wsx-ipc` (protocol, async handlers) | ~12 | all pass |
| `wsx-vision` (capture, token, fusion) | ~18 | all pass |
| `wsx-input` (humanizer, stats, recorder) | ~25 | all pass |
| `wsx-logic` (tree, quest, hazard, pepsi, scheduler) | ~35 | all pass |
| `wsx-driver` / `wsx-net` / `wsx-redteam` | ~20 | all pass |
| `wsx-tests` (acceptance suites) | ~21 | **M2.1/M2.2/M2.4 gates** |
| `wsx-app` (engine facade, hub) | ~15 | all pass |

**Acceptance gates (must be green):**
- `tests/tests/humanization_acceptance.rs` → KS p ≥ 0.05 (M2.1)
- `tests/tests/state_fusion_acceptance.rs` → tracking error < 1% (M2.2)
- `tests/tests/pillar_b_feasibility.rs` → gate fails closed (M2.4)

---

## 3. Implement the real hardware layers (the actual work)

The framework runs with placeholders until you implement these:

### 3a. Real screen capture — `vision/src/capture.rs`
Replace the `CapturedFrame::solid()` placeholder in `DxgiDuplicationSource::grab()`:
1. `FindWindowW` the game window title
2. `D3D11CreateDevice` + `IDXGIOutputDuplication::AcquireNextFrame`
3. Copy the texture → RGBA `CapturedFrame` → `ReleaseFrame`
4. Wire: `VisionEngine::capture()` must call the capture source (currently returns mock `ScreenState`)

### 3b. Real OCR — `vision/src/lib.rs` `ocr_region()`
- Add `tesseract` or `leptess` crate; extract text from `HONEY_COUNTER` / `QUEST_DIALOG` regions
- Feed results into `ScreenState.quest_text` / `honey_count`

### 3c. Real input — `input/src/lib.rs`
- Uncomment `enigo` dep; implement `HumanizedInput` methods with `enigo::Enigo`
- Keep the humanization engine (latency from personality, Bezier paths) — it's the anti-detection layer

### 3d. Real packet capture — `net/src/lib.rs`
- Enable the `real` feature + `pcap`/`wpcap` crates
- Replace the simulated loop in `PacketListener::run()` with `pcap::Capture`
- Map packets via `net/src/protocol.rs` per `docs/PROTOCOL_MAP.md`

### 3e. (Pillar B, own server) Real injection — `core/src/runtime.rs` + `process/mod.rs`
- Verify the Win32 attach/inject path against the research client
- Build the in-game runtime DLL that exposes the AUNC InGame API

---

## 4. Self-test + preflight

```powershell
cargo run -p wsx-app -- --selftest    # sandbox + kernel + tree + vision + net
cargo run -p wsx-app -- --preflight   # red-team gate (0 findings required)
```

---

## 5. Research-server measurement run (D4.4 → v1.0)

Follow `docs/RELEASE_v1.0.md` §4:
1. 72h unattended run → log uptime/errors → target ≥ 95%
2. 100 Pillar B injections → attach p50/p95/p99 → target < 100 ms
3. Red-team sweep vs live client → 0 findings
4. Record numbers into `docs/BENCHMARK.md`

---

## 6. Frontend (optional)

```powershell
cd src-v2/frontend
npm install
npm run tauri dev
```
(The IPC protocol + panels are ready; the frontend is a dev shell until wired to a live engine.)

---

## Progress tracker

- [ ] Step 1: `cargo build --workspace` clean (fix the error classes above)
- [ ] Step 2: `cargo test --workspace` — 161 green, 3 acceptance gates pass
- [ ] Step 3a: real DXGI capture
- [ ] Step 3b: real OCR
- [ ] Step 3c: real HID input (enigo)
- [ ] Step 3d: real pcap capture
- [ ] Step 3e: Pillar B injection on own server
- [ ] Step 4: selftest + preflight pass
- [ ] Step 5: server measurements → v1.0 release

> **Realistic expectation:** Steps 1–2 take ~1–3 hours (fixing compile drift).
> Step 3 is days-to-weeks of Windows/game integration work.
> Everything in this log is reproducible from source — no black boxes.
