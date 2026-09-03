use crate::state::AppState;
use codex_route::lifecycle::{ActivationResult, DeactivationResult, LifecycleStatus};
use codex_route::provider::ProviderSummary;
use codex_route::workspace_rule::WorkspaceRouteRule;
use serde::Deserialize;
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
pub async fn list_route_rules(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceRouteRule>, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.list_route_rules().map_err(|e| e.to_string()))
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
pub async fn get_lifecycle_status(
    state: State<'_, AppState>,
) -> Result<LifecycleStatus, String> {
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
pub async fn deactivate_route(
    state: State<'_, AppState>,
) -> Result<DeactivationResult, String> {
    let mut route = state.route.lock().await;
    route.deactivate().await.map_err(|error| error.to_string())
}
