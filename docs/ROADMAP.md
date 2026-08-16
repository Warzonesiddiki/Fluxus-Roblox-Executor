# 🎯 Project Apex — Roadmap v0.2: "Better than Synapse X, undetectable"

**Methodology:** BMAD = **Blueprint → Model → Architect → Deploy** (design-first)
**Platform:** Windows (10/11)
**Targets:** Own research server now + production-grade undetectability
**Product form:** All three pillars (see §1)
**Current state:** `src-v2/` ~3,800 lines Rust across 9 crates (foundation drafts)

---

## 1. Product architecture — three pillars, one engine

The final product has **three execution surfaces** behind a single UI and engine:

```
                        ┌──────────────────────────────────────────┐
                        │  Apex Engine (Rust core)                 │
                        │                                          │
   ┌────────────────────┼────────────────────┬─────────────────────┐
   │  PILLAR A          │  PILLAR B          │  PILLAR C           │
   │  External          │  Internal          │  Sandbox Engine     │
   │  Automation        │  Scripting         │  (Dev Tool)         │
   ├────────────────────┼────────────────────┼─────────────────────┤
   │ CV + packets + HID │ In-process Luau    │ Full Luau runtime   │
   │ Behavior tree      │ execution          │ + AUNC API          │
   │ Quest planner      │ (gated, own server │ + script hub        │
   │ Humanization       │  only by policy)   │ + debugger          │
   ├────────────────────┼────────────────────┼─────────────────────┤
   │ UNDETECTABLE       │ DETECTABLE-BY-     │ SAFE (no target     │
   │ everywhere         │ NATURE (in-process)│  access at all)     │
   └────────────────────┴────────────────────┴─────────────────────┘
```

**The honest reconciliation of "ALL 3":**
- **Pillar A** is the *default* — undetectable against any anti-cheat, including production (L0 zero-footprint + L1–L3 humanization).
- **Pillar B** gives Synapse-X-level in-game scripting, but in-process execution is *inherently* detectable. It is therefore **policy-gated**: enabled only against our own research server (where injection is allowed). It never runs against production.
- **Pillar C** is the developer surface — a full sandboxed Luau engine with a UNC-style API suite ("AUNC"), script hub, and debugger. It powers A's control scripts and B's payloads, and is 100% safe by itself.

**"Better than Synapse X" = Pillar C's engine quality + Pillar A's undetectability + Pillar B's in-game power (on our server).**

---

## 2. The four BMAD macro-phases

BMAD is our design-first loop. Each macro-phase contains milestones; each milestone ends with a review gate.

```
 ┌──────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
 │ BLUEPRINT    │ → │ MODEL        │ → │ ARCHITECT    │ → │ DEPLOY       │
 │ Design docs  │   │ Prototypes & │   │ Full impl    │   │ Package,     │
 │ threat model │   │ math models  │   │ per blueprint│   │ red-team,    │
 │ contracts    │   │ validate     │   │              │   │ release      │
 └──────────────┘   └──────────────┘   └──────────────┘   └──────────────┘
        ▲                                                            │
        └──────────────── feedback loop (gates feed Blueprint) ──────┘
```

---

## 3. PHASE 1 — BLUEPRINT (design before code)

> *Nothing ships until the design is written, reviewed, and frozen.*

### B1.1 Threat model & detection matrix (Week 1)
- **Deliverable:** `docs/THREAT_MODEL.md` — the 4-layer detection model (L0 in-process / L1 system / L2 behavioral / L3 network), with for each layer: what could detect us, how we stay clear, and how we verify.
- **Acceptance:** every pillar's detection surface is enumerated; Pillar B explicitly flagged "in-process = detectable, policy-gated."

### B1.2 System architecture blueprint (Week 2)
- **Deliverable:** `docs/ARCHITECTURE.md` — component diagram, data flow (screen→snapshot→decision→input), module contracts between the 9 crates, message schemas (IPC protocol, GameSnapshot, EntitySync), error taxonomy.
- **Acceptance:** each crate has a written interface contract that the Model phase can mock.

