use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use thiserror::Error;

use crate::config::ScanConfig;
use crate::rollout::{
    discover_active_rollouts, discover_rollouts, is_archived_path, read_session_meta, RolloutError,
    RolloutSessionMeta,
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

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct WorkspaceAggregate {
    pub workspace: PathBuf,
    pub workspace_exists: bool,
    pub session_ids: Vec<String>,
    pub thread_ids: Vec<String>,
    pub rollout_paths: Vec<PathBuf>,
    pub last_activity: Option<i64>,
    pub conflicting_sessions: bool,
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

    /// Returns an index built only from active Codex sessions.
    pub fn build_active(config: &ScanConfig) -> Result<Self, IndexError> {
        let paths = discover_active_rollouts(&config.codex_home)?;
        let mut sessions = BTreeMap::<String, Vec<RolloutSessionMeta>>::new();
        for path in paths {
            match read_session_meta(&path, false, config.max_rollout_bytes) {
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

    /// Returns one row per unique workspace, ordered by the most recent
    /// active session activity. Archived records are ignored by construction
    /// when called on an index from `build_active`.
    pub fn workspaces(&self) -> Vec<WorkspaceAggregate> {
        let mut grouped = BTreeMap::<PathBuf, WorkspaceAggregateBuilder>::new();
        let mut session_workspaces = BTreeMap::<String, BTreeSet<PathBuf>>::new();

        for record in self
            .sessions
            .values()
            .flatten()
            .filter(|record| !record.archived)
        {
            let workspace = normalize_workspace(&record.workspace);
            session_workspaces
                .entry(record.session_id.clone())
                .or_default()
                .insert(workspace.clone());

            grouped
                .entry(workspace.clone())
                .or_default()
                .add(record, workspace);
        }

        let ambiguous_sessions = session_workspaces
            .into_iter()
            .filter_map(|(session_id, workspaces)| (workspaces.len() > 1).then_some(session_id))
            .collect::<BTreeSet<_>>();

        let mut workspaces = grouped
            .into_iter()
            .map(|(_, builder)| builder.finish(&ambiguous_sessions))
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| {
            compare_activity_desc(left.last_activity, right.last_activity)
                .then_with(|| left.workspace.cmp(&right.workspace))
        });
        workspaces
    }

    /// Returns the workspace for the most recently modified active session.
    /// Archived sessions are considered only when no active session exists.
    pub fn latest_workspace(&self) -> Option<WorkspaceLookup> {
        let latest_active = self
            .sessions
            .values()
            .flatten()
            .filter(|record| !record.archived)
            .max_by(|left, right| compare_recency(left, right));
        let latest = latest_active.or_else(|| {
            self.sessions
                .values()
                .flatten()
                .max_by(|left, right| compare_recency(left, right))
        })?;
        self.resolve(&latest.session_id).ok()
    }
}

#[derive(Default)]
struct WorkspaceAggregateBuilder {
    workspace: Option<PathBuf>,
    session_ids: BTreeSet<String>,
    thread_ids: BTreeSet<String>,
    rollout_paths: BTreeSet<PathBuf>,
    latest_record: Option<RolloutSessionMeta>,
}

impl WorkspaceAggregateBuilder {
    fn add(&mut self, record: &RolloutSessionMeta, workspace: PathBuf) {
        self.workspace = Some(workspace);
        self.session_ids.insert(record.session_id.clone());
        self.thread_ids.insert(record.thread_id.clone());
        self.rollout_paths.insert(record.rollout_path.clone());

        if self
            .latest_record
            .as_ref()
            .is_none_or(|current| compare_recency(record, current) == Ordering::Greater)
        {
            self.latest_record = Some(record.clone());
        }
    }

    fn finish(self, ambiguous_sessions: &BTreeSet<String>) -> WorkspaceAggregate {
        let workspace = self.workspace.unwrap_or_default();
        let last_activity = self
            .latest_record
            .as_ref()
            .and_then(|record| record.modified_at)
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64);
        let conflicting_sessions = self
            .session_ids
            .iter()
            .any(|session_id| ambiguous_sessions.contains(session_id));

        WorkspaceAggregate {
            workspace_exists: workspace.exists(),
            workspace,
            session_ids: self.session_ids.into_iter().collect(),
            thread_ids: self.thread_ids.into_iter().collect(),
            rollout_paths: self.rollout_paths.into_iter().collect(),
            last_activity,
            conflicting_sessions,
        }
    }
}

fn compare_activity_desc(left: Option<i64>, right: Option<i64>) -> Ordering {
    right.cmp(&left)
}

fn compare_recency(left: &RolloutSessionMeta, right: &RolloutSessionMeta) -> Ordering {
    match (left.modified_at, right.modified_at) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => left.timestamp.cmp(&right.timestamp),
    }
    .then_with(|| left.rollout_path.cmp(&right.rollout_path))
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
    fn latest_workspace_prefers_recent_active_session() {
        let old_workspace = fixture_path("old-active");
        let new_workspace = fixture_path("new-active");
        let archived_workspace = fixture_path("newer-archived");
        let mut archived = record(
            "ARCHIVED",
            "TA",
            &archived_workspace,
            "2026-01-03T00:00:00Z",
        );
        archived.archived = true;
        archived.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(30));
        let mut old = record("OLD", "T1", &old_workspace, "2026-01-01T00:00:00Z");
        old.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(10));
        let mut new = record("NEW", "T2", &new_workspace, "2026-01-02T00:00:00Z");
        new.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(20));
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([
                ("ARCHIVED".to_string(), vec![archived]),
                ("OLD".to_string(), vec![old]),
                ("NEW".to_string(), vec![new]),
            ]),
        };

        let latest = index.latest_workspace().expect("latest workspace");
        assert_eq!(latest.session_id, "NEW");
        assert_eq!(latest.workspace, new_workspace);
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

    #[test]
    fn aggregates_active_workspaces_by_recency_and_path() {
        let recent_workspace = fixture_path("recent");
        let older_workspace = fixture_path("older");
        let archived_workspace = fixture_path("archived-only");
        let mut archived = record(
            "ARCHIVED",
            "TA",
            &archived_workspace,
            "2026-01-03T00:00:00Z",
        );
        archived.archived = true;
        archived.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(300));
        let mut recent_a = record("RECENT", "T1", &recent_workspace, "2026-01-01T00:00:00Z");
        recent_a.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(200));
        let mut recent_b = record("RECENT", "T2", &recent_workspace, "2026-01-02T00:00:00Z");
        recent_b.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(250));
        let mut older = record("OLDER", "TO", &older_workspace, "2026-01-01T00:00:00Z");
        older.modified_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(100));

        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([
                ("ARCHIVED".to_string(), vec![archived]),
                ("RECENT".to_string(), vec![recent_a, recent_b]),
                ("OLDER".to_string(), vec![older]),
            ]),
        };

        let workspaces = index.workspaces();
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0].workspace, recent_workspace);
        assert_eq!(workspaces[0].session_ids, vec!["RECENT".to_string()]);
        assert_eq!(
            workspaces[0].thread_ids,
            vec!["T1".to_string(), "T2".to_string()]
        );
        assert_eq!(workspaces[0].last_activity, Some(250));
        assert_eq!(workspaces[1].workspace, older_workspace);
    }

    #[test]
    fn marks_workspace_when_a_session_has_conflicting_active_workspaces() {
        let left = fixture_path("left");
        let right = fixture_path("right");
        let index = SessionWorkspaceIndex {
            sessions: BTreeMap::from([(
                "S".to_string(),
                vec![
                    record("S", "T1", &left, "2026-01-01T00:00:00Z"),
                    record("S", "T2", &right, "2026-01-02T00:00:00Z"),
                ],
            )]),
        };

        assert!(index
            .workspaces()
            .iter()
            .all(|workspace| workspace.conflicting_sessions));
    }
}
