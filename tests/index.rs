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
    cwd: &str,
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
            "cwd": cwd,
            "originator": "codex",
            "cli_version": "test"
        }
    });
    fs::write(path, format!("{line}\n")).expect("fixture should be written");
}

#[test]
fn groups_active_and_archived_threads_by_session() {
    let home = TempDir::new().expect("temporary home should be created");
    write_rollout(
        home.path(),
        "sessions",
        "rollout-a.jsonl",
        "S",
        "T1",
        "/repo",
        "2026-01-01T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "archived_sessions",
        "rollout-b.jsonl",
        "S",
        "T2",
        "/repo",
        "2026-01-02T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "sessions",
        "rollout-other.jsonl",
        "OTHER",
        "T3",
        "/other",
        "2026-01-01T00:00:00Z",
    );

    let config = ScanConfig {
        codex_home: home.path().to_path_buf(),
        max_rollout_bytes: 64 * 1024,
    };
    let index = SessionWorkspaceIndex::build(&config).expect("index should build");
    let result = index.resolve("S").expect("session should resolve");

    assert_eq!(result.workspace, Path::new("/repo"));
    assert_eq!(result.thread_ids, vec!["T1".to_string(), "T2".to_string()]);
    assert_eq!(result.workspaces, vec![Path::new("/repo").to_path_buf()]);
    assert!(!result.conflicting_workspaces);
}

#[test]
fn reports_conflicting_workspaces_without_guessing_parent_lineage() {
    let home = TempDir::new().expect("temporary home should be created");
    write_rollout(
        home.path(),
        "sessions",
        "rollout-new.jsonl",
        "S",
        "T2",
        "/repo-worktree",
        "2026-01-02T00:00:00Z",
    );
    write_rollout(
        home.path(),
        "sessions",
        "rollout-old.jsonl",
        "S",
        "T1",
        "/repo",
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

    assert_eq!(result.workspace, Path::new("/repo"));
    assert_eq!(
        result.workspaces,
        vec![
            Path::new("/repo").to_path_buf(),
            Path::new("/repo-worktree").to_path_buf()
        ]
    );
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
