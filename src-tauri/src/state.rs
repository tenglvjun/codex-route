use codex_route::config::ScanConfig;
use codex_route::lifecycle::{EmbeddedRouteService, LifecycleError, LifecyclePaths};
use codex_route::provider_store::ProviderStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ProviderStore>,
    pub route: Arc<Mutex<EmbeddedRouteService>>,
}

impl AppState {
    pub fn initialize() -> Result<Self, String> {
        let scan_config = ScanConfig::from_cli(None, None).map_err(|error| error.to_string())?;
        let data_dir = default_data_dir()?;
        let store = Arc::new(
            ProviderStore::open(data_dir.join("codex-route.db"))
                .map_err(|error| error.to_string())?,
        );
        let paths = LifecyclePaths::new(data_dir, scan_config.codex_home.clone());
        let route = EmbeddedRouteService::new(
            paths,
            store.clone(),
            scan_config,
            None,
            codex_route::route::DEFAULT_ROUTE_PORT,
        );
        Ok(Self {
            store,
            route: Arc::new(Mutex::new(route)),
        })
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let mut route = self.route.lock().await;
        match route.deactivate().await {
            Ok(_) | Err(LifecycleError::NotActive) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn default_data_dir() -> Result<PathBuf, String> {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .map(|path| path.join("codex-route"))
        .ok_or_else(|| "cannot determine a configuration directory".to_string())
}
