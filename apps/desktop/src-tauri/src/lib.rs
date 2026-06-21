use std::path::PathBuf;
use std::sync::Mutex;

use mvv_core::{
    FileBrowserSnapshot, IndexSummary, SearchHit, VaultItemView, VaultRuntime, VaultStats,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{Emitter, Manager, State};

const DEFAULT_VAULT_PATH: &str =
    "/Users/viggomeesters/Library/Mobile Documents/iCloud~md~obsidian/Documents/vault";
const STATE_DIR_ENV: &str = "MEGA_VAULT_VIEWER_STATE_DIR";

#[derive(Default)]
struct AppState {
    runtime: Mutex<Option<VaultRuntime>>,
    watcher: Mutex<Option<VaultWatcher>>,
}

struct VaultWatcher {
    _watcher: RecommendedWatcher,
    path: PathBuf,
}

#[derive(Debug, Serialize)]
struct IndexSnapshot {
    stats: VaultStats,
    first_item: Option<VaultItemView>,
    index_summary: IndexSummary,
}

#[derive(Debug, Serialize)]
struct RefreshSnapshot {
    stats: VaultStats,
    index_summary: IndexSummary,
}

#[derive(Debug, Serialize)]
struct SaveSnapshot {
    stats: VaultStats,
    item: VaultItemView,
}

#[derive(Debug, Clone, Serialize)]
struct VaultChangedPayload {
    paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WatchStatus {
    watching: bool,
    path: Option<PathBuf>,
}

#[tauri::command]
fn default_vault_path() -> String {
    DEFAULT_VAULT_PATH.to_string()
}

#[tauri::command]
async fn index_vault(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<IndexSnapshot, String> {
    let state_dir = runtime_state_dir(&app)?;
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let first_item = runtime.first_item()?;
        let index_summary = runtime.index_summary();

        Ok::<_, anyhow::Error>((runtime, stats, first_item, index_summary))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, first_item, index_summary) = snapshot;

    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(IndexSnapshot {
        stats,
        first_item,
        index_summary,
    })
}

#[tauri::command]
async fn reset_index(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<IndexSnapshot, String> {
    let state_dir = runtime_state_dir(&app)?;
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        VaultRuntime::reset_runtime_state(&state_dir)?;
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let first_item = runtime.first_item()?;
        let index_summary = runtime.index_summary();

        Ok::<_, anyhow::Error>((runtime, stats, first_item, index_summary))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, first_item, index_summary) = snapshot;

    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(IndexSnapshot {
        stats,
        first_item,
        index_summary,
    })
}

#[tauri::command]
fn start_vault_watcher(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<WatchStatus, String> {
    let path = PathBuf::from(vault_path);
    let watcher = create_vault_watcher(app, path)?;
    let status = WatchStatus {
        watching: true,
        path: Some(watcher.path.clone()),
    };
    *state.watcher.lock().map_err(|_| "watcher lock poisoned")? = Some(watcher);
    Ok(status)
}

#[tauri::command]
fn stop_vault_watcher(state: State<'_, AppState>) -> Result<WatchStatus, String> {
    *state.watcher.lock().map_err(|_| "watcher lock poisoned")? = None;
    Ok(WatchStatus {
        watching: false,
        path: None,
    })
}

#[tauri::command]
fn watch_status(state: State<'_, AppState>) -> Result<WatchStatus, String> {
    let guard = state.watcher.lock().map_err(|_| "watcher lock poisoned")?;
    Ok(WatchStatus {
        watching: guard.is_some(),
        path: guard.as_ref().map(|watcher| watcher.path.clone()),
    })
}

#[tauri::command]
async fn refresh_index(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<RefreshSnapshot, String> {
    let state_dir = runtime_state_dir(&app)?;
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let index_summary = runtime.index_summary();

        Ok::<_, anyhow::Error>((runtime, stats, index_summary))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, index_summary) = snapshot;

    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(RefreshSnapshot {
        stats,
        index_summary,
    })
}

#[tauri::command]
fn search(state: State<'_, AppState>, query: String) -> Result<Vec<SearchHit>, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before searching".to_string())?;
    runtime
        .search(&query, 20)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document(state: State<'_, AppState>, slug: String) -> Result<VaultItemView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before opening notes".to_string())?;
    runtime
        .open_item_by_slug(&slug)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document_by_id(state: State<'_, AppState>, id: i64) -> Result<VaultItemView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before opening notes".to_string())?;
    runtime
        .open_item_by_id(id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document_by_path(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<VaultItemView, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before opening notes".to_string())?;
    runtime
        .open_item_by_relative_path(&relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_item_by_path(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<VaultItemView, String> {
    open_document_by_path(state, relative_path)
}

#[tauri::command]
fn open_item_in_system(state: State<'_, AppState>, relative_path: String) -> Result<(), String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before opening files".to_string())?;
    let item = runtime
        .open_item_by_relative_path(&relative_path)
        .map_err(|error| error.to_string())?;
    std::process::Command::new("open")
        .arg(&item.path)
        .spawn()
        .map_err(|error| format!("open in system failed: {error}"))?;
    Ok(())
}

#[tauri::command]
fn file_browser(state: State<'_, AppState>) -> Result<FileBrowserSnapshot, String> {
    let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard
        .as_ref()
        .ok_or_else(|| "Open a vault before browsing files".to_string())?;
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
        .ok_or_else(|| "Open a vault before editing notes".to_string())?;
    runtime
        .document_source_by_relative_path(&relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_document_source(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    vault_path: String,
    relative_path: String,
    source: String,
) -> Result<SaveSnapshot, String> {
    {
        let guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
        let runtime = guard
            .as_ref()
            .ok_or_else(|| "Open a vault before editing notes".to_string())?;
        runtime
            .write_document_source_by_relative_path(&relative_path, &source)
            .map_err(|error| error.to_string())?;
    }

    let state_dir = runtime_state_dir(&app)?;
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        let runtime = VaultRuntime::build(&vault_path, state_dir)?;
        let stats = runtime.stats()?;
        let item = runtime.open_item_by_relative_path(&relative_path)?;

        Ok::<_, anyhow::Error>((runtime, stats, item))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;

    let (runtime, stats, item) = snapshot;
    *state.runtime.lock().map_err(|_| "runtime lock poisoned")? = Some(runtime);
    Ok(SaveSnapshot { stats, item })
}

fn runtime_state_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    explicit_state_dir(std::env::var_os(STATE_DIR_ENV))
        .map(Ok)
        .unwrap_or_else(|| app.path().app_data_dir().map_err(|error| error.to_string()))
}

fn create_vault_watcher(app: tauri::AppHandle, path: PathBuf) -> Result<VaultWatcher, String> {
    let emit_app = app.clone();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) => {
                let paths = event
                    .paths
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect();
                let _ = emit_app.emit("vault_changed", VaultChangedPayload { paths });
            }
            Err(error) => {
                let _ = emit_app.emit("vault_watch_error", error.to_string());
            }
        })
        .map_err(|error| error.to_string())?;
    watcher
        .watch(&path, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    Ok(VaultWatcher {
        _watcher: watcher,
        path,
    })
}

fn explicit_state_dir(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            default_vault_path,
            index_vault,
            reset_index,
            refresh_index,
            start_vault_watcher,
            stop_vault_watcher,
            watch_status,
            search,
            open_document,
            open_document_by_id,
            open_document_by_path,
            open_item_by_path,
            open_item_in_system,
            file_browser,
            read_document_source,
            save_document_source
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mega Vault Viewer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_viggos_obsidian_vault() {
        assert_eq!(
            default_vault_path(),
            "/Users/viggomeesters/Library/Mobile Documents/iCloud~md~obsidian/Documents/vault"
        );
    }

    #[test]
    fn supports_explicit_runtime_state_directory_override() {
        assert_eq!(
            explicit_state_dir(Some("/tmp/mvv-state-test".into())),
            Some(PathBuf::from("/tmp/mvv-state-test"))
        );
        assert_eq!(explicit_state_dir(Some("".into())), None);
        assert_eq!(explicit_state_dir(None), None);
    }
}
