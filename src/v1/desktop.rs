use crate::desktop::Desktop;
use serde_json::{Value, json};
use tauri::State;
#[tauri::command]
pub async fn preview_revision(id: String, state: State<'_, Desktop>) -> Result<Value, String> {
    let folders = state.shared_folders();
    tokio::task::spawn_blocking(move || super::edits::preview(&folders, &id))
        .await
        .map_err(|_| "Aperçu interrompu")?
}
#[tauri::command]
pub fn v1_workspace(state: State<'_, Desktop>) -> Result<Value, String> {
    let folders = state.shared_folders();
    let indexes:Vec<_>=folders.iter().map(|f|json!({"folder":f.id,"status":super::index::status(&folders,&f.id).unwrap_or_else(|e|json!({"error":e}))})).collect();
    Ok(
        json!({"brands":super::state::brands()?,"workflows":super::jobs::list()?,"reviews":super::edits::list()?,"indexes":indexes}),
    )
}
#[tauri::command]
pub fn save_brand_profiles(profiles: Value, state: State<'_, Desktop>) -> Result<(), String> {
    state.check_stopped()?;
    super::state::save_brands(serde_json::from_value(profiles).map_err(|e| e.to_string())?)
}
#[tauri::command]
pub async fn build_local_index(folder: String, state: State<'_, Desktop>) -> Result<Value, String> {
    let folders = state.shared_folders();
    tokio::task::spawn_blocking(move || {
        super::index::build(
            &folders,
            super::index::IndexArgs {
                folder,
                offset: None,
                max_files: Some(1000),
            },
        )
    })
    .await
    .map_err(|_| "Index interrompu")?
}
#[tauri::command]
pub fn revision_action(
    id: String,
    action: String,
    state: State<'_, Desktop>,
) -> Result<Value, String> {
    state.check_stopped()?;
    super::edits::apply(&state.shared_folders(), &id, &action)
}
#[tauri::command]
pub fn workflow_action(
    id: String,
    action: String,
    state: State<'_, Desktop>,
) -> Result<Value, String> {
    let folders = state.shared_folders();
    if action == "undo" {
        state.check_stopped()?;
        return super::jobs::undo(&folders, &id);
    }
    super::jobs::control(
        folders,
        super::jobs::ControlArgs { id, action },
        state.workflow_lifetime()?,
    )
}
