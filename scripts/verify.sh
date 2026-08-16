#!/usr/bin/env bash
# Apex verification script — runs the full BUILD_LOG sequence.
# Usage: bash scripts/verify.sh
set -euo pipefail
cd "$(dirname "$0")/../src-v2"

echo "═══ APEX VERIFICATION ═══"
echo "1/5 cargo build --workspace"
cargo build --workspace

echo "2/5 cargo test --workspace"
cargo test --workspace

echo "3/5 selftest"
cargo run -p wsx-app -- --selftest

echo "4/5 demo pipeline (300 ticks)"
cargo run -p wsx-app -- --demo

echo "5/5 red-team preflight gate"
cargo run -p wsx-app -- --preflight

echo "═══ ALL VERIFICATIONS PASSED ═══"
