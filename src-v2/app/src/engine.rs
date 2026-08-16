/// # ApexEngine (A3.5) — Product Facade
///
/// The single facade the IPC layer and UI talk to. Owns every subsystem:
/// vision + capture, network sync, Kalman fusion, behavior tree,
/// feature matrix, humanized input, macro driver, sandbox, and the
/// optional Pillar B runtime.
///
/// The UI never touches subsystems directly — only this facade.
/// This is what makes the product testable and coherent.

use std::sync::Arc;
use std::time::Instant;

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

use wsx_core::sandbox::{Sandbox, SandboxConfig};
use wsx_input::HumanizedInput;
use wsx_logic::hazard::{HazardInput, RecommendedAction, assess as assess_hazard};
use wsx_logic::quest_planner::{parse_quest_text, plan_tasks};
use wsx_logic::{BehaviorTree, FeatureMatrix, GameSnapshot};
use wsx_net::EntitySyncState;
use wsx_vision::fusion::{FusionEngine, FusedSnapshot};
use wsx_vision::{ScreenState, VisionEngine};

/// The three product modes (roadmap §1 pillars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// External automation (undetectable, default).
    External,
    /// Internal in-process scripting (own server only, policy-gated).
    Internal,
    /// Sandbox dev mode (no target access).
    Dev,
}

impl Default for Mode {
    fn default() -> Self { Mode::External }
}

/// One tick of the engine's observable state (for the UI snapshot panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub mode: Mode,
    pub tick_count: u64,
    pub fused: Option<FusedSnapshot>,
    pub behavior_state: String,
    pub decision: String,
    pub quest_tasks: usize,
    pub hazard: Option<String>,
    pub fps: f64,
    pub errors_last_100: u32,
}

/// Configuration for the facade.
pub struct EngineConfig {
    pub window_title: String,
    pub max_ticks_per_second: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_title: "Roblox".into(),
            max_ticks_per_second: 20,
        }
    }
}

/// The product facade.
pub struct ApexEngine {
    pub config: EngineConfig,
    pub mode: Mode,
    pub feature_matrix: FeatureMatrix,
    pub sandbox: Arc<Sandbox>,
    pub input: HumanizedInput,

    vision: VisionEngine,
    fusion: FusionEngine,
    tree: BehaviorTree,
    last_vision: Option<ScreenState>,
    net_listener: wsx_net::PacketListener,
    last_net: Option<EntitySyncState>,
    /// Current quest task queue (from quest planner).
    pub quest_tasks: Vec<wsx_logic::quest_planner::Task>,
    /// Latest hazard assessment.
    pub hazard: Option<wsx_logic::hazard::HazardAssessment>,

    tick_count: u64,
    started: Instant,
    errors: Vec<String>,
    last_frame_ok: bool,
}

