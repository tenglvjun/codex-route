use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use codex_route::rollout::{discover_rollouts, read_session_meta, RolloutError};
use tempfile::TempDir;

fn rollout_line(session_id: &str, thread_id: &str, cwd: &Path) -> String {
    serde_json::json!({
        "timestamp": "2026-09-02T12:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "session_id": session_id,
            "id": thread_id,
            "timestamp": "2026-09-02T12:00:00Z",
            "cwd": cwd.to_string_lossy(),
            "originator": "codex",
            "cli_version": "test"
        }
    })
    .to_string()
}

fn write_plain_rollout(
    codex_home: &Path,
    directory: &str,
    file_name: &str,
    contents: &str,
) -> PathBuf {
    let directory = codex_home.join(directory).join("2026/09/02");
    fs::create_dir_all(&directory).expect("fixture directory should be created");
    let path = directory.join(file_name);
    fs::write(&path, contents).expect("fixture should be written");
    path
}

#[test]
fn discovers_active_archived_and_ignores_unrelated_files() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    write_plain_rollout(
        home.path(),
        "sessions",
        "rollout-active.jsonl",
        &format!("{}\n", rollout_line("S", "T1", &repo)),
    );
    write_plain_rollout(
        home.path(),
        "archived_sessions",
        "rollout-archived.jsonl",
        &format!("{}\n", rollout_line("S", "T2", &repo)),
    );
    write_plain_rollout(home.path(), "sessions", "notes.jsonl", "not a rollout\n");

    let mut paths = discover_rollouts(home.path()).expect("rollouts should be discovered");
    paths.sort();

    assert_eq!(paths.len(), 2);
    assert!(paths
        .iter()
        .any(|path| path.ends_with("rollout-active.jsonl")));
    assert!(paths
        .iter()
        .any(|path| path.ends_with("rollout-archived.jsonl")));
}

#[test]
fn reads_plain_and_compressed_metadata() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    let plain = write_plain_rollout(
        home.path(),
        "sessions",
        "rollout-plain.jsonl",
        &format!(
            "{}\nuser content after metadata\n",
            rollout_line("S", "T1", &repo)
        ),
    );
    let compressed =
        write_plain_rollout(home.path(), "sessions", "rollout-compressed.jsonl.zst", "");
    let compressed_bytes = zstd::stream::encode_all(
        format!("{}\n", rollout_line("S", "T2", &repo)).as_bytes(),
        3,
    )
    .expect("fixture should compress");
    fs::write(&compressed, compressed_bytes).expect("compressed fixture should be written");

    let plain_meta = read_session_meta(&plain, false, 64 * 1024)
        .expect("plain rollout should be read")
        .expect("plain metadata should be present");
    let compressed_meta = read_session_meta(&compressed, false, 64 * 1024)
        .expect("compressed rollout should be read")
        .expect("compressed metadata should be present");

    assert_eq!(plain_meta.session_id, "S");
    assert_eq!(plain_meta.thread_id, "T1");
    assert_eq!(compressed_meta.thread_id, "T2");
    assert_eq!(compressed_meta.workspace, repo);
}

#[test]
fn scan_limit_is_applied_to_logical_rollout_bytes() {
    let home = TempDir::new().expect("temporary home should be created");
    let path = write_plain_rollout(home.path(), "sessions", "rollout-large.jsonl", "");
    let mut file = fs::File::create(&path).expect("fixture should be opened");
    file.write_all(&[b'x'; 65])
        .expect("fixture should be written");

    assert!(matches!(
        read_session_meta(&path, false, 64),
        Err(RolloutError::ScanLimitExceeded { limit: 64, .. })
    ));
}
