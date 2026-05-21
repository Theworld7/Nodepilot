use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::tray;

use crate::version::{
    types::NodeVersion, VersionActivator, VersionDeleter, VersionFetcher, VersionInstaller,
};

pub struct AppState {
    pub fetcher: VersionFetcher,
    pub installer: VersionInstaller,
    pub activator: VersionActivator,
    pub deleter: VersionDeleter,
    pub setup_flag: PathBuf,
    pub config_path: PathBuf,
    pub source_url: std::sync::Mutex<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub mirror_url: Option<String>,
}

fn enrich_versions(state: &AppState, mut versions: Vec<NodeVersion>) -> Vec<NodeVersion> {
    let installed = state.activator.get_installed_versions().unwrap_or_default();
    let current = state.activator.get_current_version();

    for v in &mut versions {
        v.installed = Some(installed.contains(&v.version));
        v.active = Some(current.as_deref() == Some(&v.version));
    }
    versions
}

async fn emit_versions(app: &AppHandle, state: &AppState) {
    if let Ok(versions) = state.fetcher.fetch_or_cache().await {
        let versions = enrich_versions(state, versions);
        let _ = app.emit("versions_updated", &versions);
    }
}

#[tauri::command]
pub async fn get_versions(state: State<'_, Arc<AppState>>) -> Result<Vec<NodeVersion>, String> {
    let versions = state.fetcher.fetch_or_cache().await.map_err(|e| e.to_string())?;
    Ok(enrich_versions(&state, versions))
}

#[tauri::command]
pub async fn refresh_versions(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<NodeVersion>, String> {
    let versions = state.fetcher.refresh().await.map_err(|e| e.to_string())?;
    let enriched = enrich_versions(&state, versions);
    let _ = app.emit("versions_updated", &enriched);
    Ok(enriched)
}

#[tauri::command]
pub async fn install_version(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    version: String,
) -> Result<(), String> {
    state
        .installer
        .install(&version, &app)
        .await
        .map_err(|e| e.to_string())?;

    emit_versions(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn activate_version(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    version: String,
) -> Result<(), String> {
    state
        .activator
        .activate(&version)
        .map_err(|e| e.to_string())?;

    if let Some(tray) = app.tray_by_id("main") {
        let icon = tray::generate_icon(&version);
        let _ = tray.set_icon(Some(icon));
    }

    let _ = app.emit("version_activated", serde_json::json!({ "version": version }));

    Ok(())
}

#[tauri::command]
pub async fn delete_version(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    version: String,
) -> Result<(), String> {
    state
        .deleter
        .delete(&version)
        .map_err(|e| e.to_string())?;

    emit_versions(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn is_first_run(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    Ok(!state.setup_flag.exists())
}

#[tauri::command]
pub async fn mark_setup_done(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Some(parent) = state.setup_flag.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(&state.setup_flag, b"1")
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, String> {
    if state.config_path.exists() {
        let data = std::fs::read_to_string(&state.config_path)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    } else {
        let url = state.source_url.lock().unwrap().clone();
        let mirror_url = if url != "https://nodejs.org/dist/index.json" {
            Some(url)
        } else {
            None
        };
        Ok(AppConfig { mirror_url })
    }
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, Arc<AppState>>,
    config: AppConfig,
) -> Result<(), String> {
    if let Some(ref url) = config.mirror_url {
        *state.source_url.lock().unwrap() = url.clone();
    }

    if let Some(parent) = state.config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&state.config_path, data).map_err(|e| e.to_string())
}
