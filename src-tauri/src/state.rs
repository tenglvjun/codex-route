use codex_route::config::ScanConfig;
use codex_route::lifecycle::{EmbeddedRouteService, LifecyclePaths};
use codex_route::provider_store::ProviderStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::diagnostics::DiagnosticsStore;
use crate::runtime::RouteSupervisor;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ProviderStore>,
    pub runtime: Arc<RouteSupervisor>,
    pub scan_config: ScanConfig,
    pub settings: Arc<RwLock<ClientSettings>>,
    pub data_dir: PathBuf,
    pub diagnostics: Arc<DiagnosticsStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSettings {
    pub auto_start: bool,
    pub startup_consent_granted: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            auto_start: true,
            startup_consent_granted: false,
        }
    }
}

impl AppState {
    pub fn initialize() -> Result<Self, String> {
        let scan_config = ScanConfig::from_cli(None, None).map_err(|error| error.to_string())?;
        let data_dir = default_data_dir()?;
        let store = Arc::new(
            ProviderStore::open(data_dir.join("codex-route.db"))
                .map_err(|error| error.to_string())?,
        );
        let paths = LifecyclePaths::new(data_dir.clone(), scan_config.codex_home.clone());
        let route = EmbeddedRouteService::new(
            paths,
            store.clone(),
            scan_config.clone(),
            None,
            codex_route::route::DEFAULT_ROUTE_PORT,
        );
        let route = Arc::new(Mutex::new(route));
        let runtime = Arc::new(RouteSupervisor::new(route.clone()));
        let settings = Arc::new(RwLock::new(load_settings(&data_dir)));
        let diagnostics = Arc::new(DiagnosticsStore::new());
        Ok(Self {
            store,
            runtime,
            scan_config,
            settings,
            data_dir,
            diagnostics,
        })
    }

    pub async fn update_settings(
        &self,
        settings: ClientSettings,
    ) -> Result<ClientSettings, String> {
        let previous = self.settings.read().await.clone();
        let contents = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
        let path = self.data_dir.join("client-settings.json");
        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, contents).map_err(|error| error.to_string())?;
        fs::rename(&temp_path, &path).map_err(|error| error.to_string())?;
        *self.settings.write().await = settings.clone();
        if previous.auto_start && !settings.auto_start {
            self.runtime
                .stop()
                .await
                .map_err(|error| error.to_string())?;
        } else if !previous.auto_start && crate::coordinator::should_auto_start(&settings) {
            self.runtime
                .ensure_running(None, None)
                .await
                .map_err(|error| error.to_string())?;
            self.runtime.start_health_monitor().await;
        }
        Ok(settings)
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        match self.runtime.stop().await {
            Ok(_) => Ok(()),
            Err(crate::runtime::RuntimeError::NotActive) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn for_test(data_dir: PathBuf, codex_home: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
        let scan_config = ScanConfig {
            codex_home,
            max_rollout_bytes: codex_route::config::DEFAULT_MAX_ROLLOUT_BYTES,
        };
        let store = Arc::new(
            ProviderStore::open(data_dir.join("codex-route.db"))
                .map_err(|error| error.to_string())?,
        );
        let route = Arc::new(Mutex::new(EmbeddedRouteService::new(
            LifecyclePaths::new(data_dir.clone(), scan_config.codex_home.clone()),
            Arc::clone(&store),
            scan_config.clone(),
            None,
            codex_route::route::DEFAULT_ROUTE_PORT,
        )));
        Ok(Self {
            store,
            runtime: Arc::new(RouteSupervisor::new(route)),
            scan_config,
            settings: Arc::new(RwLock::new(ClientSettings::default())),
            data_dir,
            diagnostics: Arc::new(DiagnosticsStore::new()),
        })
    }
}

fn load_settings(data_dir: &std::path::Path) -> ClientSettings {
    let path = data_dir.join("client-settings.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

fn default_data_dir() -> Result<PathBuf, String> {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .map(|path| path.join("codex-route"))
        .ok_or_else(|| "cannot determine a configuration directory".to_string())
}
