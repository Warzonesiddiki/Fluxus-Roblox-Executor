// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use tauri::State;

/// Shared application state across Tauri commands.
struct AppState {
    /// The IPC bridge status.
    bridge_online: Mutex<bool>,
}

#[tauri::command]
fn execute_script(script: String) -> Result<String, String> {
    // Route to the sandbox (wsx-core) — in the full build this happens
    // through the wsx-ipc bridge. For the shell, we validate + return.
    if script.trim().is_empty() {
        return Err("Empty script".into());
    }
    Ok(format!("Script accepted ({} chars)", script.len()))
}

#[tauri::command]
fn get_feature_matrix() -> Vec<(String, bool)> {
    vec![
        ("token_farm".into(), true),
        ("auto_ability".into(), true),
        ("auto_link".into(), true),
        ("auto_sprout".into(), true),
        ("auto_feed".into(), false),
        ("auto_blender".into(), false),
        ("auto_quest".into(), true),
        ("auto_dispense".into(), true),
    ]
}

fn main() {
    let state = AppState {
        bridge_online: Mutex::new(false),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![execute_script, get_feature_matrix])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
