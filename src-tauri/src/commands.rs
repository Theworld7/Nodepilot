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

/// 剥离 ANSI 转义码（颜色、光标控制等），避免在非终端 UI 中显示乱码。
/// 处理 CSI 序列 (\x1b[...m) 和 OSC 序列 (\x1b]...\x07)。
#[allow(dead_code)]
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next(); // consume '['
                    // 跳过参数部分（数字和分号）
                    while chars.next_if(|&ch| ch.is_ascii_digit() || ch == ';').is_some() {}
                    // 跳过终止字母
                    chars.next_if(|&ch| ch.is_ascii_alphabetic());
                }
                Some(']') => {
                    // OSC 序列: \x1b]...\x07 或 \x1b]...ST
                    chars.next(); // consume ']'
                    while let Some(&ch) = chars.peek() {
                        if ch == '\x07' || ch == '\x1b' {
                            break;
                        }
                        chars.next();
                    }
                    if chars.peek() == Some(&'\x1b') {
                        chars.next(); // consume ESC
                        chars.next_if(|&ch| ch == '\\'); // consume ST terminator
                    } else {
                        chars.next_if(|&ch| ch == '\x07'); // consume BEL
                    }
                }
                _ => {
                    // 非标准 ESC 序列，跳过下一个字符
                    chars.next();
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(strip_ansi("\x1b[32mVITE\x1b[39m v5.0.12"), "VITE v5.0.12");
        assert_eq!(strip_ansi("\x1b[32m\x1b[1mVITE\x1b[22m v5.0.12\x1b[39m"), "VITE v5.0.12");
    }

    #[test]
    fn strip_ansi_removes_complex_sequences() {
        let input = "\x1b[2m\x1b[32m  ➜\x1b[39m\x1b[22m\x1b[2m  press \x1b[22m\x1b[1mh + enter\x1b[22m\x1b[2m to show help\x1b[22m";
        assert_eq!(strip_ansi(input), "  ➜  press h + enter to show help");
    }

    #[test]
    fn strip_ansi_preserves_plain_text() {
        assert_eq!(strip_ansi("hello world"), "hello world");
        assert_eq!(strip_ansi("no ansi codes here"), "no ansi codes here");
    }

    #[test]
    fn strip_ansi_handles_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn strip_ansi_handles_real_vite_output() {
        let input = "\x1b[32m\x1b[1mVITE\x1b[22m v5.0.12\x1b[39m  \x1b[2mready in \x1b[0m\x1b[1m1460\x1b[22m\x1b[2m\x1b[0m ms\x1b[22m";
        assert_eq!(strip_ansi(input), "VITE v5.0.12  ready in 1460 ms");

        let input2 = "\x1b[32m➜\x1b[39m  \x1b[1mLocal\x1b[22m:   \x1b[36mhttp://localhost:\x1b[1m3006\x1b[22m/\x1b[39m";
        assert_eq!(strip_ansi(input2), "➜  Local:   http://localhost:3006/");
    }
}

