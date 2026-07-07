mod client;
mod commands;
mod env_setup;
mod error;
mod fs;
mod tray;
mod version;

use std::sync::Arc;

use commands::AppState;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri::WindowEvent;
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home = dirs::home_dir().unwrap_or_default();
    let nodepilot_dir = home.join(".nodepilot");
    let cache_dir = nodepilot_dir.join("cache");
    let versions_dir = nodepilot_dir.join("versions");
    let config_path = nodepilot_dir.join("config.json");

    let http_client = client::HttpClientProd::new().expect("Failed to create HTTP client");
    let http_client: Arc<dyn client::HttpClient> = Arc::new(http_client);

    let fs: Arc<dyn fs::FileSystem> = Arc::new(fs::FsProd);

    let source_url = if config_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&config_path) {
            if let Ok(cfg) = serde_json::from_str::<commands::AppConfig>(&data) {
                cfg.mirror_url
                    .unwrap_or_else(|| "https://nodejs.org/dist/index.json".to_string())
            } else {
                "https://nodejs.org/dist/index.json".to_string()
            }
        } else {
            "https://nodejs.org/dist/index.json".to_string()
        }
    } else {
        "https://nodejs.org/dist/index.json".to_string()
    };

    let manager = version::VersionManager::new(
        nodepilot_dir.clone(),
        versions_dir,
        cache_dir,
        http_client,
        fs,
        source_url,
    );

    let projects_path = nodepilot_dir.join("projects.json");

    let state = AppState {
        nodepilot_dir: nodepilot_dir.clone(),
        manager,
        config_path,
        projects_path,
        servers: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        log_buffers: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_versions,
            commands::refresh_versions,
            commands::install_version,
            commands::activate_version,
            commands::delete_version,
            commands::auto_setup,
            commands::rollback_setup,
            commands::is_auto_setup_done,
            commands::get_setup_error,
            commands::mark_setup_skipped,
            commands::get_config,
            commands::set_config,
            commands::bind_project,
            commands::get_project_bindings,
            commands::unbind_project,
            commands::update_project_name,
            commands::update_project_config,
            commands::read_package_json,
            commands::detect_pm,
            commands::start_dev_server,
            commands::stop_dev_server,
            commands::get_dev_server_logs,
            commands::list_git_branches,
            commands::checkout_branch,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            {
                let mut autostart = tauri_plugin_autostart::Builder::default();
                #[cfg(target_os = "macos")]
                {
                    autostart = autostart.macos_launcher(MacosLauncher::LaunchAgent);
                }
                app.handle().plugin(autostart.build())?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;

            // Auto environment setup on first launch – try once silently.
            // If it fails, the frontend will show a dialog after the webview is ready.
            {
                let nodepilot_dir = &app.state::<AppState>().nodepilot_dir;
                if !env_setup::is_setup_done(nodepilot_dir) {
                    if let Err(e) = env_setup::setup(nodepilot_dir) {
                        // Store error so the frontend can retrieve it and prompt the user
                        let error_path = nodepilot_dir.join(".auto-setup-error");
                        let _ = std::fs::write(&error_path, e.to_string());
                        log::warn!("auto environment setup failed: {e}");
                    }
                }
            }

            let current_version = app
                .state::<AppState>()
                .manager
                .get_current_version()
                .unwrap_or_else(|| "node".to_string());

            let tray_icon = tray::generate_icon(&current_version);

            #[cfg(not(debug_assertions))]
            {
                let _ = app.handle().plugin(
                    tauri_plugin_updater::Builder::new().build(),
                );
            }

            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("nodepilot")
                .icon(tray_icon)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<commands::AppState>();
                commands::cleanup_all_servers(state.inner());
            }
        });
}
