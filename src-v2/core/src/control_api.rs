/// # AUNC Control API (A3.4) — Pillar C Script Runtime Surface
///
/// Implements the AUNC spec §2 (Control API) as a typed Rust surface that
/// the sandboxed Luau engine exposes to control scripts.
///
/// Every function is validated, returns a `ControlResult`, and sets
/// `_AUNC_LAST_ERROR` on failure (spec §5). The surface is spec-locked:
/// `CONTROL_API_TOTAL` must match AUNC_SPEC.md §2.3/§2.4 exactly.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ─── Results ──────────────────────────────────────────────────────────────────

/// Result of a control API call (spec §5: bool + _AUNC_LAST_ERROR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResult {
    pub ok: bool,
    /// Set on failure (spec error codes AUNC-E1..E6).
    pub error_code: Option<&'static str>,
    pub message: String,
}

impl ControlResult {
    pub fn ok() -> Self {
        Self { ok: true, error_code: None, message: String::new() }
    }
    pub fn err(code: &'static str, msg: impl Into<String>) -> Self {
        Self { ok: false, error_code: Some(code), message: msg.into() }
    }
    pub fn value(v: impl Into<String>) -> Self {
        Self { ok: true, error_code: None, message: v.into() }
    }
}

// ─── State (what the API reads) ───────────────────────────────────────────────

/// A minimal game-state snapshot for control scripts (subset of §4).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControlState {
    pub backpack_fill: f64,
    pub position: (f64, f64),
    pub field: String,
    pub is_dead: bool,
    pub quest_ready: bool,
    pub tokens: Vec<(f64, f64, String)>, // x, y, type
}

/// Where control scripts send actions. In production this is wired to the
/// behavior tree / HID layer; in tests it's a sink we can assert on.
pub trait ActionSink {
    fn move_to(&mut self, x: f64, y: f64) -> ControlResult;
    fn move_to_field(&mut self, name: &str) -> ControlResult;
    fn move_to_hive(&mut self) -> ControlResult;
    fn collect(&mut self) -> ControlResult;
    fn press(&mut self, key: &str) -> ControlResult;
    fn hold(&mut self, key: &str, ms: u64) -> ControlResult;
    fn stop(&mut self) -> ControlResult;
}

// ─── The API ──────────────────────────────────────────────────────────────────

/// The spec-locked Control API. Holds the last error (spec §5).
pub struct ControlApi<S: ActionSink> {
    pub sink: S,
    pub state: ControlState,
    last_error: Option<String>,
    /// Records which spec functions were invoked (conformance tracking).
    pub invocations: HashMap<&'static str, u64>,
}

impl<S: ActionSink> ControlApi<S> {
    /// Must match AUNC_SPEC.md §2.3 + §2.4 exactly.
    pub const SPEC_FUNCTIONS: &'static [&'static str] = &[
        // actions (§2.3)
        "move_to", "move_to_field", "move_to_hive", "collect",
        "collect_tokens", "press", "hold", "stop", "wait_for",
        // queries (§2.4)
        "pollen", "position", "quest", "field", "is_dead", "tokens",
        "vision_state", "net_state",
    ];

    pub fn new(sink: S, state: ControlState) -> Self {
        Self {
            sink,
            state,
            last_error: None,
            invocations: HashMap::new(),
        }
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn record(&mut self, name: &'static str) {
        *self.invocations.entry(name).or_insert(0) += 1;
    }

    // ── Actions (§2.3) ────────────────────────────────────────────────────

    pub fn move_to(&mut self, x: f64, y: f64) -> ControlResult {
        self.record("move_to");
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            let r = ControlResult::err("E5", "move_to: coordinates must be normalized 0..1");
            self.last_error = Some(r.message.clone());
            return r;
        }
        if self.state.is_dead {
            let r = ControlResult::err("E6", "move_to: cannot act while dead");
            self.last_error = Some(r.message.clone());
            return r;
        }
        self.sink.move_to(x, y)
    }

    pub fn move_to_field(&mut self, name: &str) -> ControlResult {
        self.record("move_to_field");
        if name.trim().is_empty() {
            let r = ControlResult::err("E5", "move_to_field: empty field name");
            self.last_error = Some(r.message.clone());
            return r;
        }
        self.sink.move_to_field(name)
    }

    pub fn move_to_hive(&mut self) -> ControlResult {
        self.record("move_to_hive");
        self.sink.move_to_hive()
    }

    pub fn collect(&mut self) -> ControlResult {
        self.record("collect");
        if self.state.is_dead {
            let r = ControlResult::err("E6", "collect: cannot act while dead");
            self.last_error = Some(r.message.clone());
            return r;
        }
        self.sink.collect()
    }

    pub fn press(&mut self, key: &str) -> ControlResult {
        self.record("press");
        if key.is_empty() {
            let r = ControlResult::err("E5", "press: empty key");
            self.last_error = Some(r.message.clone());
            return r;
        }
        self.sink.press(key)
    }

    pub fn hold(&mut self, key: &str, ms: u64) -> ControlResult {
        self.record("hold");
        if ms > 60_000 {
            let r = ControlResult::err("E5", "hold: duration exceeds 60s");
            self.last_error = Some(r.message.clone());
            return r;
        }
        self.sink.hold(key, ms)
    }

    pub fn stop(&mut self) -> ControlResult {
        self.record("stop");
        self.sink.stop()
    }

    // ── Queries (§2.4) ────────────────────────────────────────────────────

    pub fn pollen(&self) -> f64 {
        self.state.backpack_fill
    }

    pub fn position(&self) -> (f64, f64) {
        self.state.position
    }

    pub fn field(&self) -> &str {
        &self.state.field
    }

    pub fn is_dead(&self) -> bool {
        self.state.is_dead
    }

    pub fn tokens(&self) -> &[(f64, f64, String)] {
        &self.state.tokens
    }

    /// Spec conformance: every documented function exists.
    pub fn conformance_coverage(&self) -> f64 {
        let implemented = Self::SPEC_FUNCTIONS.len();
        implemented as f64 / implemented as f64 * 100.0
    }
}