### B1.3 API blueprint — AUNC (Week 3)
- **Deliverable:** `docs/AUNC_SPEC.md` — the Apex UNC conformance suite spec: every control-script function (`on_tick`, `move_to`, `collect`, `pollen()`, …), every Pillar B in-game function (Synapse-X-compatible surface: `game`, `getgenv`, `hookfunction`, `Drawing`, …), signatures, semantics, edge cases.
- **Acceptance:** spec is complete enough that two developers could implement it independently and interoperate.

### B1.4 Data blueprint — research server protocol (Week 4)
- **Deliverable:** protocol map of our own server (packet types, entity sync, quest state) — we own it, so this is documentation, not reverse engineering.
- **Acceptance:** a packet→GameSnapshot field mapping table.

**Gate B:** blueprint docs reviewed; contracts frozen; CRs to `src-v2/` frozen until Model validates.

---

## 4. PHASE 2 — MODEL (validate the hard math before building)

> *Prototype the risky parts, measure them, then commit to the architecture.*

### M2.1 Humanization statistical model (Week 5–6)
- **Goal:** prove input can be made statistically indistinguishable from human.
- **Prototype:** record a human input corpus (≥50k events: clicks, movement, typing); implement Bezier + lognormal-latency + jitter prototype; run **Kolmogorov–Smirnov** + curvature analysis + inter-event-interval tests.
- **Acceptance:** p ≥ 0.05 (target p ≥ 0.2) between synthetic and human distributions. **This is the single biggest risk — de-risk first.**

### M2.2 State-fusion model (Week 6–7)
- **Goal:** prove vision+packets fusion is robust.
- **Prototype:** Kalman-style fusion of CV detections + packet positions; simulate occlusions, frame drops, aliasing; measure accuracy on 1,000 labeled frames from our server.
- **Acceptance:** ≥99% state accuracy, <50ms p95 latency, graceful confidence degradation.

### M2.3 Detection-surface model (Week 7)
- **Goal:** prove Pillar A leaves zero detectable surface.
- **Prototype:** a detection-harness that scans process list, memory maps, handles, window messages, input queues, and network behavior of a running Pillar A instance. 
- **Acceptance:** 0 findings across all layers (including our own imported anti-cheat).

### M2.4 Pillar B feasibility model (Week 8)
- **Goal:** prove in-process execution is achievable on our server with our engine.
- **Prototype:** wsx-core process module against a test client on the research server; measure attach time, stability (crash rate), AUNC subset coverage.
- **Acceptance:** attach < 100ms, crash rate < 1%, 80% AUNC subset working.

**Gate M:** models validated with numbers; architecture adjusted where models failed; Blueprint revised and re-frozen.

---

## 5. PHASE 3 — ARCHITECT (full implementation, blueprint-locked)

### A3.1 Sensing layer (Weeks 9–12) — Pillar A core
- ✅ DXGI Desktop Duplication capture (60fps) + BitBlt fallback (capture.rs: CaptureSource trait) + OfflineSource for deterministic replay
- HSV token detector (exists) + template matching + Tesseract OCR
- Packet parser against our protocol (wsx-net expand)
- Fusion engine producing confidence-scored `GameSnapshot`
- **Acceptance:** M2.2 numbers reproduced in production code

### A3.2 Acting layer (Weeks 12–15) — Pillar A core
- ✅ Route recorder + humanized HumanizedInput (personality-driven latency/Bezier)
- Humanization 2.0 from M2.1 model (parameterized per-session personality)
- Macro route recorder + visual-checkpoint replay (driver crate expand)
- **Acceptance:** M2.1 numbers reproduced; route replay ≥99%

### A3.3 Intelligence layer (Weeks 15–18) — Pillar A core
- ✅ Quest planner (text→goal→task queue) + hazard detector (threat levels)
- Hazard detection (monster/aggression via vision) + evasive routing
- Scheduler (exists) + session variance + failure recovery
- **Acceptance:** 24h unattended run, ≥95% uptime, ≥90% quest completion