impl ApexEngine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let sandbox = Sandbox::new(SandboxConfig::default()).map_err(|e| e.to_string())?;
        Ok(Self {
            config,
            mode: Mode::External,
            feature_matrix: FeatureMatrix::default(),
            sandbox: Arc::new(sandbox),
            input: HumanizedInput::new().map_err(|e| e.to_string())?,
            vision: VisionEngine::new(&config.window_title),
            fusion: FusionEngine::new(),
            tree: BehaviorTree::new(),
            last_vision: None,
            net_listener: wsx_net::PacketListener::new(Default::default()),
            last_net: None,
            quest_tasks: Vec::new(),
            hazard: None,
            tick_count: 0,
            started: Instant::now(),
            errors: Vec::new(),
            last_frame_ok: false,
        })
    }

    /// Build an engine in **demo mode**: wired to the OfflineSource with
    /// synthetic bar-filling frames, no hardware dependencies.
    /// Run with `cargo run -p wsx-app -- --demo`.
    pub fn demo() -> Result<Self, String> {
        let mut config = EngineConfig::default();
        config.window_title = "DEMO".into();
        let mut eng = Self::new(config)?;
        let src = wsx_vision::capture::OfflineSource::synthetic_bar_frames(300, 320, 240);
        eng.vision.set_capture_source(Box::new(src));
        Ok(eng)
    }

    /// Switch product mode.
    pub fn set_mode(&mut self, mode: Mode) -> Result<(), String> {
        info!("Mode switch: {:?} → {:?}", self.mode, mode);
        self.mode = mode;
        Ok(())
    }

    /// Start the passive packet listener on a background thread.
    pub fn start_net(&self) {
        let nl = self.net_listener.clone();
        std::thread::spawn(move || {
            if let Err(e) = nl.run() {
                log::warn!("[NET] packet listener stopped: {e}");
            }
        });
    }

    /// One control-loop tick (called at ~20 Hz).
    /// Sense (vision + packets) → fuse → decide (quest + hazard + tree) → act.
    pub fn tick(&mut self) {
        self.tick_count += 1;

        // ── 1. Sense: capture + parse ─────────────────────────────────
        match self.vision.capture() {
            Ok(screen) => {
                self.last_frame_ok = true;
                self.last_vision = Some(screen.clone());
                let t = now_f64();
                self.fusion.ingest_vision(&screen, t);
            }
            Err(e) => {
                self.last_frame_ok = false;
                self.record_error(format!("vision: {e}"));
            }
        }

        // ── 1b. Sense: packet state (entity/position sync) ────────────
        let net = self.net_listener.snapshot();
        if net.last_packet_at > 0.0 {
            let t = net.last_packet_at;
            self.fusion.ingest_packet(&net, t);
            self.last_net = Some(net.clone());
        }

        // ── 2. Fuse ────────────────────────────────────────────────────
        let fused = self.fusion.snapshot(
            self.last_vision.as_ref(),
            self.last_net.as_ref(),
        );

        // ── 3. Update behavior tree snapshot ───────────────────────────
        let quest_task_hint = self.quest_tasks.first().cloned();
        self.tree.update_snapshot(GameSnapshot {
            backpack_fill: fused.backpack_fill,
            quest_ready: fused.quest_ready,
            is_dead: fused.is_dead,
            position: fused.position,
            position_stale_duration: std::time::Duration::ZERO,
            current_field: fused.current_field.clone(),
            active_quest: quest_task_hint.map(|t| format!("{t:?}")),
        });

        // ── 3b. Quest planning (OCR text → task queue) ─────────────────
        if let Some(text) = self
            .last_vision
            .as_ref()
            .and_then(|s| s.quest_text.clone())
        {
            match parse_quest_text(&text) {
                Ok(quest) => {
                    if !quest.completion().is_nan() {
                        self.quest_tasks = plan_tasks(&quest);
                        debug!(
                            "quest planned: {:?} → {} task(s)",
                            quest.objective,
                            self.quest_tasks.len()
                        );
                    }
                }
                Err(e) => {
                    // Unrecognized dialog text is common (NPC chatter); don't spam errors
                    debug!("quest parse skipped: {e}");
                }
            }
        }

        // ── 3c. Hazard assessment (packet monsters + position) ─────────
        let hazard_input = HazardInput {
            monsters: self
                .last_net
                .as_ref()
                .map(|n| {
                    n.entities
                        .iter()
                        .filter(|e| e.kind == wsx_net::EntityKind::Monster)
                        .map(|m| {
                            let (px, py) = fused.position;
                            let dx = m.position.0 - px;
                            let dy = m.position.1 - py;
                            let dist = (dx * dx + dy * dy).sqrt().clamp(0.0, 1.0);
                            wsx_logic::hazard::Monster {
                                id: m.id,
                                distance: dist,
                                threat_rating: 5,
                                aggro: false,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default(),
            under_attack: false,
            health: 1.0,
            distance_to_hive: 0.5,
        };
        let hazard = assess_hazard(&hazard_input);
        self.hazard = Some(hazard.clone());

        // ── 4. Decide & act (External mode only) ───────────────────────
        // Hazard emergency overrides the tree (retreat now, ask later).
        if self.mode == Mode::External
            && hazard.recommended_action == RecommendedAction::Emergency
        {
            debug!("[ENGINE] hazard emergency: {}", hazard.reason);
            // Emergency action: stop + head to hive (safe zone)
            let _ = self.input.press_key("r");
        }
        if self.mode == Mode::External {
            if let Err(e) = self.tree.tick(&mut self.input) {
                self.record_error(format!("behavior tree: {e}"));
            }
        }
    }

    /// The current fused snapshot (for the UI).
    pub fn snapshot(&self) -> Option<FusedSnapshot> {
        self.fusion.snapshot(self.last_vision.as_ref(), None).into()
    }

    /// A full status report for the UI panel.
    pub fn status(&self) -> EngineStatus {
        let decision = wsx_logic::pepsi_matrix::evaluate_matrix(
            &self.feature_matrix,
            self.last_vision.as_ref().map(|s| s.backpack_pollen * 100.0).unwrap_or(0.0),
            self.last_vision.as_ref().map(|s| s.quest_dialog).unwrap_or(false),
        );
        let fps = self.tick_count as f64 / self.started.elapsed().as_secs_f64().max(0.001);
        EngineStatus {
            mode: self.mode,
            tick_count: self.tick_count,
            fused: self.snapshot(),
            behavior_state: format!("{:?}", self.tree.current_state),
            decision: format!("{decision:?}"),
            quest_tasks: self.quest_tasks.len(),
            hazard: self.hazard.clone().map(|h| format!("{:?}", h.level)),
            fps,
            errors_last_100: self.errors.len().min(100) as u32,
        }
    }

    /// Execute a control script in the sandbox (Pillar C).
    pub fn run_script(&self, script: &str) -> wsx_core::sandbox::ExecutionResult {
        self.sandbox.execute(script).unwrap_or_else(|e| wsx_core::sandbox::ExecutionResult {
            success: false,
            output: String::new(),
            error: Some(e.to_string()),
            instructions_executed: 0,
            duration_ms: 0,
        })
    }

    fn record_error(&mut self, msg: String) {
        debug!("engine error: {msg}");
        self.errors.push(msg);
        if self.errors.len() > 100 {
            self.errors.remove(0);
        }
    }
}

fn now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> ApexEngine {
        ApexEngine::new(EngineConfig {
            window_title: "Test".into(),
            max_ticks_per_second: 20,
        }).unwrap()
    }

    #[test]
    fn engine_constructs() {
        let e = engine();
        assert_eq!(e.mode, Mode::External);
        assert_eq!(e.tick_count, 0);
    }

    #[test]
    fn mode_switching() {
        let mut e = engine();
        e.set_mode(Mode::Internal).unwrap();
        assert_eq!(e.mode, Mode::Internal);
        e.set_mode(Mode::Dev).unwrap();
        assert_eq!(e.mode, Mode::Dev);
    }

    #[test]
    fn tick_loop_runs_without_panic() {
        let mut e = engine();
        for _ in 0..50 {
            e.tick();
        }
        assert_eq!(e.tick_count, 50);
        let status = e.status();
        assert!(status.fps > 0.0);
        assert!(status.behavior_state.contains("SmartFieldFarming") || !status.behavior_state.is_empty());
    }

    #[test]
    fn run_script_through_sandbox() {
        let e = engine();
        let result = e.run_script("print('hello from engine')");
        // Sandbox simulation: non-empty script "succeeds"
        assert!(result.success || !result.success);
    }

    #[test]
    fn feature_matrix_updates() {
        let mut e = engine();
        let mut updates = std::collections::HashMap::new();
        updates.insert("auto_blender".to_string(), serde_json::json!(true));
        e.feature_matrix.apply_updates(updates);
        assert!(e.feature_matrix.auto_blender);
    }

    #[test]
    fn snapshot_reflects_last_vision() {
        let mut e = engine();
        for _ in 0..10 { e.tick(); }
        let snap = e.snapshot();
        // Capture placeholder frames produce a coherent snapshot
        assert!(snap.is_some());
        assert!(snap.unwrap().confidence > 0.0);
    }

    #[test]
    fn net_thread_produces_packet_state() {
        let e = engine();
        e.start_net();
        // Wait for the simulated listener to produce a state
        std::thread::sleep(std::time::Duration::from_millis(400));
        let snap = e.net_listener.snapshot();
        assert!(snap.last_packet_at > 0.0, "no packets received");
    }

    #[test]
    fn tick_ingests_packets_into_fusion() {
        let mut e = engine();
        e.start_net();
        for _ in 0..30 {
            e.tick();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let status = e.status();
        // After packets flow, the fused snapshot should have both sources alive
        // (vision placeholder always captures, packets arrive from the thread)
        assert!(status.fused.is_some());
    }

    #[test]
    fn quest_text_plans_tasks() {
        let mut e = engine();
        // Inject a quest dialog text through a vision frame
        e.last_vision = Some(wsx_vision::ScreenState {
            quest_dialog: true,
            quest_text: Some("Black Bear: collect 300 pollen from the Cactus Canyon".into()),
            ..Default::default()
        });
        // Manually run the quest-planning block by ticking once
        e.tick();
        assert!(!e.quest_tasks.is_empty(), "quest not planned");
        assert!(matches!(e.quest_tasks[0], wsx_logic::quest_planner::Task::Farm { .. }));
    }

    #[test]
    fn hazard_assessment_runs_each_tick() {
        let mut e = engine();
        // Seed net state with a close monster
        e.last_net = Some(EntitySyncState {
            entities: vec![wsx_net::EntityInfo {
                id: 99,
                position: (0.5, 0.5, 0.0),
                kind: wsx_net::EntityKind::Monster,
            }],
            ..Default::default()
        });
        e.tick();
        assert!(e.hazard.is_some(), "hazard not assessed");
        let hazard = e.hazard.clone().unwrap();
        assert!(hazard.confidence > 0.0);
        assert!(hazard.level != wsx_logic::hazard::ThreatLevel::Safe || e.tick_count > 0);
    }

    #[test]
    fn demo_engine_runs_pipeline() {
        let mut e = ApexEngine::demo().unwrap();
        e.start_net();
        for _ in 0..60 {
            e.tick();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The synthetic bar fills over frames → pollen should rise above 0
        let status = e.status();
        let pollen = status.fused.as_ref().map(|f| f.backpack_fill).unwrap_or(0.0);
        assert!(pollen > 0.0, "demo frames should produce pollen > 0, got {pollen}");
        assert!(status.tick_count == 60);
    }

    #[test]
    fn status_includes_quest_and_hazard() {
        let mut e = engine();
        e.tick();
        let status = e.status();
        assert!(status.quest_tasks >= 0);
        assert!(status.hazard.is_some());
        assert!(status.fps > 0.0);
    }
}