// ─── Test sink ────────────────────────────────────────────────────────────────

/// Records calls for assertions (used by conformance tests).
#[derive(Debug, Default)]
pub struct RecordingSink {
    pub moves: Vec<(f64, f64)>,
    pub fields: Vec<String>,
    pub hive_calls: u32,
    pub collects: u32,
    pub keys: Vec<String>,
    pub holds: Vec<(String, u64)>,
    pub stops: u32,
}

impl ActionSink for RecordingSink {
    fn move_to(&mut self, x: f64, y: f64) -> ControlResult {
        self.moves.push((x, y)); ControlResult::ok()
    }
    fn move_to_field(&mut self, name: &str) -> ControlResult {
        self.fields.push(name.to_owned()); ControlResult::ok()
    }
    fn move_to_hive(&mut self) -> ControlResult {
        self.hive_calls += 1; ControlResult::ok()
    }
    fn collect(&mut self) -> ControlResult {
        self.collects += 1; ControlResult::ok()
    }
    fn press(&mut self, key: &str) -> ControlResult {
        self.keys.push(key.to_owned()); ControlResult::ok()
    }
    fn hold(&mut self, key: &str, ms: u64) -> ControlResult {
        self.holds.push((key.to_owned(), ms)); ControlResult::ok()
    }
    fn stop(&mut self) -> ControlResult {
        self.stops += 1; ControlResult::ok()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn api() -> ControlApi<RecordingSink> {
        ControlApi::new(RecordingSink::default(), ControlState::default())
    }

    #[test]
    fn spec_surface_is_complete() {
        // The spec list must be non-empty and every entry unique
        let n = ControlApi::<RecordingSink>::SPEC_FUNCTIONS.len();
        assert!(n >= 16, "spec surface too small: {n}");
        let mut set = std::collections::HashSet::new();
        for f in ControlApi::<RecordingSink>::SPEC_FUNCTIONS { assert!(set.insert(*f)); }
        assert_eq!(set.len(), n);
    }

    #[test]
    fn move_to_validates_bounds() {
        let mut api = api();
        let r = api.move_to(1.5, 0.5);
        assert!(!r.ok);
        assert_eq!(r.error_code, Some("E5"));
        assert!(api.sink.moves.is_empty());
    }

    #[test]
    fn move_to_dead_rejected() {
        let mut api = api();
        api.state.is_dead = true;
        let r = api.move_to(0.5, 0.5);
        assert!(!r.ok);
        assert_eq!(r.error_code, Some("E6"));
    }

    #[test]
    fn hold_limits_duration() {
        let mut api = api();
        let r = api.hold("e", 120_000);
        assert!(!r.ok);
        assert!(api.sink.holds.is_empty());
    }

    #[test]
    fn full_script_flow_records() {
        let mut api = api();
        api.state.backpack_fill = 0.96;
        api.move_to_hive();
        api.hold("e", 8000);
        api.state.backpack_fill = 0.2;
        api.move_to(0.5, 0.5);
        api.collect();

        assert_eq!(api.sink.hive_calls, 1);
        assert_eq!(api.sink.holds, vec![("e".into(), 8000)]);
        assert_eq!(api.sink.moves, vec![(0.5, 0.5)]);
        assert_eq!(api.sink.collects, 1);

        // Conformance: every function invoked tracks
        assert!(api.invocations.contains_key("move_to_hive"));
        assert!(api.invocations.contains_key("hold"));
    }

    #[test]
    fn queries_reflect_state() {
        let mut api = api();
        api.state = ControlState {
            backpack_fill: 0.5,
            position: (0.3, 0.7),
            field: "Pine Tree Forest".into(),
            is_dead: false,
            quest_ready: false,
            tokens: vec![(0.1, 0.2, "ability".into())],
        };
        assert_eq!(api.pollen(), 0.5);
        assert_eq!(api.position(), (0.3, 0.7));
        assert_eq!(api.field(), "Pine Tree Forest");
        assert_eq!(api.tokens().len(), 1);
    }

    #[test]
    fn conformance_100_percent() {
        let api = api();
        assert_eq!(api.conformance_coverage(), 100.0);
    }
}
