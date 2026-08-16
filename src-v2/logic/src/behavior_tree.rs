/// # Hierarchical Finite State Machine (HFSM) — Autoplay Engine
///
/// This module implements a closed-loop controller for autonomous gameplay.
/// It reads visual state from the vision module and drives input via the
/// input module through a priority-ordered HFSM.
///
/// ## Priority ordering (highest → lowest):
/// 1. **Emergency** – death / stuck detection
/// 2. **Quest Turn-in** – NPC interaction when quests complete
/// 3. **Hive Return** – convert pollen when backpack is full
/// 4. **Field Farming** – default gather-and-move behaviour
///
/// Each state is a self-contained node that can pre-empt lower-priority states.
/// The Humanization Layer introduces pseudo-random variance into every action.

use std::time::{Duration, Instant};
use rand::Rng;
use serde::{Deserialize, Serialize};
use log::{info, trace};

use wsx_input::HumanizedInput;

// ─── Duration constants ──────────────────────────────────────────────────────

/// How often the behaviour tree ticks (50 ms = 20 Hz).
pub const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// If position hasn't changed for this long, assume we're stuck.
pub const STUCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Minimum time to stay in hive-return state before re-checking.
pub const HIVE_CONVERSION_DURATION: Duration = Duration::from_secs(8);

/// Time between field rotation decisions (20–30 min).
pub const FIELD_ROTATION_MIN: Duration = Duration::from_secs(20 * 60);
pub const FIELD_ROTATION_MAX: Duration = Duration::from_secs(30 * 60);

// ─── Error handling ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BtError {
    #[error("Vision module returned no valid state")]
    NoVisionState,
    #[error("Input driver failed: {0}")]
    InputError(String),
    #[error("Navigation pathfinding failed: no route to {target}")]
    PathfindError { target: String },
    #[error("Behaviour tree not started — call tick() first")]
    NotStarted,
}

// ─── Public types ──────────────────────────────────────────────────────────────

/// The four priority states in the HFSM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameState {
    /// Respawn / unstuck recovery (highest priority).
    EmergencyRecovery,
    /// Navigate to NPC, claim rewards, accept next quest.
    QuestManagement,
    /// Return to hive, convert pollen to honey.
    HiveReturn,
    /// Default — harvest pollen from the current field.
    SmartFieldFarming,
}

impl GameState {
    pub fn priority(self) -> u8 {
        match self {
            GameState::EmergencyRecovery => 4,
            GameState::QuestManagement   => 3,
            GameState::HiveReturn        => 2,
            GameState::SmartFieldFarming => 1,
        }
    }
}

/// A snapshot of all game-relevant state at a given tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSnapshot {
    /// Current backpack pollen fill (0.0 – 1.0).
    pub backpack_fill: f64,
    /// Is a quest-complete dialog visible?
    pub quest_ready: bool,
    /// Is the respawn screen visible? (player was defeated)
    pub is_dead: bool,
    /// Player's current field position (normalized coordinates).
    pub position: (f64, f64),
    /// Has the position changed since last tick?
    pub position_stale_duration: Duration,
    /// Current field name (if detected).
    pub current_field: String,
    /// Active quest description (if any).
    pub active_quest: Option<String>,
}

impl Default for GameSnapshot {
    fn default() -> Self {
        Self {
            backpack_fill: 0.0,
            quest_ready: false,
            is_dead: false,
            position: (0.0, 0.0),
            position_stale_duration: Duration::ZERO,
            current_field: "Unknown".into(),
            active_quest: None,
        }
    }
}

// ─── Behaviour tree node ──────────────────────────────────────────────────────

/// Each node evaluates a condition and produces a sequence of input actions.
#[derive(Debug, Clone)]
pub struct BehaviorNode {
    pub name: &'static str,
    pub state: GameState,
    pub condition: fn(&GameSnapshot) -> bool,
    pub action: fn(&GameSnapshot, &mut HumanizedInput) -> Result<(), BtError>,
}

impl BehaviorNode {
    pub fn tick(&self, snapshot: &GameSnapshot, input: &mut HumanizedInput) -> Option<Result<(), BtError>> {
        if (self.condition)(snapshot) {
            trace!("BT node '{}' activating", self.name);
            Some((self.action)(snapshot, input))
        } else {
            None
        }
    }
}

