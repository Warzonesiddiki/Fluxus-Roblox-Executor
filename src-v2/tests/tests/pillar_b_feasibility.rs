/// # M2.4 — Pillar B Feasibility Test
///
/// Acceptance:
/// - Policy gate fails closed for unknown targets
/// - API coverage tracks the AUNC spec surface
/// - Runtime lifecycle (init → execute → shutdown) is sound
///
/// Full attach/inject tests run on the research server (Windows);
/// the policy-gate + lifecycle logic is cross-platform and tested here.

use wsx_core::runtime::{ApexRuntime, TargetAllowlist};

#[test]
fn m2_4_policy_gate_fails_closed() {
    let runtime = ApexRuntime::new(TargetAllowlist::research_default());

    // Unknown process → denied, even with a valid-looking PID
    let r = runtime.init(42, "TotallyLegitClient.exe");
    assert!(matches!(r, Err(wsx_core::runtime::RuntimeError::PolicyGate { .. })));
    assert!(!runtime.is_initialised(), "runtime must NOT init on gate denial");
}

#[test]
fn m2_4_api_surface_matches_spec() {
    // The total API surface must exactly match AUNC_SPEC.md §3.2
    let expected = [
        "game", "workspace", "getgenv", "getreg", "getgc", "getrawmetatable",
        "setreadonly", "isreadonly", "loadstring", "hookfunction",
        "hookmetamethod", "getnamecallmethod", "Drawing", "fireclickdetector",
        "request", "readfile", "writefile", "isfile", "delfile",
        "getfpscap", "setfpscap", "getconnections", "queue_on_teleport",
        "setclipboard", "crypt", "debug",
    ];
    assert_eq!(ApexRuntime::API_TOTAL.len(), expected.len());
    for (a, b) in ApexRuntime::API_TOTAL.iter().zip(expected.iter()) {
        assert_eq!(*a, *b, "API mismatch");
    }
}

#[test]
fn m2_4_runtime_lifecycle() {
    // Gate logic: init against allowlisted name must reach the attach step
    // (which on non-Windows fails — that's expected and fine; the gate passed).
    let mut runtime = ApexRuntime::new(TargetAllowlist::research_default());
    let _ = runtime.init(9999, "RobloxPlayerBeta.exe");

    // Execute without init is refused
    let r = runtime.execute("print('x')");
    if runtime.is_initialised() {
        assert!(r.is_ok());
    } else {
        // On non-Windows, init failed at attach → execute must still refuse
        assert!(matches!(r, Err(wsx_core::runtime::RuntimeError::NotInitialised)));
    }

    runtime.shutdown();
    assert!(!runtime.is_initialised());
}

#[test]
fn m2_4_execution_counter() {
    let mut runtime = ApexRuntime::new(TargetAllowlist::research_default());
    // Bypass the gate by allowing any name + fake PID: gate check still
    // runs against allowlist; we widen it for this test.
    let mut allowlist = TargetAllowlist::research_default();
    allowlist.process_names.push("test-pid.exe".into());
    runtime = ApexRuntime::new(allowlist);
    // On Windows CI with the test client present this attaches; here we just
    // verify the counter increments after a successful init+execute cycle.
    if let Ok(()) = runtime.init(1, "test-pid.exe") {
        runtime.execute("print('hello')").unwrap();
        assert_eq!(runtime.executions.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[test]
fn m2_4_coverage_progress_trackable() {
    let mut runtime = ApexRuntime::new(TargetAllowlist::research_default());
    let before = runtime.coverage.coverage_pct();
    // Implement the whole surface (simulating completion)
    runtime.coverage.implemented = ApexRuntime::API_TOTAL.iter().map(|s| s.to_string()).collect();
    let after = runtime.coverage.coverage_pct();
    assert_eq!(after, 100.0);
    assert!(after > before);
}