pub struct AppState {
    pub nodepilot_dir: PathBuf,
    pub manager: crate::version::VersionManager,
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
    #[serde(default)]
    pub default_script: Option<String>,
    #[serde(default)]
    pub command_prefix: Option<String>,
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
pub async fn auto_setup(state: State<'_, AppState>) -> Result<bool, AppError> {
    crate::env_setup::setup(&state.nodepilot_dir)
        .map(|_| true)
        .map_err(|e| AppError::Setup(e.to_string()))
}

#[tauri::command]
pub async fn rollback_setup(state: State<'_, AppState>) -> Result<(), AppError> {
    crate::env_setup::rollback(&state.nodepilot_dir);
    Ok(())
}

#[tauri::command]
pub async fn is_auto_setup_done(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(crate::env_setup::is_setup_done(&state.nodepilot_dir))
}

#[tauri::command]
pub async fn get_setup_error(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    let error_path = state.nodepilot_dir.join(".auto-setup-error");
    if error_path.exists() {
        let content = std::fs::read_to_string(&error_path)
            .map_err(|e| AppError::Io(e.to_string()))?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn mark_setup_skipped(state: State<'_, AppState>) -> Result<(), AppError> {
    let flag_path = state.nodepilot_dir.join(".auto-setup-done");
    std::fs::write(&flag_path, b"skipped").map_err(|e| AppError::Io(e.to_string()))
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
        default_script: None,
        command_prefix: None,
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

#[tauri::command]
pub fn update_project_name(
    state: State<'_, AppState>,
    version: String,
    path: String,
    new_name: String,
) -> Result<(), AppError> {
    let mut projects = read_projects(&state.projects_path);
    if let Some(p) = projects.iter_mut().find(|p| p.version == version && p.path == path) {
        p.name = new_name;
    } else {
        return Err(AppError::NotFound("project binding not found".to_string()));
    }
    let data =
        serde_json::to_string_pretty(&projects).map_err(|e| AppError::Config(e.to_string()))?;
    if let Some(parent) = state.projects_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    std::fs::write(&state.projects_path, data).map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn update_project_config(
    state: State<'_, AppState>,
    version: String,
    path: String,
    default_script: Option<String>,
    command_prefix: Option<String>,
) -> Result<(), AppError> {
    let mut projects = read_projects(&state.projects_path);
    if let Some(p) = projects.iter_mut().find(|p| p.version == version && p.path == path) {
        p.default_script = default_script;
        p.command_prefix = command_prefix;
    } else {
        return Err(AppError::NotFound("project binding not found".to_string()));
    }
    let data =
        serde_json::to_string_pretty(&projects).map_err(|e| AppError::Config(e.to_string()))?;
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

    // 打包应用双击启动时，子进程 stdout/stderr 是管道而非 TTY，
    // 系统默认全缓冲（约 8KB）。stdbuf 只影响直接子进程，孙进程（如 npm → vite）
    // 继承管道 fd 后仍会全缓冲。使用 PTY 包装，让整个进程树认为连接了终端 → 行缓冲。
    // macOS: 内置 script -q /dev/null <命令>
    // Linux: stdbuf -o0 -e0 <命令> 作为降级（不覆盖孙进程，但至少子进程无缓冲）
    let pty_program;
    let use_pty: bool;
    #[cfg(target_os = "macos")]
    {
        pty_program = "/usr/bin/script";
        use_pty = std::path::Path::new(pty_program).exists();
    }
    #[cfg(not(target_os = "macos"))]
    {
        pty_program = "";
        use_pty = false;
    }

    let (program, args): (&str, Vec<&str>) = if use_pty {
        ("script", {
            let mut a = vec!["-q", "/dev/null", parts[0]];
            a.extend(&parts[1..]);
            a
        })
    } else {
        // 降级：stdbuf 至少让直接子进程无缓冲
        let stdbuf_path = "/usr/bin/stdbuf";
        let has_stdbuf = std::path::Path::new(stdbuf_path).exists();
        if has_stdbuf {
            ("stdbuf", {
                let mut a = vec!["-o0", "-e0", parts[0]];
                a.extend(&parts[1..]);
                a
            })
        } else {
            (parts[0], parts[1..].iter().copied().collect())
        }
    };

    // 将 nodepilot 管理的 Node bin 目录加入 PATH，
    // 避免打包应用因 PATH 受限而找不到 npm/pnpm/yarn 等命令
    let nodepilot_bin = state.nodepilot_dir.join("current").join("bin");
    let existing_path = std::env::var("PATH").unwrap_or_default();

    // 注入常见开发工具路径，解决打包应用 PATH 受限问题
    // 例如 tauri CLI 需要 cargo，Homebrew 工具等
    let home = dirs::home_dir().unwrap_or_default();
    let extra_paths = [
        home.join(".cargo/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/local/sbin"),
    ];
    let extra = extra_paths
        .iter()
        .filter(|p| p.exists())
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":");

    let new_path = if extra.is_empty() {
        format!("{}:{}", nodepilot_bin.display(), existing_path)
    } else {
        format!("{}:{}:{}", nodepilot_bin.display(), extra, existing_path)
    };

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