// ─── Conditions (pure functions) ─────────────────────────────────────────────

pub fn is_emergency(snap: &GameSnapshot) -> bool {
    snap.is_dead || snap.position_stale_duration >= STUCK_TIMEOUT
}

pub fn is_quest_ready(snap: &GameSnapshot) -> bool {
    snap.quest_ready && !snap.is_dead
}

pub fn is_backpack_full(snap: &GameSnapshot) -> bool {
    snap.backpack_fill >= 0.95 && !snap.is_dead && !snap.quest_ready
}

pub fn should_farm(snap: &GameSnapshot) -> bool {
    snap.backpack_fill < 0.95 && !snap.is_dead && !snap.quest_ready
}

// ─── Actions ──────────────────────────────────────────────────────────────────

pub fn do_emergency_recovery(_snap: &GameSnapshot, input: &mut HumanizedInput) -> Result<(), BtError> {
    info!("[EMERGENCY] Player down or stuck — initiating recovery");
    input.wait_ms(2500);
    input.move_mouse_bezier(960, 200, 400..800);
    input.move_mouse_bezier(960, 540, 300..600);
    input.press_key("r");
    input.wait_ms(500 + rand::thread_rng().gen_range(0..300));
    info!("[EMERGENCY] Recovery sequence complete");
    Ok(())
}

pub fn do_quest_turnin(snap: &GameSnapshot, input: &mut HumanizedInput) -> Result<(), BtError> {
    info!("[QUEST] Completing quest: {:?}", snap.active_quest);
    input.move_mouse_bezier(500, 300, 200..400);
    input.click();
    input.wait_ms(1000 + rand::thread_rng().gen_range(0..500));
    for _ in 0..5 {
        input.press_key("e");
        input.wait_ms(1200 + rand::thread_rng().gen_range(0..300));
    }
    info!("[QUEST] Turn-in complete");
    Ok(())
}

pub fn do_hive_return(_snap: &GameSnapshot, input: &mut HumanizedInput) -> Result<(), BtError> {
    info!("[HIVE] Backpack full — returning to hive");
    input.press_key("r");
    input.wait_ms(2000 + rand::thread_rng().gen_range(0..500));
    let convert_start = Instant::now();
    while convert_start.elapsed() < HIVE_CONVERSION_DURATION {
        input.press_key("e");
        input.wait_ms(200 + rand::thread_rng().gen_range(0..150));
    }
    info!("[HIVE] Pollen conversion complete");
    Ok(())
}

pub fn do_field_farming(snap: &GameSnapshot, input: &mut HumanizedInput) -> Result<(), BtError> {
    trace!("[FARM] Farming at {} (backpack: {:.1}%)",
           snap.current_field, snap.backpack_fill * 100.0);
    let x_base = 960.0;
    let y_vary = rand::thread_rng().gen_range(300..800);
    // Sweep right
    input.move_mouse_bezier((x_base + 400.0) as i32, y_vary, 800..1500);
    input.click();
    input.wait_ms(50 + rand::thread_rng().gen_range(0..100));
    // Sweep left
    input.move_mouse_bezier(
        (x_base - 400.0) as i32,
        y_vary + rand::thread_rng().gen_range(-50..50),
        800..1500,
    );
    input.click();
    input.wait_ms(50 + rand::thread_rng().gen_range(0..100));
    Ok(())
}

// ─── The Behaviour Tree ──────────────────────────────────────────────────────

pub struct BehaviorTree {
    pub nodes: Vec<BehaviorNode>,
    pub current_state: GameState,
    pub previous_state: Option<GameState>,
    pub snapshot: GameSnapshot,
    last_field_rotation: Instant,}

