/// # Quest Planner (A3.3) — Goal Inference & Task Queue
///
/// Converts detected quest state (packet `QuestState` or OCR'd dialog text)
/// into a structured **task queue** the behavior tree can execute:
/// `target field → required amount → current progress → action`.
///
/// ## Pipeline
/// ```
/// QuestState/OCR text → normalize → infer goal → TaskQueue
/// ```

use serde::{Deserialize, Serialize};

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum QuestError {
    #[error("Quest text not recognized: '{0}'")]
    Unrecognized(String),
    #[error("No quest text supplied")]
    Empty,
}

// ─── Data model ───────────────────────────────────────────────────────────────

/// A normalized quest with an inferred goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quest {
    pub id: String,
    pub title: String,
    /// The field the quest targets (if any).
    pub target_field: Option<String>,
    /// What to collect / do, e.g. "pollen", "defeat", "collect_tokens".
    pub objective: QuestObjective,
    /// Required amount (if numeric).
    pub goal: u32,
    /// Current progress (from packets/OCR).
    pub progress: u32,
    /// NPC who gave the quest, if known.
    pub giver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestObjective {
    CollectPollen { field: String },
    DefeatMonsters { count: u32 },
    CollectTokens { token_type: String, count: u32 },
    UseAbility { count: u32 },
    Unknown,
}

impl Quest {
    /// Fractional completion (0..1).
    pub fn completion(&self) -> f64 {
        if self.goal == 0 { return 0.0; }
        (self.progress as f64 / self.goal as f64).clamp(0.0, 1.0)
    }
}

/// One executable step toward a quest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Task {
    /// Go farm a field until the amount is collected.
    Farm { field: String, amount_needed: u32 },
    /// Return to the NPC to turn in / claim.
    TurnIn { npc: String },
    /// Collect N tokens of a type.
    CollectTokens { token_type: String, count: u32 },
    /// Defeat N monsters (hazard-aware).
    Defeat { count: u32 },
    /// No actionable task (wait / idle).
    Idle,
}

// ─── Planner ──────────────────────────────────────────────────────────────────

/// Converts raw quest text (OCR) into a normalized Quest.
pub fn parse_quest_text(text: &str) -> Result<Quest, QuestError> {
    let t = text.trim().to_lowercase();
    if t.is_empty() { return Err(QuestError::Empty); }

    // Pattern 1: "Collect 500 pollen from the Pine Tree Forest"
    if t.contains("pollen") && t.contains("from the") {
        let (field, goal) = extract_field_and_amount(text)?;
        return Ok(Quest {
            id: slug(text),
            title: text.to_owned(),
            target_field: Some(field.clone()),
            objective: QuestObjective::CollectPollen { field },
            goal,
            progress: 0,
            giver: extract_giver(text),
        });
    }

    // Pattern 2: "Defeat 10 monsters" / "Defeat 5 Mobs"
    if t.contains("defeat") || t.contains("kill") {
        let goal = extract_amount(text).unwrap_or(10);
        return Ok(Quest {
            id: slug(text),
            title: text.to_owned(),
            target_field: None,
            objective: QuestObjective::DefeatMonsters { count: goal },
            goal,
            progress: 0,
            giver: extract_giver(text),
        });
    }

    // Pattern 3: "Collect 50 ability tokens"
    if t.contains("token") && t.contains("collect") {
        let goal = extract_amount(text).unwrap_or(20);
        return Ok(Quest {
            id: slug(text),
            title: text.to_owned(),
            target_field: None,
            objective: QuestObjective::CollectTokens { token_type: "ability".into(), count: goal },
            goal,
            progress: 0,
            giver: extract_giver(text),
        });
    }

    Err(QuestError::Unrecognized(text.to_owned()))
}

