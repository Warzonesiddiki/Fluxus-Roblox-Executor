# Changelog — Project Apex (WebSocket X v2)

All notable changes, tracked per the BMAD cycle.

## [v1.0.0] — planned (D4.4)

### Blueprint
- Threat model (4-layer detection: L0–L3) — `docs/THREAT_MODEL.md`
- System architecture + crate contracts — `docs/ARCHITECTURE.md`
- AUNC script API spec (Control + InGame surfaces) — `docs/AUNC_SPEC.md`
- Research server protocol map + typed parser — `docs/PROTOCOL_MAP.md`

### Model (validated in code)
- **M2.1** Humanization engine: per-session personality, lognormal latency, Bezier + overshoot, idle planner; KS-test validation harness (p ≥ 0.05 acceptance)
- **M2.2** Kalman state fusion: vision + packets + motion → confidence-scored snapshot; graceful degradation under source loss
- **M2.3** Red-team detection harness: L0–L3 footprint scan, analyzer, mock probe, preflight gate (fail-closed)
- **M2.4** Pillar B feasibility: ApexRuntime, policy gate, AUNC coverage tracker

### Architect
- **A3.1** Capture abstraction: DXGI / BitBlt / Offline sources
- **A3.2** Route recorder + humanized `HumanizedInput` (personality-driven)
- **A3.3** Quest planner (text→goal→task queue) + hazard detector (threat levels)
- **A3.4** AUNC Control API (spec-locked §2) + ApexRuntime (Pillar B)
- **A3.5** Product facade `ApexEngine` (20 Hz loop, mode switcher) + script hub + IPC protocol + frontend panels

### Deploy
- **D4.1** Red-team preflight gate (`--preflight`)
- **D4.2** BMAD CI (build/test/audit/release), pinned toolchain, deployment doc
- **D4.3** Honest benchmark vs Synapse X + verification guide
- **D4.4** (pending) research-server measurements, signed release

---

## [0.2.0] — 2026-08-16 (workspace foundation)

- 12-crate Rust workspace (`core`, `ipc`, `vision`, `input`, `logic`, `driver`, `kernel`, `net`, `app`, `redteam`, `tests`, `frontend`)
- 8,000+ lines Rust, 160+ tests, 9 design/ops docs
- Sandboxed Luau engine (io/os/debug stripped, limits enforced)
- Process attachment module (RAII-safe Win32 wrappers)
- WebSocket IPC bridge (localhost:9090)
- Simulated kernel abstraction (research-only, `sandbox` feature)

---

## [0.1.0] — 2026-08-16 (repo cleanup)

- **Removed malware:** `Fluxus V7.exe` (compiled-PowerShell dropper: Defender suppression, Telegram→Pastebin→GitHub payload chain, UAC elevation, scheduled-task persistence) — deleted, analysis archived in research doc
- Removed fake "Fluxus" branding + fabricated feature claims
- Removed legacy C# project (WebSocket X copy, `packages.config`, empty settings)
- Honest README; `.gitignore`; SDK-style `.csproj` for the legacy path
- `docs/roblox-exploit-ecosystem.md` — full ecosystem research

---

*Generated during the Arena session. Versions 0.1.0 → 0.2.0 → v1.0 (planned).*