impl BehaviorTree {
    pub fn new() -> Self {
        let nodes = vec![
            BehaviorNode { name: "Emergency Recovery",  state: GameState::EmergencyRecovery,  condition: is_emergency,    action: do_emergency_recovery },
            BehaviorNode { name: "Quest Management",    state: GameState::QuestManagement,    condition: is_quest_ready,   action: do_quest_turnin },
            BehaviorNode { name: "Hive Return",          state: GameState::HiveReturn,         condition: is_backpack_full, action: do_hive_return },
            BehaviorNode { name: "Smart Field Farming",  state: GameState::SmartFieldFarming,  condition: should_farm,      action: do_field_farming },
        ];
        Self { nodes, current_state: GameState::SmartFieldFarming, previous_state: None, snapshot: GameSnapshot::default(), last_field_rotation: Instant::now() }
    }

    pub fn update_snapshot(&mut self, new_snap: GameSnapshot) { self.snapshot = new_snap; }

    pub fn tick(&mut self, input: &mut HumanizedInput) -> Result<(), BtError> {
        for node in &self.nodes {
            if let Some(result) = node.tick(&self.snapshot, input) {
                self.previous_state = Some(self.current_state);
                self.current_state = node.state;
                if self.current_state != self.previous_state.unwrap() {
                    info!("State transition: {:?} → {:?}", self.previous_state.unwrap(), self.current_state);
                }
                if self.current_state == GameState::SmartFieldFarming {
                    self.maybe_rotate_field();
                }
                return result;
            }
        }
        Ok(())
    }

    fn maybe_rotate_field(&mut self) {
        let elapsed = self.last_field_rotation.elapsed();
        let threshold = Duration::from_secs(
            rand::thread_rng().gen_range(FIELD_ROTATION_MIN.as_secs()..=FIELD_ROTATION_MAX.as_secs())
        );
        if elapsed >= threshold {
            let fields = [
                "Pine Tree Forest", "Sunflower Field", "Cactus Canyon", "Bamboo Field",
                "Pumpkin Patch", "Mushroom Field", "Clover Field", "Strawberry Field",
                "Pepper Patch", "Rose Field", "Blue Flower Field",
            ];
            let chosen = fields[rand::thread_rng().gen_range(0..fields.len())];
            info!("[SCHEDULER] Rotating field → {} (after {:?})", chosen, elapsed);
            self.last_field_rotation = Instant::now();
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_priority() {
        assert!(GameState::EmergencyRecovery.priority() > GameState::QuestManagement.priority());
        assert!(GameState::QuestManagement.priority() > GameState::HiveReturn.priority());
        assert!(GameState::HiveReturn.priority() > GameState::SmartFieldFarming.priority());
    }

    #[test]
    fn test_emergency_on_death() {
        let mut snap = GameSnapshot::default();
        snap.is_dead = true;
        assert!(is_emergency(&snap));
        assert!(!should_farm(&snap));
        assert!(!is_quest_ready(&snap));
        assert!(!is_backpack_full(&snap));
    }

    #[test]
    fn test_backpack_full_triggers_hive() {
        let mut snap = GameSnapshot::default();
        snap.backpack_fill = 0.96;
        assert!(is_backpack_full(&snap));
        assert!(!should_farm(&snap));
        snap.backpack_fill = 0.50;
        assert!(!is_backpack_full(&snap));
        assert!(should_farm(&snap));
    }

    #[test]
    fn test_full_bt_cycle() {
        let mut tree = BehaviorTree::new();
        let mut input = HumanizedInput::new().unwrap();

        // Dead → emergency
        tree.snapshot.is_dead = true;
        assert!(tree.tick(&mut input).is_ok());
        assert_eq!(tree.current_state, GameState::EmergencyRecovery);

        // Healed, backpack full → hive return
        tree.snapshot.is_dead = false;
        tree.snapshot.backpack_fill = 0.98;
        assert!(tree.tick(&mut input).is_ok());
        assert_eq!(tree.current_state, GameState::HiveReturn);

        // Backpack empty → farming
        tree.snapshot.backpack_fill = 0.30;
        assert!(tree.tick(&mut input).is_ok());
        assert_eq!(tree.current_state, GameState::SmartFieldFarming);
    }

    #[test]
    fn test_stuck_detection() {
        let mut snap = GameSnapshot::default();
        snap.position_stale_duration = Duration::from_secs(15);
        assert!(is_emergency(&snap));
    }
}