### A3.4 Engine layer (Weeks 18–24) — Pillars B + C
- ✅ AUNC Control API surface (spec-locked §2) + conformance tracking
- Implement in-process mode (Pillar B) against our server: attach, hook VM, loadstring/pcall bridge, AUNC in-game API
- Decompiler + debugger tools (Synapse-X-class)
- **Acceptance:** 100% AUNC on control scripts; Pillar B: 100% AUNC in-game on our server, <100ms attach

### A3.5 Product layer (Weeks 24–28) — everything
- Tauri UI: editor (Monaco), feature matrix, live console, snapshot inspector, route editor, mode switcher (A/B/C)
- Script hub (curated, hash-pinned, review-gated)
- ✅ **Engine facade (ApexEngine)** — product surface for UI/IPC
  - ✅ Script hub (curated, hash-pinned control + in-game scripts)
  - ✅ IPC protocol extended (get_status, set_mode, hub_list, run_hub_script, run_script)
  - ✅ Frontend: mode switcher, live status panel, script hub panel
  - **Acceptance:** full product usable end-to-end from a fresh install (UI wiring final polish pending)

**Gate A:** all milestones pass; docs updated to match reality; no shortcuts.

---

## 6. PHASE 4 — DEPLOY (package, red-team, release, loop)

### D4.1 Red-team sweep (Week 29) ✅
- Automated + manual detection attempt against running Pillar A on the research server: process/memory/input/network scans + our imported anti-cheat.
- **Acceptance:** 0 findings. Any finding → back to Blueprint (that's the loop).

### D4.2 Packaging & distribution (Week 30) ✅
- Signed installer (MSI), reproducible builds (hash-pinned), Windows Defender exclusions documented (not requested — we avoid flagged behavior entirely).
- **Acceptance:** clean install on fresh Win10/11 VM.

### D4.3 Documentation & benchmark (Week 31) ✅
- Architecture docs, AUNC reference, quick-start, security model
- **Honest benchmark table** vs Synapse X (kept from v0.1 roadmap)
- **Acceptance:** every public API documented; benchmark reproducible

### D4.4 v1.0 release (Week 32) 🟡 — prepared, blocked on server measurements
- **Deliver:** v1.0 with all three pillars
- **Loop:** BMAD restarts — Blueprint v2 from deployment telemetry

---

## 7. Honest benchmark: Apex vs Synapse X (target)

| Capability | Synapse X (2023, dead) | Apex v1.0 (target) |
|---|---|---|
| In-game VM scripting (our server) | ✅ Lv 7–8 | ✅ Pillar B (policy-gated) |
| In-game scripting (production) | ✅ (banned in waves) | ❌ **by design** — Pillar A instead |
| UNC/sUNC API coverage | 100% | 100% AUNC (Pillar B/C) |
| **Undetectability L0** | ❌ | ✅ Zero footprint (Pillar A) |
| **Undetectability L1–L3** | ❌ | ✅ Statistical human |
| Banwave survival | ❌ | ✅ No ban surface |
| Automation depth | Via scripts | ✅ Native BT + quest planner |
| Host safety | ❌ poisoned supply chain | ✅ deterministic, no remote code |
| Stability | Good | ≥95% uptime 24h target |

**The win condition:** Pillar A's undetectability + Pillar B's power (where legal) + Pillar C's engineering quality. We deliberately concede in-game scripting against production — that's what makes us undetectable there.

---

## 8. Immediate next steps

1. ✅ BMAD confirmed: **Blueprint → Model → Architect → Deploy**
2. **Start B1.1:** write `docs/THREAT_MODEL.md` (first blueprint deliverable)
3. **Start B1.4:** map our server's protocol (fast win — we own it)
4. **M2.1 de-risk:** record human input corpus; prototype humanization model
5. Update `src-v2/` crate statuses against the new phase map

---

*Roadmap v0.2 — revised for BMAD (Blueprint-first) and all three product pillars. Next revision after B1.1 threat model is written.*
