# Apex — Deployment & Packaging (D4.2)

**Goal:** reproducible, signed, hash-pinned release builds — the anti-supply-chain posture from the threat model.

---

## 1. Build reproducibility

- Toolchain pinned in `src-v2/rust-toolchain.toml` (1.83.0) → identical compiler everywhere.
- `cargo build --release --locked` → Cargo.lock pins exact dependency versions.
- CI builds are tagged releases only (`refs/tags/v*`).

```bash
cd src-v2
rustup toolchain install 1.83.0
cargo build --release --locked
```

## 2. CI pipeline (BMAD)

`.github/workflows/ci.yml` runs on every push/PR:

| Job | BMAD phase | What it enforces |
|---|---|---|
| `build` | Build | workspace compiles on Ubuntu + Windows, clippy `-D warnings`, fmt check |
| `test` | Measure | `cargo test --workspace` (161 tests), app `--selftest`, red-team `--preflight` mock gate |
| `audit` | Adjust | cargo-audit (CVEs), cargo-deny (licenses + banned crates) |
| `release` | Deliver | tag-only Windows release artifact + `SHA256SUMS` |

## 3. Packaging steps (Windows release)

1. `cargo build --release --locked`
2. Collect: `wsx-app.exe` + `wsx-frontend` resources (Tauri bundle if UI included)
3. Generate checksums: `sha256sum * > SHA256SUMS`
4. Sign with a code-signing cert (EV recommended): `signtool sign /fd SHA256 wsx-app.exe`
5. Installer (optional): NSIS script or MSI via WiX — bundles the exe + manifest
6. Upload release + SHA256SUMS to the release page

## 4. Runtime integrity guarantees

- **No auto-updater** — updates ship as new releases, user-verified via SHA256SUMS.
- **No remote code** — the engine makes zero outbound connections (red-team L3 check enforces loopback-only).
- **No telemetry leaves the machine** — local logs only.
- **Pillar B gating** — in-game mode refuses non-allowlisted targets (fails closed).

## 5. Release checklist (v1.0)

- [ ] `cargo test --workspace` green (154+ tests)
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo audit` no high-severity CVEs
- [ ] `cargo run -p wsx-app -- --selftest` passes
- [ ] `cargo run -p wsx-app -- --preflight` passes on research server
- [ ] Red-team sweep vs live Pillar A: 0 findings (L0–L3)
- [ ] Artifacts signed + SHA256SUMS published
- [ ] Docs: ARCHITECTURE / AUNC_SPEC / BENCHMARK current
- [ ] Tag `v1.0.0` → CI release job produces artifacts

---

*Next: docs/BENCHMARK.md (D4.3) — honest Apex vs Synapse X comparison.*
