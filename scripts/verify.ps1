# Apex verification script (PowerShell) — runs the full BUILD_LOG sequence.
# Usage: powershell -File scripts/verify.ps1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\src-v2")

Write-Host "=== APEX VERIFICATION ==="
Write-Host "1/5 cargo build --workspace"
cargo build --workspace

Write-Host "2/5 cargo test --workspace"
cargo test --workspace

Write-Host "3/5 selftest"
cargo run -p wsx-app -- --selftest

Write-Host "4/5 demo pipeline (300 ticks)"
cargo run -p wsx-app -- --demo

Write-Host "5/5 red-team preflight gate"
cargo run -p wsx-app -- --preflight

Write-Host "=== ALL VERIFICATIONS PASSED ==="
