mod commands;
mod state;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Manager, RunEvent};

pub fn run() {
    let state = AppState::initialize().expect("codex-route desktop state should initialize");
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_started_for_run = Arc::clone(&shutdown_started);

    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::set_current_provider,
            commands::list_route_rules,
            commands::upsert_route_rule,
            commands::remove_route_rule,
            commands::get_lifecycle_status,
            commands::activate_route,
            commands::deactivate_route,
        ])
        .build(tauri::generate_context!())
        .expect("error while building codex-route desktop application");

    app.run(move |app_handle, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
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
        });
}
