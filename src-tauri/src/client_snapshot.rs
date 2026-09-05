use crate::runtime::RuntimeSnapshot;
use codex_route::codex_projects::load_saved_project_roots;
use codex_route::config::ScanConfig;
use codex_route::index::{SessionWorkspaceIndex, WorkspaceAggregate};
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
    pub session_ids: Vec<String>,
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
    #[serde(default)]
    pub workspaces: Vec<WorkspaceSnapshot>,
    /// The most recent workspace is retained for older renderer clients.
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
        let workspaces = workspace_snapshots(scan_config, &rules, &provider_records);
        let workspace = workspaces.first().cloned();
        let default_provider = current_provider.map(ProviderSummary::from);
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
            workspaces,
            workspace,
            provider: default_provider,
            providers,
            rules,
            runtime,
            diagnostics,
        })
    }
}

fn workspace_snapshots(
    scan_config: &ScanConfig,
    rules: &[WorkspaceRouteRule],
    providers: &[Provider],
) -> Vec<WorkspaceSnapshot> {
    let project_roots = load_saved_project_roots(&scan_config.codex_home);
    let Ok(index) =
        SessionWorkspaceIndex::build_active_with_project_roots(scan_config, &project_roots)
    else {
        return Vec::new();
    };
    index
        .workspaces()
        .into_iter()
        .map(|aggregate| workspace_snapshot(aggregate, rules, providers))
        .collect()
}

fn workspace_snapshot(
    aggregate: WorkspaceAggregate,
    rules: &[WorkspaceRouteRule],
    providers: &[Provider],
) -> WorkspaceSnapshot {
    let provider_id = rules
        .iter()
        .find(|rule| rule.workspace == aggregate.workspace)
        .and_then(|rule| {
            providers
                .iter()
                .any(|provider| provider.id == rule.provider_id)
                .then(|| rule.provider_id.clone())
        });
    let session_id = aggregate.session_ids.first().cloned().unwrap_or_default();
    WorkspaceSnapshot {
        path: aggregate.workspace,
        exists: aggregate.workspace_exists,
        session_id,
        session_ids: aggregate.session_ids,
        thread_ids: aggregate.thread_ids,
        provider_id,
        last_activity: aggregate.last_activity,
        conflicting_workspaces: aggregate.conflicting_sessions,
    }
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
            workspaces: Vec::new(),
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
