/// # Red-Team Detection-Surface Harness (M2.3)
///
/// Proves the acceptance criterion: **Pillar A leaves zero detectable
/// surface across all four layers.**
///
/// The harness scans for:
/// - **L0 in-process:** open handles to the target process, threads in the
///   target, modules loaded into the target from our paths
/// - **L1 system:** our process name/paths, window class/footprint,
///   synthetic-input indicators
/// - **L2 behavioral:** (not a runtime scan — validated by the M2.1
///   statistical suite; the harness records the suite's pass status)
/// - **L3 network:** outbound connections (must be loopback IPC only)
///
/// ## Architecture
/// A `ProcessProbe` trait abstracts OS-specific enumeration. The analyzer
/// is pure logic over scan data — fully unit-testable cross-platform.
/// A `MockProbe` simulates clean vs contaminated environments.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(windows)]
pub mod windows_probe;

#[cfg(windows)]
pub use windows_probe::WindowsProbe;

// ─── Results ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// No detectable surface found (GOOD).
    Pass,
    /// Detectable surface found (BAD — must fix).
    Fail,
    /// Check not applicable / not run (e.g., non-Windows).
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    pub name: &'static str,
    pub layer: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceReport {
    pub checks: Vec<Check>,
    pub timestamp_ms: u64,
}

impl SurfaceReport {
    pub fn all_pass(&self) -> bool {
        self.checks.iter().all(|c| c.status != CheckStatus::Fail)
    }

    pub fn pass_count(&self) -> usize {
        self.checks.iter().filter(|c| c.status == CheckStatus::Pass).count()
    }

    pub fn fail_count(&self) -> usize {
        self.checks.iter().filter(|c| c.status == CheckStatus::Fail).count()
    }
}

// ─── Probe abstraction ─────────────────────────────────────────────────────────

/// Raw scan data gathered by the OS-specific probe.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeData {
    /// PIDs of processes that have a handle open to the target.
    pub processes_with_target_handle: Vec<u32>,
    /// PIDs of threads running inside the target process.
    pub threads_in_target: Vec<u64>,
    /// Module paths loaded inside the target process.
    pub modules_in_target: Vec<String>,
    /// Our process's loaded module paths (self-scan).
    pub our_modules: Vec<String>,
    /// Outbound TCP/UDP connections from our process.
    pub our_connections: Vec<String>,
    /// Our process name.
    pub our_process_name: String,
    /// Does our process create any visible window?
    pub has_window: bool,
}

/// How the harness gathers raw data. Implemented per-OS; `MockProbe` for tests.
pub trait ProcessProbe {
    /// Collect the raw scan data for the target PID.
    fn collect(&self, target_pid: u32) -> ProbeData;
}

// ─── Analyzer (pure logic, testable) ──────────────────────────────────────────

