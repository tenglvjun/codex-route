use crate::client_snapshot::ClientSnapshot;
use crate::diagnostics::DiagnosticSeverity;
use crate::logging;
use crate::state::{AppState, ClientSettings};
use codex_route::cc_switch_import::{CcSwitchImporter, ImportScan};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};

pub fn should_auto_start(settings: &ClientSettings) -> bool {
    settings.auto_start && settings.startup_consent_granted
}

pub struct ClientCoordinator<R: Runtime> {
    state: Arc<AppState>,
    app: AppHandle<R>,
}

impl<R: Runtime> ClientCoordinator<R> {
    pub fn new(state: Arc<AppState>, app: AppHandle<R>) -> Self {
        Self { state, app }
    }

    pub async fn build_snapshot(&self) -> Result<ClientSnapshot, String> {
        crate::commands::build_client_snapshot(&self.state).await
    }

    pub fn start(self) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move { self.run().await })
    }

    async fn run(self) {
        self.state
            .runtime
            .attach_diagnostics(Arc::clone(&self.state.diagnostics))
            .await;

        let scan = tauri::async_runtime::spawn_blocking(|| {
            CcSwitchImporter::new(CcSwitchImporter::discover_default_db()).read_codex_providers()
        })
        .await;
        match scan {
            Ok(Ok(ImportScan { candidates, .. })) => {
                logging::record(
                    &self.state,
                    DiagnosticSeverity::Info,
                    "provider.cc_switch_scan_ready",
                    format!(
                        "Discovered {} importable cc-switch providers",
                        candidates.len()
                    ),
                    "coordinator",
                    BTreeMap::new(),
                    &[],
                )
                .await;
            }
            Ok(Err(error)) => {
                logging::record(
                    &self.state,
                    DiagnosticSeverity::Warning,
                    "provider.cc_switch_unavailable",
                    error.to_string(),
                    "coordinator",
                    BTreeMap::new(),
                    &[],
                )
                .await;
            }
            Err(error) => log::warn!(target: "coordinator", "cc-switch scan task failed: {error}"),
        }

        let settings = self.state.settings.read().await.clone();
        if should_auto_start(&settings) {
            if let Err(error) = self
                .state
                .runtime
                .ensure_running(None, Some(settings.port))
                .await
            {
                let severity =
                    if matches!(error, crate::runtime::RuntimeError::ExternalModification) {
                        DiagnosticSeverity::Warning
                    } else {
                        DiagnosticSeverity::Error
                    };
                logging::record(
                    &self.state,
                    severity,
                    "runtime.start_failed",
                    error.to_string(),
                    "coordinator",
                    BTreeMap::new(),
                    &[],
                )
                .await;
            }
        } else if !settings.startup_consent_granted {
            self.state
                .runtime
                .mark_degraded("Automatic Route startup needs your approval")
                .await;
            logging::record(
                &self.state,
                DiagnosticSeverity::Info,
                "runtime.startup_consent_required",
                "Automatic Route startup needs your approval",
                "coordinator",
                BTreeMap::new(),
                &[],
            )
            .await;
        }
        self.state.runtime.start_health_monitor().await;
        if let Ok(snapshot) = self.build_snapshot().await {
            let _ = self.app.emit("client-snapshot-updated", snapshot);
        }
    }
}

pub fn start<R: Runtime>(
    state: Arc<AppState>,
    app: AppHandle<R>,
) -> tauri::async_runtime::JoinHandle<()> {
    ClientCoordinator::new(state, app).start()
}

#[cfg(test)]
mod tests {
    use super::should_auto_start;
    use crate::state::ClientSettings;

    #[test]
    fn auto_start_requires_explicit_consent_and_setting() {
        assert!(!should_auto_start(&ClientSettings::default()));
        assert!(should_auto_start(&ClientSettings {
            auto_start: true,
            startup_consent_granted: true,
            port: codex_route::route::DEFAULT_ROUTE_PORT,
            ..ClientSettings::default()
        }));
        assert!(!should_auto_start(&ClientSettings {
            auto_start: false,
            startup_consent_granted: true,
            port: codex_route::route::DEFAULT_ROUTE_PORT,
            ..ClientSettings::default()
        }));
    }
}