/// Convert a Quest into an actionable task queue (ordered).
pub fn plan_tasks(quest: &Quest) -> Vec<Task> {
    match &quest.objective {
        QuestObjective::CollectPollen { field } => {
            let remaining = quest.goal.saturating_sub(quest.progress);
            if remaining > 0 {
                vec![
                    Task::Farm { field: field.clone(), amount_needed: remaining },
                    // After farming, turn in (if the quest came from an NPC)
                    quest.giver.clone().map(|npc| Task::TurnIn { npc }).into_iter().collect::<Vec<_>>(),
                ].into_iter().flatten().collect()
            } else {
                quest.giver.clone().map(|npc| Task::TurnIn { npc }).into_iter().collect()
            }
        }
        QuestObjective::DefeatMonsters { count } => {
            let remaining = count.saturating_sub(quest.progress);
            vec![
                Task::Defeat { count: remaining },
                quest.giver.clone().map(|npc| Task::TurnIn { npc }).into_iter().collect::<Vec<_>>(),
            ].into_iter().flatten().collect()
        }
        QuestObjective::CollectTokens { token_type, count } => {
            let remaining = count.saturating_sub(quest.progress);
            vec![Task::CollectTokens { token_type: token_type.clone(), count: remaining }]
        }
        QuestObjective::Unknown => vec![Task::Idle],
        QuestObjective::UseAbility { .. } => vec![Task::Idle],
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn extract_field_and_amount(text: &str) -> Result<(String, u32), QuestError> {
    let lower = text.to_lowercase();
    let field = text
        .split("from the")
        .nth(1)
        .map(|s| s.trim().trim_end_matches(['.', '!']).to_owned())
        .unwrap_or_else(|| "Unknown Field".to_owned());
    let amount = extract_amount(text).unwrap_or(100);
    Ok((field, amount))
}

fn extract_amount(text: &str) -> Option<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .next()
        .and_then(|s| s.parse().ok())
}

fn extract_giver(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for npc in ["black bear", "mother bear", "brown bear", "panda bear", "scientist"] {
        if lower.contains(npc) {
            return Some(npc.to_owned());
        }
    }
    None
}

fn slug(text: &str) -> String {
    let mut s = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        }
    }
    if s.len() > 24 { s.truncate(24); }
    if s.is_empty() { "quest".into() } else { s }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pollen_quest() {
        let q = parse_quest_text("Collect 500 pollen from the Pine Tree Forest").unwrap();
        assert_eq!(q.goal, 500);
        assert_eq!(q.target_field.as_deref(), Some("Pine Tree Forest"));
        assert_eq!(q.objective, QuestObjective::CollectPollen { field: "Pine Tree Forest".into() });
    }

    #[test]
    fn parse_defeat_quest() {
        let q = parse_quest_text("Defeat 10 monsters").unwrap();
        assert_eq!(q.goal, 10);
        assert!(matches!(q.objective, QuestObjective::DefeatMonsters { .. }));
    }

    #[test]
    fn parse_token_quest() {
        let q = parse_quest_text("Collect 25 ability tokens").unwrap();
        assert!(matches!(q.objective, QuestObjective::CollectTokens { .. }));
        assert_eq!(q.goal, 25);
    }

    #[test]
    fn parse_with_npc_giver() {
        let q = parse_quest_text("Black Bear: collect 300 pollen from the Cactus Canyon").unwrap();
        assert_eq!(q.giver.as_deref(), Some("black bear"));
    }

    #[test]
    fn unrecognized_quest_errors() {
        assert!(matches!(parse_quest_text("zzzqqq random text"), Err(QuestError::Unrecognized(_))));
        assert!(matches!(parse_quest_text(""), Err(QuestError::Empty)));
    }

    #[test]
    fn plan_full_pollen_quest() {
        let q = parse_quest_text("Mother Bear: collect 1000 pollen from the Bamboo Field").unwrap();
        let tasks = plan_tasks(&q);
        assert_eq!(tasks[0], Task::Farm { field: "Bamboo Field".into(), amount_needed: 1000 });
        assert_eq!(tasks[1], Task::TurnIn { npc: "mother bear".into() });
    }

    #[test]
    fn plan_completed_quest_skips_farm() {
        let mut q = parse_quest_text("collect 100 pollen from the Clover Field").unwrap();
        q.progress = 100;
        let tasks = plan_tasks(&q);
        assert!(!tasks.iter().any(|t| matches!(t, Task::Farm{..})));
    }

    #[test]
    fn completion_ratio() {
        let mut q = parse_quest_text("collect 100 pollen from the Clover Field").unwrap();
        q.progress = 50;
        assert_eq!(q.completion(), 0.5);
    }
}
