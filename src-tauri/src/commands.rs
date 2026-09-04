use crate::state::AppState;
use crate::client_snapshot::{ClientSnapshot, DiagnosticsSummary};
use crate::diagnostics::DiagnosticSeverity;
use crate::logging;
use crate::state::ClientSettings;
use codex_route::cc_switch_import::{
    CcSwitchImporter, ConflictPolicy, ImportReport, RejectedProvider,
};
use codex_route::lifecycle::{ActivationResult, DeactivationResult, LifecycleStatus};
use codex_route::provider::ProviderSummary;
use codex_route::workspace_rule::WorkspaceRouteRule;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::BTreeMap;
use tauri::{AppHandle, Emitter, State};

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkspaceProviderRequest {
    pub workspace: PathBuf,
    pub provider_id: String,
}

pub(crate) async fn build_client_snapshot(state: &AppState) -> Result<ClientSnapshot, String> {
    let runtime = state.runtime.snapshot().await;
    let diagnostics = state.diagnostics.recent(200).await;
    let store = Arc::clone(&state.store);
    let scan_config = state.scan_config.clone();
    let mut snapshot = tauri::async_runtime::spawn_blocking(move || {
        ClientSnapshot::build(&store, &scan_config, runtime)
    })
    .await
    .map_err(|error| format!("client snapshot command failed: {error}"))??;
    snapshot.diagnostics = DiagnosticsSummary {
        unread_count: diagnostics
            .iter()
            .filter(|record| record.severity != DiagnosticSeverity::Info)
            .count(),
        last_error: diagnostics
            .iter()
            .find(|record| record.severity == DiagnosticSeverity::Error)
            .map(|record| record.message.clone())
            .or_else(|| snapshot.runtime.last_error.clone()),
    };
    Ok(snapshot)
}

