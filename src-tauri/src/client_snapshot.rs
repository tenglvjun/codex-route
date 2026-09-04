use crate::runtime::RuntimeSnapshot;
use codex_route::config::ScanConfig;
use codex_route::index::SessionWorkspaceIndex;
use codex_route::provider::{Provider, ProviderSummary};
use codex_route::provider_store::ProviderStore;
use codex_route::workspace_rule::WorkspaceRouteRule;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexStatus {
    pub home: PathBuf,
    pub config_path: PathBuf,
    pub installed: bool,
    pub version: Option<String>,
    pub config_exists: bool,
    pub config_managed: bool,
    pub external_modification: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub path: PathBuf,
    pub exists: bool,
    pub session_id: String,
    pub thread_ids: Vec<String>,
    pub provider_id: Option<String>,
    pub last_activity: Option<i64>,
    pub conflicting_workspaces: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSummary {
    pub unread_count: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSnapshot {
    pub schema_version: u16,
    pub sequence: u64,
    pub generated_at: i64,
    pub codex: CodexStatus,
    pub workspace: Option<WorkspaceSnapshot>,
    pub provider: Option<ProviderSummary>,
    pub providers: Vec<ProviderSummary>,
    pub rules: Vec<WorkspaceRouteRule>,
    pub runtime: RuntimeSnapshot,
    pub diagnostics: DiagnosticsSummary,
}

impl ClientSnapshot {
    pub fn build(
        store: &ProviderStore,
        scan_config: &ScanConfig,
        runtime: RuntimeSnapshot,
    ) -> Result<Self, String> {
        let provider_records = store.list().map_err(|error| error.to_string())?;
        let providers = provider_records
            .iter()
            .map(ProviderSummary::from)
            .collect::<Vec<_>>();
        let rules = store
            .list_route_rules()
            .map_err(|error| error.to_string())?;
        let current_provider = provider_records.iter().find(|provider| provider.is_current);
        let workspace = latest_workspace(scan_config, &rules, &provider_records);
        let effective_provider = workspace
            .as_ref()
            .and_then(|workspace| workspace.provider_id.as_deref())
            .and_then(|provider_id| {
                provider_records
                    .iter()
                    .find(|provider| provider.id == provider_id)
            })
            .or(current_provider)
            .map(ProviderSummary::from);
        let codex = detect_codex(scan_config, &runtime);
        let diagnostics = DiagnosticsSummary {
            unread_count: usize::from(runtime.last_error.is_some()),
            last_error: runtime.last_error.clone(),
        };

        Ok(Self {
            schema_version: 1,
            sequence: runtime.sequence,
            generated_at: now_unix_seconds(),
            codex,
            workspace,
            provider: effective_provider,
            providers,
            rules,
            runtime,
            diagnostics,
        })
    }
}

fn latest_workspace(
    scan_config: &ScanConfig,
    rules: &[WorkspaceRouteRule],
    providers: &[Provider],
) -> Option<WorkspaceSnapshot> {
    let index = SessionWorkspaceIndex::build(scan_config).ok()?;
    let lookup = index.latest_workspace()?;
    let provider_id = rules
        .iter()
        .find(|rule| rule.workspace == lookup.workspace)
        .and_then(|rule| {
            providers
                .iter()
                .any(|provider| provider.id == rule.provider_id)
                .then(|| rule.provider_id.clone())
        });
    let last_activity = lookup
        .rollout_paths
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
        .filter_map(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .max();
    Some(WorkspaceSnapshot {
        path: lookup.workspace.clone(),
        exists: lookup.workspace_exists,
        session_id: lookup.session_id,
        thread_ids: lookup.thread_ids,
        provider_id,
        last_activity,
        conflicting_workspaces: lookup.conflicting_workspaces,
    })
}

fn detect_codex(scan_config: &ScanConfig, runtime: &RuntimeSnapshot) -> CodexStatus {
    let config_path = scan_config.codex_home.join("config.toml");
    static CODEX_INSTALLATION: OnceLock<(bool, Option<String>)> = OnceLock::new();
    let (installed, version) = CODEX_INSTALLATION
        .get_or_init(|| {
            Command::new("codex")
                .arg("--version")
                .output()
                .ok()
                .map(|output| {
                    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    (output.status.success(), (!text.is_empty()).then_some(text))
                })
                .unwrap_or((false, None))
        })
        .clone();
    CodexStatus {
        home: scan_config.codex_home.clone(),
        config_path,
        installed,
        version,
        config_exists: scan_config.codex_home.join("config.toml").is_file(),
        config_managed: runtime.config_managed,
        external_modification: runtime.external_modification,
    }
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{ClientSnapshot, CodexStatus, DiagnosticsSummary};
    use crate::runtime::RuntimeSnapshot;
    use std::path::PathBuf;

    #[test]
    fn snapshot_serializes_as_the_renderer_contract() {
        let snapshot = ClientSnapshot {
            schema_version: 1,
            sequence: 0,
            generated_at: 42,
            codex: CodexStatus {
                home: PathBuf::from("/tmp/.codex"),
                config_path: PathBuf::from("/tmp/.codex/config.toml"),
                installed: true,
                version: Some("codex-cli 1.0".to_string()),
                config_exists: true,
                config_managed: false,
                external_modification: false,
            },
            workspace: None,
            provider: None,
            providers: Vec::new(),
            rules: Vec::new(),
            runtime: RuntimeSnapshot::default(),
            diagnostics: DiagnosticsSummary::default(),
        };

        let value = serde_json::to_value(snapshot).expect("serialize snapshot");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["sequence"], 0);
        assert_eq!(value["runtime"]["restartCount"], 0);
        assert_eq!(value["codex"]["configPath"], "/tmp/.codex/config.toml");
    }
}
