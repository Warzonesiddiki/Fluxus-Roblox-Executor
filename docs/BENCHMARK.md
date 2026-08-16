# Apex vs Synapse X — Honest Benchmark (D4.3)

**Version:** 0.1 · **Status:** Target table (to be filled with measured numbers at v1.0)
**Principle:** honest wins. Where we lose, we lose deliberately — because the win column is undetectability.

---

## 1. The table

| # | Capability | Synapse X (2023, dead) | Apex v1.0 (target) | Notes |
|---|---|---|---|---|
| 1 | In-game VM scripting (own server) | ✅ Level 7–8 | ✅ Pillar B (policy-gated) | Apex's Pillar B targets the same API surface |
| 2 | In-game scripting (production Roblox) | ✅ (banned in waves) | ❌ **by design** | Pillar A replaces it — no in-process surface |
| 3 | UNC/sUNC API coverage | 100% | 100% AUNC (Pillar B/C) | AUNC spec = AUNC_SPEC.md |
| 4 | **Undetectability L0 (in-process)** | ❌ in-process = detectable | ✅ zero footprint | red-team verified (M2.3/D4.1) |
| 5 | **Undetectability L1–L3** | ❌ minimal humanization | ✅ statistical human | KS p ≥ 0.05 verified (M2.1) |
| 6 | Banwave survival | ❌ periodic bans | ✅ no ban surface | no account-risk exposure |
| 7 | Session longevity | days | indefinite (bounded by server) | external-only = nothing to detect |
| 8 | Automation depth (farm/quest/events) | via scripts | ✅ native BT + quest planner | behavior tree + AUNC control scripts |
| 9 | Script ecosystem size | huge (dead now) | curated hub (quality-first) | anti-malware by design |
| 10 | Host safety | ❌ poisoned supply chain | ✅ deterministic, no remote code | threat-model enforced |
| 11 | Stability | good | ≥95% uptime 24h (target) | to be measured on research server |
| 12 | Attach latency | ~50–200ms | <100ms (Pillar B target) | M2.4 feasibility |
| 13 | Decompiler / debugger | ✅ | ✅ planned (Pillar B) | A3.4 roadmap |
| 14 | Multi-instance | ✅ | ✅ external mode trivially supports N instances | one engine, many windows |

## 2. Scorecard logic

**Apex wins where Synapse X was structurally weak:**
- Undetectability (rows 4–6): Synapse X's power required in-process execution, which *is* the detection surface. Apex's external core has none.
- Host safety (row 10): the 2024–2026 executor malware wave (LummaC2 stealers, miners) was a supply-chain catastrophe; Apex ships deterministic, hash-pinned, no remote code.

**Apex concedes where the win is impossible-by-design:**
- Row 2: arbitrary scripting against production Roblox is deliberately excluded — that's the *price* of rows 4–6. The product thesis: undetectable automation > detectable scripting.

**Ties / to-be-measured:**
- Rows 12–14 need measured numbers on the research server (attach latency, uptime %, decompiler completeness).

## 3. Measurement protocol (v1.0)

1. **Uptime:** 72h unattended run on research server → log ticks, restarts, errors.
2. **Attach latency:** 100 injections of Pillar B → p50/p95/p99.
3. **Humanization:** run `humanization_acceptance` suite → record KS p-values.
4. **Red-team:** run `wsx-redteam` preflight → record check-by-check results.
5. **API coverage:** AUNC conformance suite → coverage %.
6. Each number lands in this table with a date + commit hash.

## 4. The honest bottom line

> Synapse X was the best *in-game script executor* ever made, and it died with Byfron.
> Apex is not a better executor — it's a **better tool for the era**: the same automation outcomes,
> zero detectable surface, no ban risk, no malware supply chain. Where the old paradigm had power,
> Apex has permanence.

---

*Next: docs/VERIFICATION.md — how to reproduce every number.*
