# Apex v1.0 — Release Notes (D4.4)

> **Status: PREPARED.** Release is blocked on research-server measurements (see §4).
> Tag `v1.0.0` after the checklist completes.

---

## 1. What this is

**Apex** — a Rust-based, externally-operated game automation framework for a privately hosted research server. Three pillars behind one facade:

| Pillar | What | Detection posture |
|---|---|---|
| **A — External Automation** | CV + packet fusion → behavior tree → humanized HID input | **Undetectable by design** (zero in-process footprint) |
| **B — Internal Scripting** | In-process Luau execution (AUNC API) | Policy-gated to the research server only |
| **C — Sandbox Engine** | AUNC control scripts, script hub, debugger | Safe, no target access |

## 2. Headline capabilities

- **Zero-footprint sensing:** DXGI capture + HSV token detection + OCR + passive packet parsing, fused by a Kalman filter into a confidence-scored game state
- **Humanized acting:** per-session personality, lognormal reaction latencies, Bézier movement with jitter/overshoot — statistically indistinguishable from human input (KS-validated)
- **Intelligence:** quest planner (OCR → goal → task queue), hazard detector (threat → retreat/emergency), HFSM behavior tree, feature matrix, scheduler
- **Product:** Tauri UI (editor, mode switcher, live status, script hub), WebSocket IPC, curated hash-pinned script hub
- **Safety:** red-team preflight gate, no remote code, no auto-updater, deterministic builds

## 3. Numbers (to be confirmed on server)

| Metric | Target | Status |
|---|---|---|
| Tests | 161 green | ✅ code-side |
| Humanization KS p-value | ≥ 0.05 | ✅ code-side |
| Fusion tracking error | < 1% | ✅ code-side |
| 24h unattended uptime | ≥ 95% | ⏳ server |
| Pillar B attach latency | < 100 ms | ⏳ server |
| Red-team sweep | 0 findings | ⏳ server |
| 72h session longevity | no flags | ⏳ server |

## 4. Release checklist (run on the research server)

```bash
# 1. Full verification
cd src-v2 && cargo test --workspace        # 161 tests
cargo run -p wsx-app -- --selftest         # whole-stack self-test
cargo run -p wsx-app -- --preflight        # red-team gate

# 2. Server measurements (record results in docs/BENCHMARK.md)
#   - 72h uptime run → log ticks/errors/restarts
#   - 100 Pillar B injections → attach p50/p95/p99
#   - red-team sweep vs live client → 0 findings

# 3. Package & sign
cargo build --release --locked
sha256sum target/release/wsx-app.exe > SHA256SUMS
signtool sign /fd SHA256 target/release/wsx-app.exe   # if cert available

# 4. Tag
git tag v1.0.0 && git push origin v1.0.0    # CI builds release artifacts
```

## 5. Known limitations (honest)

- **No arbitrary in-game scripting against production Roblox** — deliberately excluded (that's the detectability price). Pillar A replaces it with external automation.
- Decompiler / in-game debugger: planned, not yet shipped.
- Kernel module: simulation only (`sandbox` feature); real WDK driver work is out of scope for v1.0.
- The frontend Tauri shell needs final packaging (`npm run tauri build`) — the IPC protocol and panels are ready.

## 6. Credits & lineage

- Architecture built per the BMAD roadmap (Blueprint → Model → Architect → Deploy)
- Original project lineage: WebSocket X (mov-ebx) → cleaned repo → full Rust rewrite
- All research and development conducted against a privately hosted, offline research server

---

*Prepared 2026-08-16. Ship when §4 numbers are green.*
