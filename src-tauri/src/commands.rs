use crate::state::AppState;
use codex_route::cc_switch_import::{
    CcSwitchImporter, ConflictPolicy, ImportReport, RejectedProvider,
};
use codex_route::lifecycle::{ActivationResult, DeactivationResult, LifecycleStatus};
use codex_route::provider::ProviderSummary;
use codex_route::workspace_rule::WorkspaceRouteRule;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivateRouteRequest {
    pub provider_id: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertRouteRuleRequest {
    pub workspace: PathBuf,
    pub provider_id: String,
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCcSwitchRequest {
    pub provider_ids: Vec<String>,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchProviderCandidate {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub already_imported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchScanReport {
    pub source: PathBuf,
    pub providers: Vec<CcSwitchProviderCandidate>,
    pub rejected: Vec<RejectedProvider>,
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderSummary>, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .list()
            .map(|providers| providers.iter().map(ProviderSummary::from).collect())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider command failed: {error}"))?
}

#[tauri::command]
pub async fn set_current_provider(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderSummary, String> {
    let provider_id = provider_id.trim().to_string();
    if provider_id.is_empty() {
        return Err("providerId must not be empty".to_string());
    }
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .set_current(&provider_id)
            .map(|provider| ProviderSummary::from(&provider))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider command failed: {error}"))?
}

#[tauri::command]
pub async fn scan_cc_switch_providers(
    state: State<'_, AppState>,
) -> Result<CcSwitchScanReport, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let scan = CcSwitchImporter::new(CcSwitchImporter::discover_default_db())
            .read_codex_providers()
            .map_err(|error| error.to_string())?;
        let providers = scan
            .candidates
            .iter()
            .map(|candidate| {
                let source_id = candidate
                    .provider
                    .source
                    .source_id()
                    .expect("cc-switch candidates always have a source ID");
                let already_imported = store
                    .find_imported(source_id)
                    .map_err(|error| error.to_string())?
                    .is_some();
                Ok(CcSwitchProviderCandidate {
                    id: source_id.to_string(),
                    name: candidate.provider.name.clone(),
                    category: candidate.provider.category.clone(),
                    already_imported,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(CcSwitchScanReport {
            source: scan.source,
            providers,
            rejected: scan.rejected,
        })
    })
    .await
    .map_err(|error| format!("provider scan command failed: {error}"))?
}

#[tauri::command]
pub async fn import_cc_switch_providers(
    state: State<'_, AppState>,
    request: ImportCcSwitchRequest,
) -> Result<ImportReport, String> {
    let provider_ids: Vec<String> = request
        .provider_ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    if provider_ids.is_empty() {
        return Err("providerIds must contain at least one provider".to_string());
    }

    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let scan = CcSwitchImporter::new(CcSwitchImporter::discover_default_db())
            .read_codex_providers()
            .map_err(|error| error.to_string())?;
        let selected = scan
            .select_provider_ids(&provider_ids)
            .map_err(|error| error.to_string())?;
        store
            .import_scan_transaction(&selected, request.conflict_policy)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider import command failed: {error}"))?
}

#[tauri::command]
pub async fn list_route_rules(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceRouteRule>, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store.list_route_rules().map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| format!("route rule command failed: {error}"))?
}

#[tauri::command]
pub async fn upsert_route_rule(
    state: State<'_, AppState>,
    request: UpsertRouteRuleRequest,
) -> Result<WorkspaceRouteRule, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let provider_id = request.provider_id.trim();
        if provider_id.is_empty() {
            return Err("providerId must not be empty".to_string());
        }
        store
            .upsert_route_rule(&request.workspace, provider_id, request.replace)
            .map_err(|error| error.to_string())?;
        store
            .get_route_rule(&request.workspace)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "route rule was not available after update".to_string())
    })
    .await
    .map_err(|error| format!("route rule command failed: {error}"))?
}

#[tauri::command]
pub async fn remove_route_rule(
    state: State<'_, AppState>,
    workspace: PathBuf,
) -> Result<WorkspaceRouteRule, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .remove_route_rule(&workspace)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("route rule command failed: {error}"))?
}

#[tauri::command]
pub async fn get_lifecycle_status(state: State<'_, AppState>) -> Result<LifecycleStatus, String> {
    state
        .route
        .lock()
        .await
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn activate_route(
    state: State<'_, AppState>,
    request: ActivateRouteRequest,
) -> Result<ActivationResult, String> {
    let mut route = state.route.lock().await;
    route
        .activate_with(request.provider_id, request.port)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn deactivate_route(state: State<'_, AppState>) -> Result<DeactivationResult, String> {
    let mut route = state.route.lock().await;
    route.deactivate().await.map_err(|error| error.to_string())
}
