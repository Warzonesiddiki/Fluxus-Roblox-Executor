# Apex — Threat Model (Blueprint B1.1)

**Version:** 0.1 · **Status:** Draft for review
**Scope:** All three pillars (A external automation / B internal scripting / C sandbox engine)
**Target environment:** Windows 10/11; own research server; production-grade undetectability for Pillar A

---

## 1. Detection layers (the model)

Detection of game-automation software happens in four independent layers. A tool is only as safe as its weakest layer. We enumerate every layer, what could detect us, our countermeasure, and how we verify it.

| Layer | What detects | Detector examples | Our exposure | Countermeasure | Verification |
|---|---|---|---|---|---|
| **L0 — In-process** | Anything inside the game client process: DLLs, memory writes, hooks, threads, VM tampering | Hyperion/Byfron-style anti-tamper: page encryption, instrumentation callbacks, syscall hooks, RX-page whitelists | **Pillar A: ZERO** (never touches process). **Pillar B: FULL** (in-process by definition). Pillar C: none (no target access). | Pillar A: no process interaction, ever. Pillar B: policy-gated to own server only, never production. | Red-team sweep: process/memory/handle scan finds nothing for Pillar A |
| **L1 — System** | Unknown processes, drivers, synthetic input flags, named objects | EAC/Vanguard-style kernel modules, Windows input integrity, driver signing checks | Low: one known process, HID-level input, no drivers | Signed, known process; SendInput/interception at HID layer; no kernel components | Our own anti-cheat's system scan: 0 hits |
| **L2 — Behavioral (server-side)** | Movement patterns, click timing, session cadence, reaction speed | Server-side analytics, behavioral anti-cheat, banwaves from pattern clustering | High (this is the real risk) | Humanization 2.0: Bezier curves, lognormal latencies, per-session personality variance, AFK pauses, reaction jitter | Statistical test suite: synthetic input indistinguishable from human corpus (KS p ≥ 0.2) |
| **L3 — Network** | Packet timing/volume anomalies, TLS fingerprints, unusual endpoints | Server traffic analysis, DPI | Low: passive listening only (Pillar A); documented protocol (own server) | Receive-only taps; traffic pacing matching human session patterns; no unusual endpoints | Net-layer red-team: capture shows only expected traffic |

---

## 2. Pillar-by-pillar exposure matrix

| Threat | Pillar A (external) | Pillar B (internal, own server) | Pillar C (sandbox dev tool) |
|---|---|---|---|
| Process scan finds us | ❌ no process attach | ✅ normal (allowed on own server) | ❌ no target access |
| Memory scan finds injection | ❌ no memory R/W | ✅ possible (accepted risk) | ❌ none |
| VM hooks detected | ❌ no hooks | ✅ possible (accepted risk) | ❌ none |
| Synthetic input flag | ⚠️ mitigated by humanization | n/a (scripts do actions in-VM) | n/a |
| Behavioral pattern detection | ⚠️ mitigated by humanization 2.0 | ⚠️ mitigated by script quality | n/a |
| Network anomaly | ⚠️ mitigated by passive + pacing | ⚠️ same as normal client | n/a |
| Supply-chain malware | ❌ deterministic builds, no remote code | ❌ same | ❌ same |
| **Overall posture** | **Undetectable by design** | **Detectable-but-permitted (own server)** | **Safe, no target** |

---

## 3. Trust boundaries

```
┌─ USER ─────────────────────────────────────────────┐
│  Apex UI (Tauri) — user-supplied scripts run here  │
│  ┌──────────────────────────────────────────────┐  │
│  │ Pillar C sandbox (untrusted scripts)         │  │
│  │  - memory limits, instruction limits         │  │
│  │  - io/os/debug stripped                      │  │
│  │  - no network except IPC loopback            │  │
│  └──────────────────────────────────────────────┘  │
└──────────────────────┬─────────────────────────────┘
                       │ IPC (loopback only)
┌──────────────────────▼─────────────────────────────┐
│  Apex Engine (trusted Rust core)                   │
│  - validates every script action                   │
│  - never executes untrusted code outside sandbox   │
└──────────────────────┬─────────────────────────────┘
                       │ Pillar A: vision/packets/HID (external)
                       │ Pillar B: in-process bridge (own server only, gated)
┌──────────────────────▼─────────────────────────────┐
│  Research server (own infrastructure)              │
└────────────────────────────────────────────────────┘
```

**Key rules:**
1. Untrusted scripts (user-supplied) execute **only** in the Pillar C sandbox.
2. The engine translates sandbox actions into validated HID/network behavior (Pillar A) — never raw memory operations.
3. Pillar B bridge activation requires an explicit config flag + a target that matches our own server allowlist. Never default-on.
4. No code is ever downloaded/executed at runtime from remote endpoints (anti-supply-chain).

---

## 4. Adversary scenarios (what we defend against)

| # | Scenario | Layers hit | Defense |
|---|---|---|---|
| S1 | Anti-tamper scans process memory for injected DLLs | L0 | Pillar A never injects; Pillar B confined to own server |
| S2 | Server flags perfect farming patterns | L2 | Humanization 2.0: movement curves, latency distributions, session variance |
| S3 | Server clusters session times/behavior across accounts | L2 | Session-length variance, daily pattern rotation |
| S4 | Input system flags synthetic events | L1 | HID-level injection, human calibration, per-session personality |
| S5 | Network monitors unusual traffic from client | L3 | Passive listening only; pacing matches human play |
| S6 | Malware-laced scripts compromise host | (host safety) | Sandboxed execution + review-gated hub + hash-pinned scripts |
| S7 | User runs Pillar B against production by mistake | L0 | Policy gate: target allowlist = our server only; fails closed |

---

## 5. Acceptance criteria (how we know we're safe)

1. **Pillar A red-team sweep:** running instance + our imported anti-cheat → 0 findings (process, memory, handles, input, network).
2. **Statistical human test:** 10k+ synthetic input events vs human corpus → KS p ≥ 0.05 (target 0.2).
3. **Session longevity:** 72h continuous Pillar A run on research server, no flags, no restarts.
4. **Policy gate test:** attempt to enable Pillar B against a non-allowlisted target → hard failure.
5. **Supply-chain audit:** every release is reproducible from source; zero network calls at runtime except loopback IPC.

---

## 6. Open questions (to resolve in Blueprint)

- Q1: Should Pillar A ever *write* to the client process (even benign)? **Default: never.**
- Q2: What telemetry does Apex collect locally, and what leaves the machine? **Default: nothing leaves.**
- Q3: Pillar B on own server — does our server's imported Byfron flag in-process execution, and do we care (we own the server)? **We care for realism: it must bypass, because that's the benchmark.**
- Q4: Distribution model — closed-source binary + signed installer, or open-source? **Default: source-visible core, signed release binaries.**

---

*Next: review this model → approve → write ARCHITECTURE.md (B1.2).*
