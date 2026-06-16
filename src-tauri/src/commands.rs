use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncBufReadExt;

use crate::error::AppError;
use crate::tray;
use crate::version::event::{EventSink, VersionEvent};
use crate::version::types::NodeVersion;
use crate::version::{VersionCommand, ExecuteOutput};

pub struct AppState {
    pub nodepilot_dir: PathBuf,
    pub manager: crate::version::VersionManager,
    pub setup_flag: PathBuf,
    pub config_path: PathBuf,
    pub projects_path: PathBuf,
    pub servers: Mutex<HashMap<String, tokio::process::Child>>,
    pub log_buffers: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub mirror_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectBinding {
    pub version: String,
    pub name: String,
    pub path: String,
}

struct TauriEventSink<'a> {
    app: &'a AppHandle,
}

impl EventSink for TauriEventSink<'_> {
    fn emit(&self, event: VersionEvent) {
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

fn emit_events(sink: &dyn EventSink, output: &ExecuteOutput) {
    for event in &output.events {
        sink.emit(event.clone());
    }
}

#[tauri::command]
pub async fn get_versions(
    state: State<'_, AppState>,
) -> Result<Vec<NodeVersion>, AppError> {
    let sink = NopSink;
    state
        .manager
        .execute(VersionCommand::Get, &sink)
        .await
        .map_err(AppError::from)
        .map(|o| o.versions)
}

#[tauri::command]
pub async fn refresh_versions(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<NodeVersion>, AppError> {
    let sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Refresh, &sink)
        .await
        .map_err(AppError::from)?;
    emit_events(&sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn install_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<NodeVersion>, AppError> {
    let sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Install { version }, &sink)
        .await
        .map_err(AppError::from)?;
    emit_events(&sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn activate_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<NodeVersion>, AppError> {
    let sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Activate { version: version.clone() }, &sink)
        .await
        .map_err(AppError::from)?;

    emit_events(&sink, &output);

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
) -> Result<Vec<NodeVersion>, AppError> {
    let sink = TauriEventSink { app: &app };
    let output = state
        .manager
        .execute(VersionCommand::Delete { version }, &sink)
        .await
        .map_err(AppError::from)?;
    emit_events(&sink, &output);
    Ok(output.versions)
}

#[tauri::command]
pub async fn is_first_run(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(!state.setup_flag.exists())
}

#[tauri::command]
pub async fn mark_setup_done(state: State<'_, AppState>) -> Result<(), AppError> {
    if let Some(parent) = state.setup_flag.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    std::fs::write(&state.setup_flag, b"1").map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    if state.config_path.exists() {
        let data =
            std::fs::read_to_string(&state.config_path).map_err(|e| AppError::Io(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| AppError::Parse(e.to_string()))
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
) -> Result<(), AppError> {
    if let Some(ref url) = config.mirror_url {
        state.manager.set_source_url(url.clone());
    }

    if let Some(parent) = state.config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| AppError::Config(e.to_string()))?;
    std::fs::write(&state.config_path, data).map_err(|e| AppError::Io(e.to_string()))
}

fn read_projects(path: &PathBuf) -> Vec<ProjectBinding> {
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(projects) = serde_json::from_str::<Vec<ProjectBinding>>(&data) {
                return projects;
            }
        }
    }
    vec![]
}

#[tauri::command]
pub fn bind_project(
    state: State<'_, AppState>,
    version: String,
    name: String,
    path: String,
) -> Result<(), AppError> {
    let mut projects = read_projects(&state.projects_path);
    projects.push(ProjectBinding {
        version,
        name,
        path,
    });
    let data =
        serde_json::to_string_pretty(&projects).map_err(|e| AppError::Config(e.to_string()))?;
    if let Some(parent) = state.projects_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    std::fs::write(&state.projects_path, data).map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn get_project_bindings(state: State<'_, AppState>) -> Result<Vec<ProjectBinding>, AppError> {
    Ok(read_projects(&state.projects_path))
}

#[tauri::command]
pub fn unbind_project(
    state: State<'_, AppState>,
    version: String,
    path: String,
) -> Result<(), AppError> {
    let projects = read_projects(&state.projects_path);
    let filtered: Vec<ProjectBinding> = projects
        .into_iter()
        .filter(|p| !(p.version == version && p.path == path))
        .collect();
    let data =
        serde_json::to_string_pretty(&filtered).map_err(|e| AppError::Config(e.to_string()))?;
    if let Some(parent) = state.projects_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    std::fs::write(&state.projects_path, data).map_err(|e| AppError::Io(e.to_string()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackageJson {
    pub scripts: Option<HashMap<String, String>>,
}

fn detect_package_manager(project_dir: &PathBuf) -> &'static str {
    let pnpm = project_dir.join("pnpm-lock.yaml");
    let yarn = project_dir.join("yarn.lock");
    if pnpm.exists() {
        "pnpm"
    } else if yarn.exists() {
        "yarn"
    } else {
        "npm"
    }
}

#[tauri::command]
pub fn read_package_json(path: String) -> Result<PackageJson, AppError> {
    let pkg_path = PathBuf::from(&path).join("package.json");
    if !pkg_path.exists() {
        return Err(AppError::NotFound("package.json not found".to_string()));
    }
    let content = std::fs::read_to_string(&pkg_path).map_err(|e| AppError::Io(e.to_string()))?;
    let pkg: PackageJson = serde_json::from_str(&content).map_err(|e| AppError::Parse(e.to_string()))?;
    Ok(pkg)
}

#[tauri::command]
pub fn detect_pm(path: String) -> String {
    detect_package_manager(&PathBuf::from(&path)).to_string()
}

#[tauri::command]
pub async fn start_dev_server(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    command: String,
) -> Result<(), AppError> {
    {
        let servers = state.servers.lock().unwrap();
        if servers.contains_key(&path) {
            return Err(AppError::Config("server already running".to_string()));
        }
    }

    let project_dir = PathBuf::from(&path);
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(AppError::Config("empty command".to_string()));
    }

    let program = parts[0];
    let args: Vec<&str> = parts[1..].iter().copied().collect();

    // 将 nodepilot 管理的 Node bin 目录加入 PATH，
    // 避免打包应用因 PATH 受限而找不到 npm/pnpm/yarn 等命令
    let nodepilot_bin = state.nodepilot_dir.join("current").join("bin");
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", nodepilot_bin.display(), existing_path);

    let mut child = tokio::process::Command::new(program)
        .args(&args)
        .current_dir(&project_dir)
        .env("PATH", &new_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| AppError::Io(format!("failed to start dev server: {e}")))?;

    let stdout = child.stdout.take()
        .ok_or_else(|| AppError::Io("no stdout".to_string()))?;
    let stderr = child.stderr.take()
        .ok_or_else(|| AppError::Io("no stderr".to_string()))?;

    {
        let mut servers = state.servers.lock().unwrap();
        servers.insert(path.clone(), child);
    }

    let log_buffers = state.log_buffers.clone();

    // Spawn stdout reader: 逐行读取 → 缓冲 + 事件
    let app_stdout = app.clone();
    let path_out = path.clone();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
            let trimmed = line.trim_end().to_string();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }
            {
                let mut buffers = log_buffers.lock().unwrap();
                let buf = buffers.entry(path_out.clone()).or_default();
                if buf.len() >= 1000 {
                    buf.remove(0);
                }
                buf.push(trimmed.clone());
            }
            let _ = app_stdout.emit("dev_server_log", serde_json::json!({
                "path": path_out,
                "line": trimmed,
            }));
            line.clear();
        }
    });

    // Spawn stderr reader
    let app_stderr = app.clone();
    let path_err = path.clone();
    let log_buffers_err = state.log_buffers.clone();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut line = String::new();
        while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
            let trimmed = line.trim_end().to_string();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }
            {
                let mut buffers = log_buffers_err.lock().unwrap();
                let buf = buffers.entry(path_err.clone()).or_default();
                if buf.len() >= 1000 {
                    buf.remove(0);
                }
                buf.push(trimmed.clone());
            }
            let _ = app_stderr.emit("dev_server_log", serde_json::json!({
                "path": path_err,
                "line": trimmed,
            }));
            line.clear();
        }
    });

    let _ = app.emit("dev_server_status", serde_json::json!({
        "path": path,
        "running": true,
    }));

    Ok(())
}

#[tauri::command]
pub async fn stop_dev_server(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), AppError> {
    let pid = {
        let mut servers = state.servers.lock().unwrap();
        servers.remove(&path).and_then(|c| c.id())
    };

    if let Some(pid) = pid {
        let _ = std::process::Command::new("kill")
            .arg("--")
            .arg(format!("-{pid}"))
            .output();
    }

    let _ = app.emit("dev_server_status", serde_json::json!({
        "path": path,
        "running": false,
    }));

    Ok(())
}

#[tauri::command]
pub fn get_dev_server_logs(state: State<'_, AppState>, path: String) -> Result<Vec<String>, AppError> {
    let buffers = state.log_buffers.lock().unwrap();
    Ok(buffers.get(&path).cloned().unwrap_or_default())
}

pub fn cleanup_all_servers(state: &AppState) {
    let mut servers = state.servers.lock().unwrap();
    let pids: Vec<u32> = servers.values().filter_map(|c| c.id()).collect();
    servers.clear();
    for pid in pids {
        let _ = std::process::Command::new("kill")
            .arg("--")
            .arg(format!("-{pid}"))
            .output();
    }
}

struct NopSink;

impl EventSink for NopSink {
    fn emit(&self, _event: VersionEvent) {}
}
