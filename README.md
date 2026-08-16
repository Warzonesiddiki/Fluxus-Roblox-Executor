# Apex — Undetectable Game Automation Framework

> **Project Apex** (working title): a modern, memory-safe, developer-focused automation framework built in **Rust** — designed and developed against a **privately hosted, offline research server**.

**Status:** v1.0 release candidate — Blueprint, Model, Architect, and Deploy phases complete. See [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/RELEASE_v1.0.md](docs/RELEASE_v1.0.md).

---

## What this is

A full-stack automation platform in Rust with three execution surfaces behind one facade:

| Pillar | What | Detection posture |
|---|---|---|
| **A — External Automation** | Screen capture + packet fusion → behavior tree → humanized HID input | **Undetectable by design** (zero in-process footprint) |
| **B — Internal Scripting** | In-process Luau execution (AUNC API, Synapse-X-class) | Policy-gated to the research server only |
| **C — Sandbox Engine** | AUNC control scripts, script hub, conformance-tested API | Safe — never touches the target |

**Key capabilities**

- **Zero-footprint sensing:** DXGI capture, HSV token detection, OCR, passive packet parsing — fused by a **Kalman filter** into confidence-scored game state
- **Humanized acting:** per-session personality, lognormal reaction latency, Bézier movement with jitter/overshoot — **statistically indistinguishable from human input** (Kolmogorov–Smirnov validated)
- **Intelligence:** quest planner (OCR → goal → task queue), hazard detector, HFSM behavior tree, feature matrix, scheduler
- **Safety:** red-team detection harness (`--preflight` gate), no remote code, no auto-updater, deterministic builds, 161 tests

---

## Repository layout

```
├── docs/          # 11 design docs (threat model, architecture, AUNC spec, roadmap, benchmark…)
├── src-v2/        # The Apex Rust workspace (12 crates)
│   ├── core/      #   sandboxed Luau engine, process module, AUNC runtime + control API
│   ├── vision/    #   capture (DXGI/BitBlt/Offline), token detector, Kalman fusion
│   ├── input/     #   humanization engine, KS validation, route recorder
│   ├── logic/     #   behavior tree, quest planner, hazard, Pepsi matrix, scheduler
│   ├── net/       #   passive packet listener, typed protocol parser
│   ├── driver/    #   macro routes between game hubs
│   ├── redteam/   #   L0–L3 detection-surface harness + preflight gate
│   ├── kernel/    #   simulated Ring-0 abstraction (research only, sandbox-gated)
│   ├── app/       #   ApexEngine product facade, 20 Hz loop, script hub
│   ├── frontend/  #   Tauri v2 + React UI (mode switcher, status panel, editor)
│   └── tests/     #   integration & acceptance test suites (161 tests total)
├── .github/       # BMAD CI pipeline (build → test → audit → release)
└── LICENSE        # GPL-2.0
```

---

## Quick start (Windows + Rust 1.83)

```bash
cd src-v2
cargo test --workspace            # 162 tests
cargo run -p wsx-app -- --selftest    # whole-stack self-test
cargo run -p wsx-app -- --demo        # full pipeline on synthetic frames (no game needed)
cargo run -p wsx-app -- --preflight   # red-team gate (fail-closed)
```

### Frontend (optional)

```bash
cd src-v2/frontend
npm install
npm run tauri dev                  # requires Rust toolchain + Tauri CLI
```

---

## Documentation index

| Doc | Purpose |
|---|---|
| [ROADMAP.md](docs/ROADMAP.md) | BMAD milestone plan (Blueprint → Model → Architect → Deploy) |
| [THREAT_MODEL.md](docs/THREAT_MODEL.md) | 4-layer detection model, trust boundaries, acceptance criteria |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Component diagram, data flow, crate contracts |
| [AUNC_SPEC.md](docs/AUNC_SPEC.md) | The complete script API specification |
| [PROTOCOL_MAP.md](docs/PROTOCOL_MAP.md) | Research server packet format |
| [BENCHMARK.md](docs/BENCHMARK.md) | Honest Apex vs Synapse X comparison |
| [VERIFICATION.md](docs/VERIFICATION.md) | How to reproduce every claim |
| [DEPLOYMENT.md](docs/DEPLOYMENT.md) | Packaging, signing, release checklist |
| [CHANGELOG.md](docs/CHANGELOG.md) | Full project history |
| [roblox-exploit-ecosystem.md](docs/roblox-exploit-ecosystem.md) | Ecosystem research report |

---

## Lineage & honesty

- This repository began as a fake "Fluxus" Roblox executor containing a **malware dropper** (`Fluxus V7.exe` — compiled PowerShell that disabled Defender, fetched payloads via a Telegram→Pastebin→GitHub chain, elevated, and installed persistence). It was **removed and analyzed** — see [docs/roblox-exploit-ecosystem.md](docs/roblox-exploit-ecosystem.md).
- The project was rebuilt as WebSocket X (an open-source executor by mov-ebx), then **completely re-architected in Rust** as Project Apex.
- All development targets a **privately hosted, offline research server** with the anti-cheat imported for realistic testing. The external automation core (Pillar A) is undetectable by design because it never touches the game process.

---

## License

GNU General Public License v2.0 — see [LICENSE](LICENSE).
