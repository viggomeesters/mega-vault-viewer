use std::path::PathBuf;
use std::sync::Mutex;

use mvv_core::{DocumentView, FileBrowserSnapshot, SearchHit, VaultRuntime, VaultStats};
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

#[derive(Debug, Serialize)]
struct RefreshSnapshot {
    stats: VaultStats,
}

#[derive(Debug, Serialize)]
struct SaveSnapshot {
    stats: VaultStats,
    document: DocumentView,
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
async fn refresh_index(
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<RefreshSnapshot, String> {
    let state_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("mvv-state");
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;

        Ok::<_, anyhow::Error>((runtime, stats))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats) = snapshot;

    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(RefreshSnapshot { stats })
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

#[tauri::command]
fn open_document_by_path(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<DocumentView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before opening notes".to_string())?;
    runtime
        .open_by_relative_path(&relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn file_browser(state: State<'_, AppState>) -> Result<FileBrowserSnapshot, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before browsing files".to_string())?;
    runtime.file_browser().map_err(|error| error.to_string())
}

#[tauri::command]
fn read_document_source(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<String, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Index a vault before editing notes".to_string())?;
    runtime
        .document_source_by_relative_path(&relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_document_source(
    state: State<'_, AppState>,
    vault_path: String,
    relative_path: String,
    source: String,
) -> Result<SaveSnapshot, String> {
    {
        let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
        let runtime = guard
            .as_ref()
            .ok_or_else(|| "Index a vault before editing notes".to_string())?;
        runtime
            .write_document_source_by_relative_path(&relative_path, &source)
            .map_err(|error| error.to_string())?;
    }

    let state_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("mvv-state");
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let document = runtime.open_by_relative_path(&relative_path)?;

        Ok::<_, anyhow::Error>((runtime, stats, document))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, document) = snapshot;
    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(SaveSnapshot { stats, document })
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            default_fixture_path,
            index_vault,
            refresh_index,
            search,
            open_document,
            open_document_by_id,
            open_document_by_path,
            file_browser,
            read_document_source,
            save_document_source
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mega Vault Viewer");
}
