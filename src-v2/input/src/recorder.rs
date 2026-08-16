/// # Route Recorder (A3.2) — Record, Save, and Replay Input Sequences
///
/// A macro route is a sequence of humanized actions captured from a live
/// session (or authored by hand) that can be replayed deterministically.
///
/// - `RecordedAction` — the serializable unit (move/click/key/wait)
/// - `RouteRecorder` — records actions with timestamps
/// - `save_route` / `load_route` — JSON persistence
/// - `replay_route` — humanized replay through `HumanizedInput`

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::HumanizedInput;

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RecorderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Route is empty")]
    EmptyRoute,
}

// ─── Recorded action ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordedAction {
    /// Mouse move to (x, y).
    Move { x: i32, y: i32 },
    /// Mouse click at current position.
    Click,
    /// Key press.
    Key { key: String },
    /// Wait (ms) — includes humanized jitter on replay.
    Wait { ms: u64 },
}

/// A full recorded route with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedRoute {
    pub name: String,
    pub created_ms: u64,
    pub actions: Vec<RecordedAction>,
}

// ─── Recorder ─────────────────────────────────────────────────────────────────

/// Records a live input session into a route.
pub struct RouteRecorder {
    route: RecordedRoute,
    started: Instant,
}

impl RouteRecorder {
    pub fn begin(name: &str) -> Self {
        Self {
            route: RecordedRoute {
                name: name.to_owned(),
                created_ms: now_ms(),
                actions: Vec::new(),
            },
            started: Instant::now(),
        }
    }

    /// Record an action (with the time since the previous action).
    pub fn record(&mut self, action: RecordedAction) {
        // Insert a Wait action reflecting elapsed wall-clock time so
        // replay preserves the original pacing.
        let elapsed = self.started.elapsed().as_millis() as u64;
        self.started = Instant::now();
        if elapsed > 0 {
            self.route.actions.push(RecordedAction::Wait { ms: elapsed });
        }
        self.route.actions.push(action);
    }

    pub fn finish(self) -> RecordedRoute {
        self.route
    }
}

// ─── Persistence ──────────────────────────────────────────────────────────────

pub fn save_route(route: &RecordedRoute, path: &Path) -> Result<(), RecorderError> {
    let json = serde_json::to_string_pretty(route)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_route(path: &Path) -> Result<RecordedRoute, RecorderError> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

// ─── Replay ───────────────────────────────────────────────────────────────────

/// Replay a route through the humanized input engine.
/// `speed` scales wait times (1.0 = original pacing).
pub fn replay_route(
    route: &RecordedRoute,
    input: &mut HumanizedInput,
    speed: f64,
) -> Result<(), RecorderError> {
    if route.actions.is_empty() {
        return Err(RecorderError::EmptyRoute);
    }
    for action in &route.actions {
        match action {
            RecordedAction::Move { x, y } => input.move_mouse_bezier(*x, *y, 150..400),
            RecordedAction::Click => input.click(),
            RecordedAction::Key { key } => input.press_key(key),
            RecordedAction::Wait { ms } => {
                let scaled = (*ms as f64 / speed.max(0.01)) as u64;
                input.wait_ms(scaled);
            }
        }
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_serialize_roundtrip() {
        let mut rec = RouteRecorder::begin("test-route");
        rec.record(RecordedAction::Move { x: 100, y: 200 });
        rec.record(RecordedAction::Click);
        rec.record(RecordedAction::Key { key: "e".into() });
        let route = rec.finish();

        let tmp = std::env::temp_dir().join("wsx_route_test.json");
        save_route(&route, &tmp).unwrap();
        let loaded = load_route(&tmp).unwrap();
        assert_eq!(loaded.name, "test-route");
        assert_eq!(loaded.actions.len(), route.actions.len());
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn replay_empty_route_fails() {
        let route = RecordedRoute {
            name: "empty".into(),
            created_ms: 0,
            actions: vec![],
        };
        let mut input = HumanizedInput::new().unwrap();
        assert!(matches!(replay_route(&route, &mut input, 1.0), Err(RecorderError::EmptyRoute)));
    }

    #[test]
    fn recorder_inserts_wait_for_pacing() {
        let mut rec = RouteRecorder::begin("pacing");
        rec.record(RecordedAction::Click);
        // The first action gets a Wait only if elapsed > 0; simulate a gap
        std::thread::sleep(Duration::from_millis(20));
        rec.record(RecordedAction::Click);
        let route = rec.finish();
        let wait_count = route.actions.iter().filter(|a| matches!(a, RecordedAction::Wait{..})).count();
        assert!(wait_count >= 1, "expected pacing waits, got actions: {:?}", route.actions);
    }
}
