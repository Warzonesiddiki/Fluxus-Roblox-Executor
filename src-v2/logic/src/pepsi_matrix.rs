/// # Pepsi Matrix — High-Level Feature Decision Table
///
/// Every toggleable feature from the original "Pepsi Swarm" script,
/// centralised into a clean, serialisable decision table. The frontend
/// reads/writes these values at runtime over the IPC bridge.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMatrix {
    pub token_farm: bool,
    pub auto_ability: bool,
    pub auto_link: bool,
    pub auto_sprout: bool,
    pub auto_feed: bool,
    pub auto_train: bool,
    pub auto_blender: bool,
    pub auto_dispense: bool,
    pub auto_quest: bool,
    pub auto_cannon: bool,
    pub request_honeystorm: bool,
    pub auto_meteor: bool,
    pub auto_stickbug: bool,
    pub auto_plant_sprouts: bool,
    pub auto_collect_honey: bool,
    pub basic_gather: bool,
    pub gather_token_switch_pct: f64,
    pub hive_return_threshold: f64,
}

impl Default for FeatureMatrix {
    fn default() -> Self {
        Self {
            token_farm: true, auto_ability: true, auto_link: true,
            auto_sprout: true, basic_gather: true, auto_quest: true,
            auto_dispense: true, auto_collect_honey: true,
            request_honeystorm: true, auto_meteor: true,
            auto_feed: false, auto_train: false, auto_blender: false,
            auto_cannon: false, auto_stickbug: false,
            auto_plant_sprouts: false,
            gather_token_switch_pct: 15.0,
            hive_return_threshold: 95.0,
        }
    }
}

impl FeatureMatrix {
    pub fn apply_updates(&mut self, updates: HashMap<String, serde_json::Value>) {
        for (key, value) in updates {
            match key.as_str() {
                "token_farm"  | "auto_ability" | "auto_link"
                | "auto_sprout" | "auto_feed" | "auto_train"
                | "auto_blender" | "auto_dispense" | "auto_quest"
                | "auto_cannon" | "request_honeystorm" | "auto_meteor"
                | "auto_stickbug" | "basic_gather" | "auto_collect_honey" => {
                    if let Some(b) = value.as_bool() {
                        match key.as_str() {
                            "token_farm" => self.token_farm = b,
                            "auto_ability" => self.auto_ability = b,
                            "auto_link" => self.auto_link = b,
                            "auto_sprout" => self.auto_sprout = b,
                            "auto_feed" => self.auto_feed = b,
                            "auto_train" => self.auto_train = b,
                            "auto_blender" => self.auto_blender = b,
                            "auto_dispense" => self.auto_dispense = b,
                            "auto_quest" => self.auto_quest = b,
                            "auto_cannon" => self.auto_cannon = b,
                            "request_honeystorm" => self.request_honeystorm = b,
                            "auto_meteor" => self.auto_meteor = b,
                            "auto_stickbug" => self.auto_stickbug = b,
                            "basic_gather" => self.basic_gather = b,
                            "auto_collect_honey" => self.auto_collect_honey = b,
                            _ => {}
                        }
                    }
                }
                "gather_token_switch_pct" | "hive_return_threshold" => {
                    if let Some(f) = value.as_f64() {
                        match key.as_str() {
                            "gather_token_switch_pct" => self.gather_token_switch_pct = f,
                            "hive_return_threshold" => self.hive_return_threshold = f,
                            _ => {}
                        }
                    }
                }
                _ => info!("[PepsiMatrix] Unknown key: {}", key),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PepsiDecision {
    GatherTokens,
    ReturnToHive,
    FeedBees,
    CraftBlender,
    QuestTurnIn,
    CollectDispenser,
    FireCannon,
    EventHoneystorm,
    EventMeteor,
    Idle,
}

pub fn evaluate_matrix(matrix: &FeatureMatrix, backpack_pct: f64, quest_ready: bool) -> PepsiDecision {
    if quest_ready && matrix.auto_quest {
        return PepsiDecision::QuestTurnIn;
    }
    if backpack_pct >= matrix.hive_return_threshold {
        return PepsiDecision::ReturnToHive;
    }
    if matrix.auto_dispense {
        return PepsiDecision::CollectDispenser;
    }
    if matrix.auto_blender {
        return PepsiDecision::CraftBlender;
    }
    if matrix.auto_feed {
        return PepsiDecision::FeedBees;
    }
    if backpack_pct < matrix.hive_return_threshold && matrix.token_farm {
        return PepsiDecision::GatherTokens;
    }
    PepsiDecision::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_defaults() {
        let m = FeatureMatrix::default();
        assert!(m.token_farm); assert!(!m.auto_blender);
    }

    #[test] fn test_quest_priority() {
        let d = evaluate_matrix(&FeatureMatrix::default(), 50.0, true);
        assert!(matches!(d, PepsiDecision::QuestTurnIn));
    }

    #[test] fn test_hive_return() {
        let d = evaluate_matrix(&FeatureMatrix::default(), 96.0, false);
        assert!(matches!(d, PepsiDecision::ReturnToHive));
    }
}
