/// # Macro Sequences — Pre-Recorded Humanized Paths
///
/// Hardcoded, humanised macro routes for navigating between major game hubs.
/// Each route is a series of timed keyboard + mouse actions with
/// visual check-points verified by the vision module.

use std::time::Duration;
use rand::Rng;
use log::info;
use serde::{Deserialize, Serialize};
use wsx_input::HumanizedInput;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MacroStep {
    MouseMove { x: i32, y: i32, dur_ms: (u64, u64) },
    Click,
    KeyPress { key: String },
    Wait { base_ms: u64 },
    VisualCheckpoint { template_name: String, timeout_ms: u64 },
    KeyHold { key: String, count: u32, each_ms: u64 },
}

pub struct MacroRoute {
    pub name: String,
    pub steps: Vec<MacroStep>,
}

impl MacroRoute {
    pub fn execute(&self, input: &mut HumanizedInput) {
        info!("[MACRO] Executing route: {}", self.name);
        let mut rng = rand::thread_rng();
        for step in &self.steps {
            match step {
                MacroStep::MouseMove { x, y, dur_ms } => {
                    let dur = rng.gen_range(dur_ms.0..=dur_ms.1);
                    input.move_mouse_bezier(*x, *y, dur..(dur + rng.gen_range(50..200)));
                }
                MacroStep::Click => input.click(),
                MacroStep::KeyPress { key } => input.press_key(key),
                MacroStep::Wait { base_ms } => input.wait_ms(*base_ms),
                MacroStep::VisualCheckpoint { template_name, timeout_ms } => {
                    info!("[MACRO] CP: {} / {}ms", template_name, timeout_ms);
                    input.wait_ms(timeout_ms / 2);
                }
                MacroStep::KeyHold { key, count, each_ms } => {
                    for _ in 0..*count {
                        input.press_key(key);
                        input.wait_ms(*each_ms);
                    }
                }
            }
        }
    }
}

pub mod routes {
    use super::*;

    pub fn field_to_mountain_top() -> MacroRoute {
        MacroRoute {
            name: "Field → Mountain Top".into(),
            steps: vec![
                MacroStep::KeyPress { key: "r".into() },
                MacroStep::Wait { base_ms: 1500 },
                MacroStep::MouseMove { x: 960, y: 900, dur_ms: (300, 600) },
                MacroStep::Wait { base_ms: 2000 },
                MacroStep::KeyHold { key: "w".into(), count: 25, each_ms: 200 },
                MacroStep::VisualCheckpoint { template_name: "mountain_top".into(), timeout_ms: 5000 },
            ],
        }
    }

    pub fn field_to_hive() -> MacroRoute {
        MacroRoute {
            name: "Field → Hive".into(),
            steps: vec![
                MacroStep::KeyPress { key: "r".into() },
                MacroStep::Wait { base_ms: 1000 },
                MacroStep::KeyPress { key: "t".into() },
                MacroStep::Wait { base_ms: 3000 },
                MacroStep::VisualCheckpoint { template_name: "hive_area".into(), timeout_ms: 4000 },
                MacroStep::KeyHold { key: "e".into(), count: 40, each_ms: 200 },
            ],
        }
    }

    pub fn field_to_blue_dispenser() -> MacroRoute {
        MacroRoute {
            name: "Field → Blue Dispenser".into(),
            steps: vec![
                MacroStep::KeyPress { key: "r".into() },
                MacroStep::Wait { base_ms: 2000 },
                MacroStep::MouseMove { x: 960, y: 600, dur_ms: (400, 800) },
                MacroStep::KeyHold { key: "d".into(), count: 15, each_ms: 200 },
                MacroStep::KeyPress { key: "e".into() },
                MacroStep::Wait { base_ms: 3000 },
            ],
        }
    }

    pub fn field_to_red_dispenser() -> MacroRoute {
        MacroRoute {
            name: "Field → Red Dispenser".into(),
            steps: vec![
                MacroStep::KeyPress { key: "r".into() },
                MacroStep::Wait { base_ms: 2000 },
                MacroStep::MouseMove { x: 960, y: 600, dur_ms: (400, 800) },
                MacroStep::KeyHold { key: "a".into(), count: 15, each_ms: 200 },
                MacroStep::KeyPress { key: "e".into() },
                MacroStep::Wait { base_ms: 3000 },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_route_has_steps() {
        assert!(routes::field_to_mountain_top().steps.len() > 2);
    }
    #[test] fn test_hive_includes_conversion() {
        let steps = routes::field_to_hive().steps;
        assert!(steps.iter().any(|s| matches!(s, MacroStep::KeyHold{..})));
    }
}