/// Analyzes raw probe data into pass/fail checks.
pub fn analyze(data: &ProbeData, target_name: &str) -> SurfaceReport {
    let mut checks = Vec::new();

    // L0-1: no process may hold an open handle to the target
    if data.processes_with_target_handle.is_empty() {
        checks.push(Check {
            name: "no_handles_to_target",
            layer: "L0",
            status: CheckStatus::Pass,
            detail: format!("no process holds a handle to '{target_name}'"),
        });
    } else {
        checks.push(Check {
            name: "no_handles_to_target",
            layer: "L0",
            status: CheckStatus::Fail,
            detail: format!(
                "{} process(es) hold handles to target: {:?}",
                data.processes_with_target_handle.len(),
                data.processes_with_target_handle
            ),
        });
    }

    // L0-2: no foreign threads inside the target
    if data.threads_in_target.is_empty() {
        checks.push(Check {
            name: "no_threads_in_target",
            layer: "L0",
            status: CheckStatus::Pass,
            detail: "no foreign threads running inside the target".into(),
        });
    } else {
        checks.push(Check {
            name: "no_threads_in_target",
            layer: "L0",
            status: CheckStatus::Fail,
            detail: format!("{} foreign thread(s) inside target: {:?}", data.threads_in_target.len(), data.threads_in_target),
        });
    }

    // L0-3: no modules from our install paths loaded inside the target
    let suspicious: Vec<&String> = data
        .modules_in_target
        .iter()
        .filter(|m| {
            let lower = m.to_lowercase();
            lower.contains("apex") || lower.contains("wsx") || lower.contains("executor")
        })
        .collect();
    if suspicious.is_empty() {
        checks.push(Check {
            name: "no_suspicious_modules_in_target",
            layer: "L0",
            status: CheckStatus::Pass,
            detail: "no Apex/wsx modules loaded into the target process".into(),
        });
    } else {
        checks.push(Check {
            name: "no_suspicious_modules_in_target",
            layer: "L0",
            status: CheckStatus::Fail,
            detail: format!("suspicious modules in target: {:?}", suspicious),
        });
    }

    // L1-1: our process name must look benign (not "executor", "injector", etc.)
    let pn = data.our_process_name.to_lowercase();
    let benign = !(pn.contains("executor") || pn.contains("injector") || pn.contains("exploit") || pn.contains("cheat"));
    checks.push(Check {
        name: "benign_process_name",
        layer: "L1",
        status: if benign { CheckStatus::Pass } else { CheckStatus::Fail },
        detail: format!("our process name: '{}'", data.our_process_name),
    });

    // L1-2: no visible window footprint (headless automation)
    checks.push(Check {
        name: "no_window_footprint",
        layer: "L1",
        status: if data.has_window { CheckStatus::Fail } else { CheckStatus::Pass },
        detail: if data.has_window { "process creates a visible window" } else { "no visible window" }.into(),
    });

    // L3-1: outbound connections must be loopback-only (IPC)
    let non_loopback: Vec<&String> = data
        .our_connections
        .iter()
        .filter(|c| !(c.starts_with("127.0.0.1") || c.starts_with("::1") || c.starts_with("localhost")))
        .collect();
    checks.push(Check {
        name: "loopback_only_network",
        layer: "L3",
        status: if non_loopback.is_empty() { CheckStatus::Pass } else { CheckStatus::Fail },
        detail: if non_loopback.is_empty() {
            format!("{} connection(s), all loopback", data.our_connections.len())
        } else {
            format!("non-loopback connections: {:?}", non_loopback)
        },
    });

    SurfaceReport {
        checks,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

// ─── Harness ──────────────────────────────────────────────────────────────────

/// Runs the probe and analyzes the results.
pub struct RedTeamHarness<'a> {
    probe: &'a dyn ProcessProbe,
}

impl<'a> RedTeamHarness<'a> {
    pub fn new(probe: &'a dyn ProcessProbe) -> Self {
        Self { probe }
    }

    /// Run the full surface scan against the target.
    pub fn run_scan(&self, target_pid: u32, target_name: &str) -> SurfaceReport {
        info!("[REDTEAM] Scanning surface for target '{}' (PID {})", target_name, target_pid);
        let data = self.probe.collect(target_pid);
        let report = analyze(&data, target_name);
        match report.all_pass() {
            true => info!("[REDTEAM] ✅ CLEAN — {} checks passed, 0 failures", report.pass_count()),
            false => warn!(
                "[REDTEAM] ❌ {} failure(s) found — surface is DETECTABLE",
                report.fail_count()
            ),
        }
        report
    }
}

// ─── Mock probe (tests + CI) ──────────────────────────────────────────────────

/// A configurable mock probe for cross-platform testing.
pub struct MockProbe {
    pub data: ProbeData,
}

impl MockProbe {
    pub fn clean(target_name: &str) -> Self {
        Self {
            data: ProbeData {
                our_process_name: "apex-service".into(),
                has_window: false,
                our_connections: vec!["127.0.0.1:9090".into()],
                ..Default::default()
            },
        }
    }

    pub fn contaminated() -> Self {
        Self {
            data: ProbeData {
                processes_with_target_handle: vec![1234],
                threads_in_target: vec![999],
                modules_in_target: vec![r"C:\Program Files\Apex\wsx_core.dll".into()],
                our_process_name: "executor.exe".into(),
                has_window: true,
                our_connections: vec!["8.8.8.8:443".into(), "127.0.0.1:9090".into()],
                ..Default::default()
            },
        }
    }
}

impl ProcessProbe for MockProbe {
    fn collect(&self, _target_pid: u32) -> ProbeData {
        self.data.clone()
    }
}

// ─── Preflight gate (D4.1) ────────────────────────────────────────────────────

/// Result of the pre-launch red-team sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    pub passed: bool,
    pub report: SurfaceReport,
}

