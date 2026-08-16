# Apex — Verification Guide (D4.3)

How to reproduce every claim in this project, from one command to the full BMAD loop.

---

## 1. Quick start

```bash
cd src-v2
cargo test --workspace          # all 161 tests
cargo run -p wsx-app -- --selftest   # whole-stack self-test
cargo run -p wsx-app -- --preflight  # red-team gate (mock on non-Windows)
```

## 2. What each test suite proves

| Suite | File(s) | Proves |
|---|---|---|
| Process & injection | `core/src/process/mod.rs` | error paths, size caps, handle lifecycle |
| Sandbox | `core/src/sandbox.rs` | empty-script rejection, limits, globals injection |
| AUNC runtime | `core/src/runtime.rs` | policy gate, API surface, lifecycle |
| AUNC control API | `core/src/control_api.rs` | spec surface completeness, validation, flow |
| IPC protocol | `ipc/src/lib.rs` | command/event serialization, async handlers |
| Humanization | `input/src/humanizer.rs` + `stats.rs` | lognormal bounds, Bezier endpoints, KS tests |
| **M2.1 acceptance** | `tests/tests/humanization_acceptance.rs` | **synthetic input indistinguishable from human (KS p≥0.05)** |
| Capture | `vision/src/capture.rs` | crop bounds, offline replay, synthetic bar frames |
| Token detector | `vision/src/token_detector.rs` | HSV math, frame processing |
| **M2.2 fusion** | `vision/src/fusion.rs` + `tests/tests/state_fusion_acceptance.rs` | **Kalman tracking, degradation, no-panic scenario** |
| Protocol parser | `net/src/protocol.rs` | round-trips, truncation, unknown types (fuzz-safe) |
| Packet listener | `net/src/lib.rs` | parse safety, snapshot cycle |
| Behavior tree | `logic/src/behavior_tree.rs` | state priorities, full cycle, stuck detection |
| Quest planner | `logic/src/quest_planner.rs` | text→goal inference, task queues |
| Hazard detector | `logic/src/hazard.rs` | threat levels, escalation, confidence |
| Pepsi matrix | `logic/src/pepsi_matrix.rs` | decision priority, updates |
| Scheduler | `logic/src/scheduler.rs` | humanization timing bounds |
| Macro routes | `driver/src/macro_sequences.rs` | route structure, conversion included |
| **M2.4 feasibility** | `tests/tests/pillar_b_feasibility.rs` | gate fails closed, API matches spec |
| **M2.3 red-team** | `redteam/src/lib.rs` | clean passes, contaminated fails, preflight gate |
| Engine facade | `app/src/engine.rs` | mode switching, tick loop, script execution |
| Script hub | `app/src/hub.rs` | unique ids, sources, sha256 format |

## 3. Milestone acceptance criteria → verification commands

| Criterion | Command | Pass bar |
|---|---|---|
| M2.1 humanization | `cargo test --test humanization_acceptance` | 4/4 green (KS p≥0.05) |
| M2.2 fusion accuracy | `cargo test --test state_fusion_acceptance` | 5/5 green (<1% error) |
| M2.3 zero footprint | `cargo test -p wsx-redteam` | clean mock passes, dirty fails |
| M2.4 feasibility | `cargo test --test pillar_b_feasibility` | 5/5 green |
| Full workspace | `cargo test --workspace` | 161/161 green |
| Whole stack | `cargo run -p wsx-app -- --selftest` | all sections log OK |
| Red-team gate | `cargo run -p wsx-app -- --preflight` | exit 0 on clean surface |

## 4. CI (BMAD loop automated)

`.github/workflows/ci.yml` runs Build → Test → Audit → Release on every push.
The `-D warnings` clippy gate means the workspace must be warning-free.

## 5. Reproducibility

- `rust-toolchain.toml` pins 1.83.0
- `Cargo.lock` pins dependencies (`--locked` builds)
- Release artifacts ship with `SHA256SUMS`
