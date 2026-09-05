use crate::client_snapshot::ClientSnapshot;
use crate::diagnostics::DiagnosticSeverity;
use crate::locale::{resolve_language, system_locale, ResolvedLocale};
use crate::logging;
use crate::state::LanguagePreference;
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItem};
#[cfg(not(target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
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
    pub fn empty(language: LanguagePreference) -> Self {
        let locale = resolve_language(language, system_locale().as_deref());
        Self {
            show_window: show_window_label(locale),
            route_status: route_unavailable_label(locale),
            route_enabled: false,
            provider_status: provider_none_label(locale),
            provider_enabled: false,
            quit: quit_label(locale),
        }
    }

    pub fn from_snapshot(snapshot: &ClientSnapshot, language: LanguagePreference) -> Self {
        let locale = resolve_language(language, system_locale().as_deref());
        let route_status = if snapshot.runtime.active {
            stop_route_label(locale)
        } else if snapshot.providers.is_empty() {
            route_unavailable_label(locale)
        } else {
            start_route_label(locale)
        };
        let provider_status = snapshot
            .provider
            .as_ref()
            .map(|provider| provider_label(locale, &provider.name))
            .unwrap_or_else(|| provider_none_label(locale));
        Self {
            show_window: show_window_label(locale),
            route_enabled: !matches!(
                snapshot.runtime.phase,
                crate::runtime::RuntimePhase::BlockedExternalModification
                    | crate::runtime::RuntimePhase::Starting
                    | crate::runtime::RuntimePhase::Recovering
            ) && (!snapshot.providers.is_empty() || snapshot.runtime.active),
            route_status,
            provider_enabled: snapshot.provider.is_some(),
            provider_status,
            quit: quit_label(locale),
        }
    }
}

fn show_window_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "显示 Codex Route",
        ResolvedLocale::ZhTw => "顯示 Codex Route",
        ResolvedLocale::En => "Show Codex Route",
    }
    .to_string()
}

fn route_unavailable_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "Route 不可用",
        ResolvedLocale::ZhTw => "Route 無法使用",
        ResolvedLocale::En => "Route unavailable",
    }
    .to_string()
}

fn start_route_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "启动 Route",
        ResolvedLocale::ZhTw => "啟動 Route",
        ResolvedLocale::En => "Start Route",
    }
    .to_string()
}

fn stop_route_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "停止 Route",
        ResolvedLocale::ZhTw => "停止 Route",
        ResolvedLocale::En => "Stop Route",
    }
    .to_string()
}

fn provider_none_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "提供商：无",
        ResolvedLocale::ZhTw => "提供商：無",
        ResolvedLocale::En => "Provider: none",
    }
    .to_string()
}

fn provider_label(locale: ResolvedLocale, name: &str) -> String {
    match locale {
        ResolvedLocale::ZhCn => format!("提供商：{name}"),
        ResolvedLocale::ZhTw => format!("提供商：{name}"),
        ResolvedLocale::En => format!("Provider: {name}"),
    }
}

fn quit_label(locale: ResolvedLocale) -> String {
    match locale {
        ResolvedLocale::ZhCn => "退出 Codex Route",
        ResolvedLocale::ZhTw => "結束 Codex Route",
        ResolvedLocale::En => "Quit Codex Route",
    }
    .to_string()
}

#[cfg(target_os = "macos")]
fn macos_tray_icon() -> tauri::Result<Image<'static>> {
    const ICON_BYTES: &[u8] = include_bytes!("../icons/tray/macos/statusbar_template_3x.png");
    Image::from_bytes(ICON_BYTES)
}

pub fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: Option<&ClientSnapshot>,
) -> tauri::Result<Menu<R>> {
    let language = app
        .try_state::<crate::state::AppState>()
        .and_then(|state| {
            state
                .settings
                .try_read()
                .ok()
                .map(|settings| settings.language)
        })
        .unwrap_or_default();
    let model = snapshot
        .map(|snapshot| TrayMenuModel::from_snapshot(snapshot, language))
        .unwrap_or_else(|| TrayMenuModel::empty(language));
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

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app, None)?;

    #[cfg(target_os = "macos")]
    let icon = macos_tray_icon()?;

    #[cfg(not(target_os = "macos"))]
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default application icon".into()))?;
    let mut builder = tauri::tray::TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip("Codex Route")
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
                            let port = settings.port;
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
                            if let Err(error) = state.runtime.ensure_running(None, Some(port)).await
                            {
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
        });

    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true).show_menu_on_left_click(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        builder = builder
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    super::present_main_window(tray.app_handle());
                }
            });
    }

    builder.build(app)?;
    Ok(())
}

pub fn update_menu<R: Runtime>(app: &AppHandle<R>, snapshot: &ClientSnapshot) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id("main") else {
        return Err(tauri::Error::AssetNotFound("main tray icon".into()));
    };
    let menu = build_menu(app, Some(snapshot))?;
    tray.set_menu(Some(menu))
}

#[cfg(test)]
mod tests {
    use super::TrayMenuModel;
    use crate::client_snapshot::{ClientSnapshot, CodexStatus, DiagnosticsSummary};
    use crate::runtime::{RuntimePhase, RuntimeSnapshot};
    use crate::state::LanguagePreference;
    use codex_route::provider::ProviderSummary;
    use std::path::PathBuf;

    fn snapshot(
        phase: RuntimePhase,
        active: bool,
        providers: Vec<ProviderSummary>,
    ) -> ClientSnapshot {
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
            workspaces: Vec::new(),
            workspace: None,
            provider: providers.first().cloned(),
            providers,
            rules: Vec::new(),
            runtime: RuntimeSnapshot {
                phase,
                active,
                ..RuntimeSnapshot::default()
            },
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
        let model = TrayMenuModel::from_snapshot(
            &snapshot(RuntimePhase::Running, true, vec![provider]),
            LanguagePreference::En,
        );
        assert_eq!(model.route_status, "Stop Route");
        assert_eq!(model.provider_status, "Provider: Provider A");
        assert!(model.route_enabled);
    }

    #[test]
    fn empty_menu_has_safe_disabled_actions() {
        let model = TrayMenuModel::empty(LanguagePreference::En);
        assert_eq!(model.route_status, "Route unavailable");
        assert_eq!(model.provider_status, "Provider: none");
        assert!(!model.route_enabled);
        assert!(!model.provider_enabled);
    }

    #[test]
    fn degraded_without_provider_disables_route_action() {
        let model = TrayMenuModel::from_snapshot(
            &snapshot(RuntimePhase::Degraded, false, vec![]),
            LanguagePreference::En,
        );
        assert_eq!(model.route_status, "Route unavailable");
        assert!(!model.route_enabled);
        assert!(!model.provider_enabled);
    }

    #[test]
    fn menu_labels_follow_explicit_locale() {
        let model = TrayMenuModel::from_snapshot(
            &snapshot(
                RuntimePhase::Running,
                true,
                vec![ProviderSummary {
                    id: "a".into(),
                    name: "Provider A".into(),
                    category: None,
                    source: "local".into(),
                    is_current: true,
                }],
            ),
            LanguagePreference::ZhCn,
        );
        assert_eq!(model.show_window, "显示 Codex Route");
        assert_eq!(model.route_status, "停止 Route");
        assert_eq!(model.provider_status, "提供商：Provider A");
        assert_eq!(model.quit, "退出 Codex Route");
    }
}
