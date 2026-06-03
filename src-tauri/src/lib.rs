mod commands;
mod tray;
mod version;

use std::sync::{Arc, Mutex};

use commands::AppState;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

#[cfg(not(debug_assertions))]
use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home = dirs::home_dir().unwrap_or_default();
    let nodepilot_dir = home.join(".nodepilot");
    let cache_dir = nodepilot_dir.join("cache");
    let versions_dir = nodepilot_dir.join("versions");
    let setup_flag = nodepilot_dir.join(".setup-done");

    let activator =
        version::VersionActivator::new(nodepilot_dir.clone(), versions_dir.clone());

    let config_path = nodepilot_dir.join("config.json");

    let source_url = if config_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&config_path) {
            if let Ok(cfg) = serde_json::from_str::<commands::AppConfig>(&data) {
                cfg.mirror_url.unwrap_or_else(|| "https://nodejs.org/dist/index.json".to_string())
            } else {
                "https://nodejs.org/dist/index.json".to_string()
            }
        } else {
            "https://nodejs.org/dist/index.json".to_string()
        }
    } else {
        "https://nodejs.org/dist/index.json".to_string()
    };

    let mut fetcher = version::VersionFetcher::new(cache_dir);
    fetcher.set_source_url(source_url.clone());

    let mut installer = version::VersionInstaller::new(versions_dir.clone());
    installer.set_source_url(if source_url.ends_with('/') || source_url.contains(".json") {
        let trimmed = source_url.trim_end_matches("index.json").trim_end_matches('/');
        format!("{}/", trimmed)
    } else {
        source_url.clone() + "/"
    });

    let state = Arc::new(AppState {
        fetcher,
        installer,
        activator,
        deleter: version::VersionDeleter::new(versions_dir.clone(), nodepilot_dir.clone()),
        setup_flag,
        config_path,
        source_url: Mutex::new(source_url),
    });

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_versions,
            commands::refresh_versions,
            commands::install_version,
            commands::activate_version,
            commands::delete_version,
            commands::is_first_run,
            commands::mark_setup_done,
            commands::get_config,
            commands::set_config,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            app.handle().plugin(
                tauri_plugin_autostart::Builder::default()
                    .macos_launcher(MacosLauncher::LaunchAgent)
                    .build(),
            )?;

            let current_version = app
                .state::<Arc<AppState>>()
                .activator
                .get_current_version()
                .unwrap_or_else(|| "node".to_string());

            let tray_icon = tray::generate_icon(&current_version);

            #[cfg(not(debug_assertions))]
            {
                let _ = app.handle().plugin(
                    tauri_plugin_updater::Builder::new()
                        .build(),
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
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            let window = app.get_webview_window("main").unwrap();

            #[cfg(not(debug_assertions))]
            {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = window_clone.hide();
                    }
                });
            }
            #[cfg(debug_assertions)]
            {
                let _ = window.show();
            }
            #[cfg(not(debug_assertions))]
            {
                let _ = window.hide();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
