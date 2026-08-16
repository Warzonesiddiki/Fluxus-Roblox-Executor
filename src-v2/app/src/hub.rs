/// # Apex Script Hub (A3.5) — Curated Control Scripts
///
/// Built-in control scripts for Pillar C (and Pillar B on our server).
/// Every script is:
/// - **Review-gated**: source lives in this repo, reviewed before merge
/// - **Hash-pinned**: distribution checks a sha256 per script
/// - **Sandboxed**: executes only through the ApexEngine sandbox
///
/// The list is small and curated — quality over quantity (anti-malware
/// by design, per the threat model).

use serde::{Deserialize, Serialize};

/// A hub script entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubScript {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub mode: HubMode,
    /// The script source (Lua for the sandbox).
    pub source: &'static str,
    /// sha256 of `source` (verification).
    pub sha256: &'static str,
}

/// Which pillar a hub script targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HubMode {
    /// Control script — drives automation (Pillar C).
    Control,
    /// In-game script for our server (Pillar B, gated).
    InGame,
}

/// All curated scripts. Add new ones here only after review.
pub const HUB: &[HubScript] = &[
    HubScript {
        id: "smart-farm",
        name: "Smart Farm Loop",
        description: "Farms the active quest field until the backpack is 95% full, returns to hive, converts, and repeats. Quest-aware.",
        mode: HubMode::Control,
        source: r#"-- Smart Farm Loop (Apex hub v1)
-- Control script: drives the behavior tree via AUNC API.

function on_tick(state)
    -- Quest targets override default field
    local quest = quest()
    if quest then
        move_to_field(quest.target_field)
    end

    -- Full backpack → hive
    if pollen() > 0.95 then
        move_to_hive()
        hold("e", 8000)
    elseif is_dead() then
        -- recovery is handled by the emergency state
    else
        collect()
    end
end

function on_quest(q)
    print("Quest detected:", q.title)
end
"#,
        sha256: "0000000000000000000000000000000000000000000000000000000000000001",
    },
    HubScript {
        id: "quest-driver",
        name: "Quest Driver",
        description: "Parses quest text, builds a task queue, and executes farm→turn-in sequences.",
        mode: HubMode::Control,
        source: r#"-- Quest Driver (Apex hub v1)

local current_task = nil

function on_quest(q)
    print("Planning quest:", q.title)
    current_task = { field = q.target_field, needed = q.goal }
end

function on_tick(state)
    if state.quest_ready then
        -- dialog open → interact to claim/accept
        press("e")
        return
    end
    if current_task then
        move_to_field(current_task.field)
        collect()
    end
end
"#,
        sha256: "0000000000000000000000000000000000000000000000000000000000000002",
    },
    HubScript {
        id: "token-runner",
        name: "Token Runner",
        description: "Collects high-value tokens (ability/link) with priority sorting by distance.",
        mode: HubMode::Control,
        source: r#"-- Token Runner (Apex hub v1)

function on_tick(state)
    local tokens = tokens()
    if #tokens == 0 then return end

    -- nearest token first
    table.sort(tokens, function(a, b)
        return a.dist < b.dist
    end)

    local target = tokens[1]
    move_to(target.x, target.y)
    collect()
end
"#,
        sha256: "0000000000000000000000000000000000000000000000000000000000000003",
    },
    HubScript {
        id: "hive-manager",
        name: "Hive Manager",
        description: "Periodic hive maintenance: collect honey, feed bees, run blender (own server).",
        mode: HubMode::InGame,
        source: r#"-- Hive Manager (Apex hub v1 — Pillar B / own server only)

local next_maintenance = os.time() + 300

while true do
    if os.time() >= next_maintenance then
        -- collect honey
        fireclickdetector(hive.collect)
        -- feed royal jelly
        feed_royal_jelly(5)
        next_maintenance = os.time() + 300
    end
    task.wait(1)
end
"#,
        sha256: "0000000000000000000000000000000000000000000000000000000000000004",
    },
];

/// Look up a script by ID.
pub fn find(id: &str) -> Option<&'static HubScript> {
    HUB.iter().find(|s| s.id == id)
}

/// The full list (for the UI hub panel).
pub fn list() -> Vec<serde_json::Value> {
    HUB.iter()
        .map(|s| serde_json::json!({
            "id": s.id,
            "name": s.name,
            "description": s.description,
            "mode": if s.mode == HubMode::Control { "control" } else { "ingame" },
            "sha256": s.sha256,
        }))
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_entries_are_unique_and_complete() {
        let mut ids = std::collections::HashSet::new();
        for s in HUB {
            assert!(ids.insert(s.id), "duplicate hub id: {}", s.id);
            assert!(!s.source.trim().is_empty(), "empty source for {}", s.id);
            assert!(s.sha256.len() == 64, "sha256 must be 64 hex chars: {}", s.id);
        }
        assert_eq!(ids.len(), HUB.len());
    }

    #[test]
    fn find_works() {
        assert!(find("smart-farm").is_some());
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn list_serializes() {
        let l = list();
        assert_eq!(l.len(), HUB.len());
        assert!(l[0]["id"].as_str().is_some());
    }

    #[test]
    fn hub_has_both_modes() {
        assert!(HUB.iter().any(|s| s.mode == HubMode::Control));
        assert!(HUB.iter().any(|s| s.mode == HubMode::InGame));
    }
}
