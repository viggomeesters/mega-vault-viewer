use std::path::PathBuf;
use std::sync::Mutex;

use mvv_core::{DocumentView, SearchHit, VaultRuntime, VaultStats};
use serde::Serialize;
use tauri::State;

#[derive(Default)]
struct AppState {
    runtime: Mutex<Option<VaultRuntime>>,
}

#[derive(Debug, Serialize)]
struct IndexSnapshot {
    stats: VaultStats,
    first_document: Option<DocumentView>,
}

#[tauri::command]
fn default_fixture_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("fixtures/demo-vault")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("fixtures/demo-vault"))
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
async fn index_vault(
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<IndexSnapshot, String> {
    let state_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("mvv-state");
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let first_document = runtime.first_document()?;

        Ok::<_, anyhow::Error>((runtime, stats, first_document))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, first_document) = snapshot;

    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(IndexSnapshot {
        stats,
        first_document,
    })
}

#[tauri::command]
fn search(state: State<'_, AppState>, query: String) -> Result<Vec<SearchHit>, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before searching".to_string())?;
    runtime
        .search(&query, 20)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document(state: State<'_, AppState>, slug: String) -> Result<DocumentView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before opening notes".to_string())?;
    runtime
        .open_by_slug(&slug)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document_by_id(state: State<'_, AppState>, id: i64) -> Result<DocumentView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before opening notes".to_string())?;
    runtime.open_by_id(id).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            default_fixture_path,
            index_vault,
            search,
            open_document,
            open_document_by_id
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mega Vault Viewer");
}
