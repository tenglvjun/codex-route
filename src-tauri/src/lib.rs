mod commands;
mod client_snapshot;
mod coordinator;
mod diagnostics;
mod logging;
mod runtime;
mod state;
mod tray;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, WebviewWindow, WindowEvent};

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

fn start_event_bridge<R: Runtime>(app_handle: AppHandle<R>, state: AppState) {
    let mut runtime_events = state.runtime.subscribe();
    let mut diagnostics = state.diagnostics.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                event = runtime_events.recv() => match event {
                    Ok(event) => {
                        let (sequence, generated_at) = match &event {
                            crate::runtime::RuntimeEvent::StatusChanged { snapshot } => {
                                (snapshot.sequence, snapshot.updated_at)
                            }
                        };
                        let payload = serde_json::json!({
                            "sequence": sequence,
                            "generatedAt": generated_at,
                            "event": event,
                        });
                        let _ = app_handle.emit("runtime-status-changed", payload);
                        if let Ok(snapshot) = commands::build_client_snapshot(&state).await {
                            if let Some(tray) = app_handle.tray_by_id("main") {
                                if let Ok(menu) = tray::build_menu(&app_handle, &snapshot) {
                                    let _ = tray.set_menu(Some(menu));
                                }
                                let _ = tray.set_tooltip(Some(format!(
                                    "Codex Route · {}",
                                    snapshot.runtime.phase.phase_label()
                                )));
                            }
                            let _ = app_handle.emit("client-snapshot-updated", snapshot);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                event = diagnostics.recv() => match event {
                    Ok(record) => {
                        let payload = serde_json::json!({
                            "sequence": record.id,
                            "generatedAt": record.timestamp,
                            "record": record,
                        });
                        let _ = app_handle.emit("diagnostic-added", payload);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
            }
        }
    });
}

trait RuntimePhaseLabel {
    fn phase_label(&self) -> &'static str;
}

impl RuntimePhaseLabel for crate::runtime::RuntimePhase {
    fn phase_label(&self) -> &'static str {
        match self {
            crate::runtime::RuntimePhase::Stopped => "stopped",
            crate::runtime::RuntimePhase::Starting => "starting",
            crate::runtime::RuntimePhase::Running => "running",
            crate::runtime::RuntimePhase::Degraded => "degraded",
            crate::runtime::RuntimePhase::Recovering => "recovering",
            crate::runtime::RuntimePhase::BlockedExternalModification => "protected",
            crate::runtime::RuntimePhase::Failed => "failed",
        }
    }
}

pub fn run() {
    let state = AppState::initialize().expect("codex-route desktop state should initialize");
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_started_for_run = Arc::clone(&shutdown_started);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            present_main_window(app);
            let _ = app.emit(
                "second-instance-opened",
                serde_json::json!({ "args": args, "cwd": cwd }),
            );
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("codex-route".into()),
                    }),
                ])
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(4))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            let state = app.state::<AppState>().inner().clone();
            start_event_bridge(app.handle().clone(), state.clone());
            coordinator::start(Arc::new(state), app.handle().clone());
            let handle = app.handle().clone();
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(snapshot) = commands::build_client_snapshot(&state).await {
                    if let Err(error) = tray::install(&handle, &snapshot) {
                        log::warn!(target: "desktop", "failed to install tray: {error}");
                    }
                }
            });
            Ok(())
        })
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
            commands::get_client_snapshot,
            commands::get_client_settings,
            commands::set_client_settings,
            commands::start_runtime,
            commands::stop_runtime,
            commands::set_workspace_provider,
            commands::get_diagnostics,
            commands::clear_diagnostics,
        ])
        .build(tauri::generate_context!())
        .expect("error while building codex-route desktop application");

    app.run(move |app_handle, event| match event {
        RunEvent::Ready => present_main_window(app_handle),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => present_main_window(app_handle),
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } if label == MAIN_WINDOW_LABEL => {
            api.prevent_close();
            if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.hide();
            }
        }
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
