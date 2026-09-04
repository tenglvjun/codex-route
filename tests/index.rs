use std::fs;
use std::path::Path;

use codex_route::config::ScanConfig;
use codex_route::index::{ResolveError, SessionWorkspaceIndex};
use tempfile::TempDir;

fn write_rollout(
    codex_home: &Path,
    directory: &str,
    name: &str,
    session_id: &str,
    thread_id: &str,
    cwd: &Path,
    timestamp: &str,
) {
    let directory = codex_home.join(directory).join("2026/09/02");
    fs::create_dir_all(&directory).expect("fixture directory should be created");
    let path = directory.join(name);
    let line = serde_json::json!({
        "timestamp": timestamp,
        "type": "session_meta",
        "payload": {
            "session_id": session_id,
            "id": thread_id,
            "timestamp": timestamp,
            "cwd": cwd.to_string_lossy(),
            "originator": "codex",
            "cli_version": "test"
        }
    });
    fs::write(path, format!("{line}\n")).expect("fixture should be written");
}

#[test]
fn groups_active_and_archived_threads_by_session() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    let other = home.path().join("other");
    write_rollout(
        home.path(),
        "sessions",
        "rollout-a.jsonl",
        "S",
        "T1",
        &repo,
        "2026-01-01T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "archived_sessions",
        "rollout-b.jsonl",
        "S",
        "T2",
        &repo,
        "2026-01-02T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "sessions",
        "rollout-other.jsonl",
        "OTHER",
        "T3",
        &other,
        "2026-01-01T00:00:00Z",
    );

    let config = ScanConfig {
        codex_home: home.path().to_path_buf(),
        max_rollout_bytes: 64 * 1024,
    };
    let index = SessionWorkspaceIndex::build(&config).expect("index should build");
    assert_eq!(
        index.session_ids(),
        vec!["OTHER".to_string(), "S".to_string()]
    );
    let result = index.resolve("S").expect("session should resolve");

    assert_eq!(result.workspace, repo);
    assert_eq!(result.thread_ids, vec!["T1".to_string(), "T2".to_string()]);
    assert_eq!(result.workspaces, vec![repo]);
    assert!(!result.conflicting_workspaces);
}

#[test]
fn reports_conflicting_workspaces_without_guessing_parent_lineage() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    let worktree = home.path().join("repo-worktree");
    write_rollout(
        home.path(),
        "sessions",
        "rollout-new.jsonl",
        "S",
        "T2",
        &worktree,
        "2026-01-02T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "sessions",
        "rollout-old.jsonl",
        "S",
        "T1",
        &repo,
        "2026-01-01T00:00:00Z",
    );

    let config = ScanConfig {
        codex_home: home.path().to_path_buf(),
        max_rollout_bytes: 64 * 1024,
    };
    let result = SessionWorkspaceIndex::build(&config)
        .expect("index should build")
        .resolve("S")
        .expect("session should resolve");

    assert_eq!(result.workspace, repo);
    assert_eq!(result.workspaces, vec![repo, worktree]);
    assert!(result.conflicting_workspaces);
}

#[test]
fn unknown_session_is_distinct_from_empty_session_id() {
    let home = TempDir::new().expect("temporary home should be created");
    let config = ScanConfig {
        codex_home: home.path().to_path_buf(),
        max_rollout_bytes: 64 * 1024,
    };
    let index = SessionWorkspaceIndex::build(&config).expect("empty home should be indexable");

    assert_eq!(
        index.resolve("missing"),
        Err(ResolveError::SessionNotFound("missing".into()))
    );
    assert_eq!(index.resolve("  "), Err(ResolveError::EmptySessionId));
}

#[test]
fn active_index_excludes_archived_session_ids() {
    let home = TempDir::new().expect("temporary home should be created");
    let workspace = home.path().join("project");
    write_rollout(
        home.path(),
        "archived_sessions",
        "rollout-archived.jsonl",
        "ARCHIVED",
        "TA",
        &workspace,
        "2026-01-03T00:00:00Z",
    );
    let config = ScanConfig {
        codex_home: home.path().to_path_buf(),
        max_rollout_bytes: 64 * 1024,
    };

    let index = SessionWorkspaceIndex::build_active(&config).expect("index should build");
    assert!(index.session_ids().is_empty());
    assert!(index.workspaces().is_empty());
}