async fn emit_snapshot(app: &AppHandle, state: &AppState) -> Result<ClientSnapshot, String> {
    let snapshot = build_client_snapshot(state).await?;
    app.emit("client-snapshot-updated", &snapshot)
        .map_err(|error| format!("failed to emit client snapshot: {error}"))?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_client_snapshot(state: State<'_, AppState>) -> Result<ClientSnapshot, String> {
    build_client_snapshot(&state).await
}

#[tauri::command]
pub async fn get_client_settings(
    state: State<'_, AppState>,
) -> Result<ClientSettings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn set_client_settings(
    state: State<'_, AppState>,
    settings: ClientSettings,
) -> Result<ClientSettings, String> {
    state.update_settings(settings).await
}

#[tauri::command]
pub async fn start_runtime(state: State<'_, AppState>) -> Result<ClientSnapshot, String> {
    let mut settings = state.settings.read().await.clone();
    settings.startup_consent_granted = true;
    state.update_settings(settings).await?;
    state
        .runtime
        .ensure_running(None, None)
        .await
        .map_err(|error| error.to_string())?;
    build_client_snapshot(&state).await
}

#[tauri::command]
pub async fn stop_runtime(state: State<'_, AppState>) -> Result<ClientSnapshot, String> {
    state
        .runtime
        .stop()
        .await
        .map_err(|error| error.to_string())?;
    build_client_snapshot(&state).await
}

#[tauri::command]
pub async fn set_workspace_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SetWorkspaceProviderRequest,
) -> Result<ClientSnapshot, String> {
    let provider_id = request.provider_id.trim().to_string();
    if provider_id.is_empty() {
        return Err("providerId must not be empty".to_string());
    }
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .upsert_route_rule(&request.workspace, &provider_id, true)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("workspace provider command failed: {error}"))??;
    let snapshot = emit_snapshot(&app, &state).await?;
    let payload = serde_json::json!({
        "sequence": snapshot.sequence,
        "generatedAt": snapshot.generated_at,
        "workspace": &snapshot.workspace,
    });
    app.emit("workspace-changed", payload)
        .map_err(|error| format!("failed to emit workspace change: {error}"))?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_diagnostics(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<crate::diagnostics::DiagnosticRecord>, String> {
    Ok(state.diagnostics.recent(limit.unwrap_or(20)).await)
}

#[tauri::command]
pub async fn clear_diagnostics(state: State<'_, AppState>) -> Result<(), String> {
    state.diagnostics.clear().await;
    Ok(())
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
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderSummary, String> {
    let provider_id = provider_id.trim().to_string();
    if provider_id.is_empty() {
        return Err("providerId must not be empty".to_string());
    }
    let store = Arc::clone(&state.store);
    let result = tauri::async_runtime::spawn_blocking(move || {
        store
            .set_current(&provider_id)
            .map(|provider| ProviderSummary::from(&provider))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider command failed: {error}"))??;
    emit_snapshot(&app, &state).await?;
    Ok(result)
}

#[tauri::command]
pub async fn scan_cc_switch_providers(
    state: State<'_, AppState>,
) -> Result<CcSwitchScanReport, String> {
    let store = Arc::clone(&state.store);
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<CcSwitchScanReport, String> {
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
    .map_err(|error| format!("provider scan command failed: {error}"))?;
    match result {
        Ok(report) => Ok(report),
        Err(error) => {
            let message = error.clone();
            logging::record(
                &state,
                DiagnosticSeverity::Warning,
                "provider.cc_switch_scan_failed",
                message,
                "cc-switch",
                BTreeMap::new(),
                &[],
            )
            .await;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn import_cc_switch_providers(
    app: AppHandle,
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
    let result = tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|error| format!("provider import command failed: {error}"))?;
    match result {
        Ok(report) => {
            emit_snapshot(&app, &state).await?;
            Ok(report)
        }
        Err(error) => {
            let message = error.clone();
            logging::record(
                &state,
                DiagnosticSeverity::Warning,
                "provider.cc_switch_import_failed",
                message,
                "cc-switch",
                BTreeMap::new(),
                &[],
            )
            .await;
            Err(error)
        }
    }
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
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpsertRouteRuleRequest,
) -> Result<WorkspaceRouteRule, String> {
    let store = Arc::clone(&state.store);
    let rule = tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|error| format!("route rule command failed: {error}"))??;
    emit_snapshot(&app, &state).await?;
    Ok(rule)
}

#[tauri::command]
pub async fn remove_route_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace: PathBuf,
) -> Result<WorkspaceRouteRule, String> {
    let store = Arc::clone(&state.store);
    let rule = tauri::async_runtime::spawn_blocking(move || {
        store
            .remove_route_rule(&workspace)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("route rule command failed: {error}"))??;
    emit_snapshot(&app, &state).await?;
    Ok(rule)
}

#[tauri::command]
pub async fn get_lifecycle_status(state: State<'_, AppState>) -> Result<LifecycleStatus, String> {
    let runtime = state.runtime.snapshot().await;
    Ok(LifecycleStatus {
        status: if runtime.active { "active" } else { "inactive" },
        active: runtime.active,
        pid: runtime.pid,
        port: runtime.port,
        server_reachable: runtime.server_reachable,
        config_managed: runtime.config_managed,
        external_modification: runtime.external_modification,
        config_path: state.scan_config.codex_home.join("config.toml"),
        state_path: state.data_dir.join("route-state.json"),
        lock_path: state.data_dir.join("route.lock"),
    })
}

#[tauri::command]
pub async fn activate_route(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ActivateRouteRequest,
) -> Result<ActivationResult, String> {
    let snapshot = state
        .runtime
        .ensure_running(request.provider_id, request.port)
        .await
        .map_err(|error| error.to_string())?;
    let pid = snapshot
        .pid
        .ok_or_else(|| "route started without a process id".to_string())?;
    let port = snapshot
        .port
        .ok_or_else(|| "route started without a listener port".to_string())?;
    let result = ActivationResult {
        status: "active",
        pid,
        port,
        route_url: format!("http://127.0.0.1:{port}/v1"),
        config_path: state.scan_config.codex_home.join("config.toml"),
        state_path: state.data_dir.join("route-state.json"),
        lock_path: state.data_dir.join("route.lock"),
    };
    let _ = emit_snapshot(&app, &state).await?;
    Ok(result)
}

#[tauri::command]
pub async fn deactivate_route(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeactivationResult, String> {
    let before = state.runtime.snapshot().await;
    let snapshot = state
        .runtime
        .stop()
        .await
        .map_err(|error| error.to_string())?;
    let result = DeactivationResult {
        status: "inactive",
        pid: before.pid,
        config_restored: !snapshot.config_managed,
        config_path: state.scan_config.codex_home.join("config.toml"),
    };
    let _ = emit_snapshot(&app, &state).await?;
    Ok(result)
}
