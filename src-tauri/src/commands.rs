use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::tray;
use crate::version::event::{EventSink, VersionEvent};
use crate::version::types::NodeVersion;
use crate::version::{VersionCommand, ExecuteOutput};

pub struct AppState {
    pub manager: crate::version::VersionManager,
    pub setup_flag: PathBuf,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub mirror_url: Option<String>,
}

struct TauriEventSink<'a> {
    app: &'a AppHandle,
}

impl EventSink for TauriEventSink<'_> {
    fn emit(&mut self, event: VersionEvent) {
        match event {
            VersionEvent::VersionsUpdated(versions) => {
                let _ = self.app.emit("versions_updated", &versions);
            }
            VersionEvent::InstallProgress {
                version,
                stage,
                percent,
            } => {
                let _ = self.app.emit(
                    "install_progress",
                    serde_json::json!({
                        "version": version,
                        "stage": stage,
                        "percent": percent,
                    }),
                );
            }
            VersionEvent::VersionActivated { version } => {
                let _ = self
                    .app
                    .emit("version_activated", serde_json::json!({ "version": version }));
            }
        }
    }
}

fn emit_events(sink: &mut dyn EventSink, output: &ExecuteOutput) {
    for event in &output.events {
        sink.emit(event.clone());
    }
}

#[tauri::command]
pub async fn get_versions(
    state: State<'_, AppState>,
) -> Result<Vec<NodeVersion>, String> {
    let mut sink = NopSink;
    state
        .manager
        .execute(VersionCommand::Get, &mut sink)
        .await
        .map_err(|e| e.to_string())
        .map(|o| o.versions)
}

#[tauri::command]
pub async fn refresh_versions(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<NodeVersion>, String> {
    let mut sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Refresh, &mut sink)
        .await
        .map_err(|e| e.to_string())?;
    emit_events(&mut sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn install_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<NodeVersion>, String> {
    let mut sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Install { version }, &mut sink)
        .await
        .map_err(|e| e.to_string())?;
    emit_events(&mut sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn activate_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<NodeVersion>, String> {
    let mut sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Activate { version: version.clone() }, &mut sink)
        .await
        .map_err(|e| e.to_string())?;

    emit_events(&mut sink, &output);

    if let Some(tray) = app.tray_by_id("main") {
        let icon = tray::generate_icon(&version);
        let _ = tray.set_icon(Some(icon));
    }

    Ok(output.versions)
}

#[tauri::command]
pub async fn delete_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<NodeVersion>, String> {
    let mut sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Delete { version }, &mut sink)
        .await
        .map_err(|e| e.to_string())?;
    emit_events(&mut sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn is_first_run(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(!state.setup_flag.exists())
}

#[tauri::command]
pub async fn mark_setup_done(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(parent) = state.setup_flag.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&state.setup_flag, b"1").map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    if state.config_path.exists() {
        let data = std::fs::read_to_string(&state.config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    } else {
        let url = state.manager.source_url();
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
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    if let Some(ref url) = config.mirror_url {
        state.manager.set_source_url(url.clone());
    }

    if let Some(parent) = state.config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&state.config_path, data).map_err(|e| e.to_string())
}

struct NopSink;

impl EventSink for NopSink {
    fn emit(&mut self, _event: VersionEvent) {}
}
