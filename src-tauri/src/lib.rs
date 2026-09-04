mod commands;
mod state;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, RunEvent, Runtime, WebviewWindow};

const MAIN_WINDOW_LABEL: &str = "main";

fn get_or_create_main_window<R: Runtime>(app_handle: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        return Some(window);
    }

    let config = app_handle
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW_LABEL)?
        .clone();

    match tauri::WebviewWindowBuilder::from_config(app_handle, &config)
        .and_then(|builder| builder.build())
    {
        Ok(window) => Some(window),
        Err(error) => {
            eprintln!("failed to recreate the main window: {error}");
            None
        }
    }
}

fn present_main_window<R: Runtime>(app_handle: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    if let Err(error) = app_handle.show() {
        eprintln!("failed to activate Codex Route: {error}");
    }

    let Some(window) = get_or_create_main_window(app_handle) else {
        eprintln!("the main window is not configured");
        return;
    };

    if let Err(error) = window.unminimize() {
        eprintln!("failed to restore the main window: {error}");
    }
    if let Err(error) = window.show() {
        eprintln!("failed to show the main window: {error}");
    }
    if let Err(error) = window.set_focus() {
        eprintln!("failed to focus the main window: {error}");
    }
}

pub fn run() {
    let state = AppState::initialize().expect("codex-route desktop state should initialize");
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_started_for_run = Arc::clone(&shutdown_started);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::set_current_provider,
            commands::scan_cc_switch_providers,
            commands::import_cc_switch_providers,
            commands::list_route_rules,
            commands::upsert_route_rule,
            commands::remove_route_rule,
            commands::get_lifecycle_status,
            commands::activate_route,
            commands::deactivate_route,
        ])
        .build(tauri::generate_context!())
        .expect("error while building codex-route desktop application");

    app.run(move |app_handle, event| match event {
        RunEvent::Ready => present_main_window(app_handle),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => present_main_window(app_handle),
        RunEvent::ExitRequested { api, .. } => {
            if shutdown_started_for_run.swap(true, Ordering::SeqCst) {
                return;
            }
            api.prevent_exit();
            let state = app_handle
                .try_state::<AppState>()
                .map(|state| state.inner().clone());
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(state) = state {
                    if let Err(error) = state.shutdown().await {
                        eprintln!("failed to clean up route before exit: {error}");
                    }
                }
                app_handle.exit(0);
            });
        }
        _ => {}
    });
}
