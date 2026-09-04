use crate::client_snapshot::ClientSnapshot;
use crate::diagnostics::DiagnosticSeverity;
use crate::logging;
use std::collections::BTreeMap;
use tauri::menu::{Menu, MenuBuilder, MenuItem};
use tauri::{AppHandle, Manager, Runtime};

pub const SHOW_WINDOW_ID: &str = "show-window";
pub const ROUTE_ACTION_ID: &str = "route-action";
pub const PROVIDER_STATUS_ID: &str = "provider-status";
pub const QUIT_ID: &str = "quit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuModel {
    pub show_window: String,
    pub route_status: String,
    pub route_enabled: bool,
    pub provider_status: String,
    pub provider_enabled: bool,
    pub quit: String,
}

impl TrayMenuModel {
    pub fn from_snapshot(snapshot: &ClientSnapshot) -> Self {
        let route_status = if snapshot.runtime.active {
            "Stop Route".to_string()
        } else if snapshot.providers.is_empty() {
            "Route unavailable".to_string()
        } else {
            "Start Route".to_string()
        };
        let provider_status = snapshot
            .provider
            .as_ref()
            .map(|provider| format!("Provider: {}", provider.name))
            .unwrap_or_else(|| "Provider: none".to_string());
        Self {
            show_window: "Show Codex Route".to_string(),
            route_enabled: !matches!(
                snapshot.runtime.phase,
                crate::runtime::RuntimePhase::BlockedExternalModification
                    | crate::runtime::RuntimePhase::Starting
                    | crate::runtime::RuntimePhase::Recovering
            ) && (!snapshot.providers.is_empty() || snapshot.runtime.active),
            route_status,
            provider_enabled: snapshot.provider.is_some(),
            provider_status,
            quit: "Quit Codex Route".to_string(),
        }
    }
}

pub fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &ClientSnapshot,
) -> tauri::Result<Menu<R>> {
    let model = TrayMenuModel::from_snapshot(snapshot);
    let show = MenuItem::with_id(app, SHOW_WINDOW_ID, model.show_window, true, None::<&str>)?;
    let route = MenuItem::with_id(
        app,
        ROUTE_ACTION_ID,
        model.route_status,
        model.route_enabled,
        None::<&str>,
    )?;
    let provider = MenuItem::with_id(
        app,
        PROVIDER_STATUS_ID,
        model.provider_status,
        model.provider_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_ID, model.quit, true, None::<&str>)?;
    MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&route)
        .item(&provider)
        .separator()
        .item(&quit)
        .build()
}

pub fn install<R: Runtime>(app: &AppHandle<R>, snapshot: &ClientSnapshot) -> tauri::Result<()> {
    let menu = build_menu(app, snapshot)?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default application icon".into()))?;
    tauri::tray::TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip("Codex Route")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_WINDOW_ID => super::present_main_window(app),
            ROUTE_ACTION_ID => {
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    let state = state.inner().clone();
                    tauri::async_runtime::spawn(async move {
                        let snapshot = state.runtime.snapshot().await;
                        if snapshot.active {
                            let _ = state.runtime.stop().await;
                        } else {
                            let mut settings = state.settings.read().await.clone();
                            settings.startup_consent_granted = true;
                            if let Err(error) = state.update_settings(settings).await {
                                logging::record(
                                    &state,
                                    DiagnosticSeverity::Error,
                                    "runtime.tray_start_failed",
                                    error,
                                    "tray",
                                    BTreeMap::new(),
                                    &[],
                                )
                                .await;
                                return;
                            }
                            if let Err(error) = state.runtime.ensure_running(None, None).await {
                                logging::record(
                                    &state,
                                    DiagnosticSeverity::Error,
                                    "runtime.tray_start_failed",
                                    error.to_string(),
                                    "tray",
                                    BTreeMap::new(),
                                    &[],
                                )
                                .await;
                            }
                        }
                    });
                }
            }
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TrayMenuModel;
    use crate::client_snapshot::{ClientSnapshot, CodexStatus, DiagnosticsSummary};
    use crate::runtime::{RuntimePhase, RuntimeSnapshot};
    use codex_route::provider::ProviderSummary;
    use std::path::PathBuf;

    fn snapshot(phase: RuntimePhase, active: bool, providers: Vec<ProviderSummary>) -> ClientSnapshot {
        ClientSnapshot {
            schema_version: 1,
            sequence: 0,
            generated_at: 1,
            codex: CodexStatus {
                home: PathBuf::from("/tmp/.codex"),
                config_path: PathBuf::from("/tmp/.codex/config.toml"),
                installed: true,
                version: None,
                config_exists: true,
                config_managed: false,
                external_modification: false,
            },
            workspace: None,
            provider: providers.first().cloned(),
            providers,
            rules: Vec::new(),
            runtime: RuntimeSnapshot { phase, active, ..RuntimeSnapshot::default() },
            diagnostics: DiagnosticsSummary::default(),
        }
    }

    #[test]
    fn running_menu_exposes_stop_and_current_provider() {
        let provider = ProviderSummary {
            id: "a".into(),
            name: "Provider A".into(),
            category: None,
            source: "local".into(),
            is_current: true,
        };
        let model = TrayMenuModel::from_snapshot(&snapshot(RuntimePhase::Running, true, vec![provider]));
        assert_eq!(model.route_status, "Stop Route");
        assert_eq!(model.provider_status, "Provider: Provider A");
        assert!(model.route_enabled);
    }

    #[test]
    fn degraded_without_provider_disables_route_action() {
        let model = TrayMenuModel::from_snapshot(&snapshot(RuntimePhase::Degraded, false, vec![]));
        assert_eq!(model.route_status, "Route unavailable");
        assert!(!model.route_enabled);
        assert!(!model.provider_enabled);
    }
}
