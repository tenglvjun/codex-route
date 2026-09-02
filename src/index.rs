use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use thiserror::Error;

use crate::config::ScanConfig;
use crate::rollout::{
    discover_rollouts, is_archived_path, read_session_meta, RolloutError, RolloutSessionMeta,
};
use crate::workspace_rule::normalize_workspace_path;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct WorkspaceLookup {
    pub session_id: String,
    pub workspace: PathBuf,
    pub workspace_exists: bool,
    pub workspaces: Vec<PathBuf>,
    pub thread_ids: Vec<String>,
    pub rollout_paths: Vec<PathBuf>,
    pub conflicting_workspaces: bool,
}

#[derive(Debug, Error)]
pub enum IndexError {
    #[error(transparent)]
    Rollout(#[from] RolloutError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("session_id must not be empty")]
    EmptySessionId,
    #[error("no rollout was found for session_id {0}")]
    SessionNotFound(String),
}

#[derive(Debug, Clone)]
pub struct SessionWorkspaceIndex {
    sessions: BTreeMap<String, Vec<RolloutSessionMeta>>,
}

impl SessionWorkspaceIndex {
    pub fn build(config: &ScanConfig) -> Result<Self, IndexError> {
        let paths = discover_rollouts(&config.codex_home)?;
        let mut sessions = BTreeMap::<String, Vec<RolloutSessionMeta>>::new();

        for path in paths {
            let archived = is_archived_path(&config.codex_home, &path);
            match read_session_meta(&path, archived, config.max_rollout_bytes) {
                Ok(Some(meta)) => sessions
                    .entry(meta.session_id.clone())
                    .or_default()
                    .push(meta),
                Ok(None) | Err(RolloutError::ScanLimitExceeded { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }

        Ok(Self { sessions })
    }

    /// Returns every unique session ID in stable lexical order.
    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    pub fn resolve(&self, session_id: &str) -> Result<WorkspaceLookup, ResolveError> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Err(ResolveError::EmptySessionId);
        }

        let records = self
            .sessions
            .get(session_id)
            .ok_or_else(|| ResolveError::SessionNotFound(session_id.to_string()))?;
        let mut ranked_records = records.clone();
        ranked_records.sort_by(compare_records);

        let normalized_workspaces = ranked_records
            .iter()
            .map(|record| normalize_workspace(&record.workspace))
            .collect::<BTreeSet<_>>();
        let conflicting_workspaces = normalized_workspaces.len() > 1;
        let primary_workspace = normalize_workspace(&ranked_records[0].workspace);
        let mut thread_ids = ranked_records
            .iter()
            .map(|record| record.thread_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let rollout_paths = ranked_records
            .iter()
            .map(|record| record.rollout_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        thread_ids.sort();

        Ok(WorkspaceLookup {
            session_id: session_id.to_string(),
            workspace_exists: primary_workspace.exists(),
            workspace: primary_workspace,
            workspaces: normalized_workspaces.into_iter().collect(),
            thread_ids,
            rollout_paths,
            conflicting_workspaces,
        })
    }
}

fn compare_records(left: &RolloutSessionMeta, right: &RolloutSessionMeta) -> Ordering {
    compare_optional_strings(&left.timestamp, &right.timestamp)
        .then_with(|| compare_system_times(left.modified_at, right.modified_at))
        .then_with(|| left.rollout_path.cmp(&right.rollout_path))
}

fn compare_optional_strings(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left.as_deref(), right.as_deref()) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_system_times(left: Option<SystemTime>, right: Option<SystemTime>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn normalize_workspace(path: &Path) -> PathBuf {
    normalize_workspace_path(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("codex-route-index-tests")
            .join(name)
    }

    fn record(
        session_id: &str,
        thread_id: &str,
        workspace: &Path,
        timestamp: &str,
    ) -> RolloutSessionMeta {
        RolloutSessionMeta {
            session_id: session_id.to_string(),
            thread_id: thread_id.to_string(),
            workspace: workspace.to_path_buf(),
            timestamp: Some(timestamp.to_string()),
            rollout_path: fixture_path(&format!("{thread_id}.jsonl")),
            archived: false,
            modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
        }
    }

    #[test]
    fn resolves_same_session_across_threads() {
        let repo = fixture_path("repo");
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([(
                "S".to_string(),
                vec![
                    record("S", "T1", &repo, "2026-01-01T00:00:00Z"),
                    record("S", "T2", &repo, "2026-01-02T00:00:00Z"),
                ],
            )]),
        };

        let result = index.resolve("S").expect("session should resolve");
        assert_eq!(result.workspace, repo);
        assert_eq!(result.thread_ids, vec!["T1", "T2"]);
        assert_eq!(result.workspaces, vec![repo]);
        assert!(!result.conflicting_workspaces);
    }

    #[test]
    fn reports_conflicting_workspaces_and_selects_oldest_metadata() {
        let new_workspace = fixture_path("new");
        let old_workspace = fixture_path("old");
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([(
                "S".to_string(),
                vec![
                    record("S", "T2", &new_workspace, "2026-01-02T00:00:00Z"),
                    record("S", "T1", &old_workspace, "2026-01-01T00:00:00Z"),
                ],
            )]),
        };

        let result = index.resolve("S").expect("session should resolve");
        assert_eq!(result.workspace, old_workspace);
        assert_eq!(result.workspaces, vec![new_workspace, old_workspace]);
        assert!(result.conflicting_workspaces);
    }

    #[test]
    fn rejects_empty_and_unknown_sessions() {
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::new(),
        };
        assert_eq!(index.resolve("  "), Err(ResolveError::EmptySessionId));
        assert_eq!(
            index.resolve("missing"),
            Err(ResolveError::SessionNotFound("missing".to_string()))
        );
    }

    #[test]
    fn lists_unique_session_ids_in_order() {
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([
                ("session-b".to_string(), Vec::new()),
                ("session-a".to_string(), Vec::new()),
                ("session-b".to_string(), Vec::new()),
            ]),
        };

        assert_eq!(
            index.session_ids(),
            vec!["session-a".to_string(), "session-b".to_string()]
        );
    }
}
