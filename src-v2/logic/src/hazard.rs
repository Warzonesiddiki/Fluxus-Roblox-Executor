/// # Hazard Detector (A3.3) — Threat Awareness from Vision State
///
/// Reads visual state (monsters, aggression indicators, proximity) and
/// produces a threat level + recommended evasive action for the behavior
/// tree.
///
/// ## Design
/// - Pure function of a `HazardInput` snapshot → `HazardAssessment`
/// - The behavior tree pre-empts farming when threat exceeds a threshold
/// - Deliberately conservative: false-positive (flee) is safer than
///   false-negative (death)

use serde::{Deserialize, Serialize};

// ─── Input ────────────────────────────────────────────────────────────────────

/// Visual/network state relevant to threats.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HazardInput {
    /// Visible monster entities (from packets: EntitySpawn kind=2).
    pub monsters: Vec<Monster>,
    /// Is the player currently being damaged (screen red flash / HP low)?
    pub under_attack: bool,
    /// Player health 0..1 (OCR/vision).
    pub health: f64,
    /// Distance to nearest hive (normalized 0..1).
    pub distance_to_hive: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monster {
    pub id: u64,
    pub distance: f64,
    /// Monster size class — bigger monsters are more dangerous.
    pub threat_rating: u8, // 1..10
    /// Is it already aggro'd (chasing)?
    pub aggro: bool,
}

// ─── Output ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatLevel {
    Safe,
    Caution,
    Dangerous,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardAssessment {
    pub level: ThreatLevel,
    /// Recommended behavior-tree override.
    pub recommended_action: RecommendedAction,
    /// Confidence in the assessment (0..1).
    pub confidence: f64,
    /// Explanation for logging/debug.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendedAction {
    /// Keep farming.
    Continue,
    /// Move away from the nearest monster (keep farming elsewhere).
    Reposition,
    /// Return to hive / safe zone.
    Retreat,
    /// Immediate emergency recovery (e.g., death imminent).
    Emergency,
}

// ─── Detector ─────────────────────────────────────────────────────────────────

/// Pure assessment function — no I/O, fully testable.
pub fn assess(input: &HazardInput) -> HazardAssessment {
    // Weighted threat score from visible monsters
    let mut score = 0.0f64;
    let mut nearest: Option<&Monster> = None;

    for m in &input.monsters {
        let proximity = (1.0 - m.distance.clamp(0.0, 1.0)); // 0 far, 1 near
        let aggro_bonus = if m.aggro { 2.0 } else { 0.5 };
        let monster_score = (m.threat_rating as f64 / 10.0) * (0.3 + 0.7 * proximity) * aggro_bonus;
        score += monster_score;
        if nearest.map_or(true, |n| m.distance < n.distance) {
            nearest = Some(m);
        }
    }

    // Under attack: major threat
    if input.under_attack {
        score += 3.0;
    }

    // Low health multiplies everything
    let health_factor = if input.health < 0.25 { 2.0 } else if input.health < 0.5 { 1.5 } else { 1.0 };
    score *= health_factor;

    let (level, action, reason) = if score >= 8.0 {
        (ThreatLevel::Critical, RecommendedAction::Emergency, format!("critical threat score {score:.1}"))
    } else if score >= 5.0 {
        (ThreatLevel::Dangerous, RecommendedAction::Retreat, format!("dangerous: score {score:.1}"))
    } else if score >= 2.0 {
        (ThreatLevel::Caution, RecommendedAction::Reposition, format!("caution: score {score:.1}"))
    } else {
        (ThreatLevel::Safe, RecommendedAction::Continue, format!("safe: score {score:.1}"))
    };

    // Confidence: more data → higher confidence
    let mut confidence = 0.5 + 0.1 * (input.monsters.len() as f64).min(5.0);
    if input.under_attack { confidence = (confidence + 0.2).min(1.0); }
    if input.health < 0.5 { confidence = (confidence + 0.1).min(1.0); }

    HazardAssessment {
        level,
        recommended_action: action,
        confidence: confidence.clamp(0.0, 1.0),
        reason,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn monster(distance: f64, rating: u8, aggro: bool) -> Monster {
        Monster { id: 1, distance, threat_rating: rating, aggro }
    }

    #[test]
    fn empty_world_is_safe() {
        let input = HazardInput { health: 1.0, ..Default::default() };
        let a = assess(&input);
        assert_eq!(a.level, ThreatLevel::Safe);
        assert_eq!(a.recommended_action, RecommendedAction::Continue);
    }

    #[test]
    fn distant_low_threat_is_caution() {
        let input = HazardInput {
            monsters: vec![monster(0.8, 4, false)],
            health: 1.0,
            ..Default::default()
        };
        let a = assess(&input);
        assert!(a.level == ThreatLevel::Safe || a.level == ThreatLevel::Caution);
    }

    #[test]
    fn aggro_near_high_threat_is_critical() {
        let input = HazardInput {
            monsters: vec![monster(0.1, 10, true)],
            health: 0.2,
            under_attack: true,
            ..Default::default()
        };
        let a = assess(&input);
        assert_eq!(a.level, ThreatLevel::Critical);
        assert_eq!(a.recommended_action, RecommendedAction::Emergency);
        assert!(a.confidence > 0.5);
    }

    #[test]
    fn low_health_escalates() {
        let input = HazardInput {
            monsters: vec![monster(0.5, 6, true)],
            health: 0.15,
            ..Default::default()
        };
        let a = assess(&input);
        assert!(a.level == ThreatLevel::Dangerous || a.level == ThreatLevel::Critical);
    }

    #[test]
    fn under_attack_flags_retreat_or_above() {
        let input = HazardInput {
            under_attack: true,
            health: 0.6,
            ..Default::default()
        };
        let a = assess(&input);
        assert!(a.level != ThreatLevel::Safe);
    }

    #[test]
    fn confidence_grows_with_data() {
        let sparse = HazardInput { monsters: vec![monster(0.5, 5, false)], health: 1.0, ..Default::default() };
        let dense = HazardInput {
            monsters: vec![monster(0.5, 5, false); 5],
            under_attack: true,
            health: 0.2,
            ..Default::default()
        };
        assert!(assess(&dense).confidence > assess(&sparse).confidence);
    }
}