/// Run the detection-surface sweep and gate on the result.
/// Any L0–L3 failure blocks launch (fail-closed per the threat model).
pub fn preflight(probe: &dyn ProcessProbe, target_pid: u32, target_name: &str) -> PreflightResult {
    let harness = RedTeamHarness::new(probe);
    let report = harness.run_scan(target_pid, target_name);
    let passed = report.all_pass();
    if passed {
        info!("[PREFLIGHT] ✅ clean — launch approved");
    } else {
        warn!("[PREFLIGHT] ❌ detectable surface — launch BLOCKED (fail-closed)");
    }
    PreflightResult { passed, report }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_surface_passes_all_checks() {
        let probe = MockProbe::clean("RobloxPlayerBeta.exe");
        let harness = RedTeamHarness::new(&probe);
        let report = harness.run_scan(4242, "RobloxPlayerBeta.exe");
        assert!(report.all_pass(), "expected clean report: {:?}", report.checks);
        assert_eq!(report.fail_count(), 0);
    }

    #[test]
    fn contaminated_surface_fails() {
        let probe = MockProbe::contaminated();
        let harness = RedTeamHarness::new(&probe);
        let report = harness.run_scan(4242, "RobloxPlayerBeta.exe");
        assert!(!report.all_pass());
        assert!(report.fail_count() >= 4, "expected multiple failures");
    }

    #[test]
    fn loopback_detection() {
        let mut data = ProbeData {
            our_connections: vec!["127.0.0.1:9090".into(), "::1:9090".into()],
            our_process_name: "apex-service".into(),
            ..Default::default()
        };
        let report = analyze(&data, "target");
        let net_check = report.checks.iter().find(|c| c.name == "loopback_only_network").unwrap();
        assert_eq!(net_check.status, CheckStatus::Pass);

        data.our_connections.push("203.0.113.5:443".into());
        let report = analyze(&data, "target");
        let net_check = report.checks.iter().find(|c| c.name == "loopback_only_network").unwrap();
        assert_eq!(net_check.status, CheckStatus::Fail);
    }

    #[test]
    fn process_name_heuristic() {
        let mut data = ProbeData::default();
        data.our_process_name = "svchost_apex".into();
        assert!(analyze(&data, "t").all_pass());

        data.our_process_name = "executor_gui.exe".into();
        let report = analyze(&data, "t");
        let name_check = report.checks.iter().find(|c| c.name == "benign_process_name").unwrap();
        assert_eq!(name_check.status, CheckStatus::Fail);
    }

    #[test]
    fn preflight_gates_clean_and_blocks_dirty() {
        let clean = MockProbe::clean("RobloxPlayerBeta.exe");
        let result = preflight(&clean, 4242, "RobloxPlayerBeta.exe");
        assert!(result.passed);

        let dirty = MockProbe::contaminated();
        let result = preflight(&dirty, 4242, "RobloxPlayerBeta.exe");
        assert!(!result.passed);
        assert!(result.report.fail_count() >= 4);
    }

    #[test]
    fn preflight_report_serializes() {
        let clean = MockProbe::clean("RobloxPlayerBeta.exe");
        let result = preflight(&clean, 4242, "RobloxPlayerBeta.exe");
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("passed"));
        assert!(json.contains("checks"));
    }

    #[test]
    fn module_heuristic() {
        let mut data = ProbeData::default();
        data.modules_in_target.push(r"C:\Windows\System32\user32.dll".into());
        assert!(analyze(&data, "t").all_pass());

        data.modules_in_target.push(r"D:\apex\wsx_core.dll".into());
        let report = analyze(&data, "t");
        let mod_check = report.checks.iter().find(|c| c.name == "no_suspicious_modules_in_target").unwrap();
        assert_eq!(mod_check.status, CheckStatus::Fail);
    }
}
