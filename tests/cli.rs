use std::fs;
use std::path::Path;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

fn write_rollout(home: &Path, session_id: &str, thread_id: &str, cwd: &Path) {
    let directory = home.join("sessions/2026/09/02");
    fs::create_dir_all(&directory).expect("fixture directory should be created");
    let path = directory.join(format!("rollout-cli-{session_id}-{thread_id}.jsonl"));
    let line = serde_json::json!({
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
    });
    fs::write(path, format!("{line}\n")).expect("fixture should be written");
}

#[test]
fn help_lists_commands() {
    let mut command = Command::cargo_bin("codex-route").expect("binary should be available");
    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("resolve"))
        .stdout(predicate::str::contains("list"));

    Command::cargo_bin("codex-route")
        .expect("binary should be available")
        .args(["resolve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--session-id"));
}

#[test]
fn list_emits_unique_sorted_session_ids() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    write_rollout(home.path(), "session-b", "thread-1", &repo);
    write_rollout(home.path(), "session-a", "thread-1", &repo);
    write_rollout(home.path(), "session-a", "thread-2", &repo);

    let output = Command::cargo_bin("codex-route")
        .expect("binary should be available")
        .args([
            "list",
            "--codex-home",
            home.path()
                .to_str()
                .expect("temporary path should be UTF-8"),
        ])
        .output()
        .expect("command should run");

    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("successful output should be JSON");
    assert_eq!(value, serde_json::json!(["session-a", "session-b"]));
}

#[test]
fn resolve_emits_json_and_not_found_has_exit_code_three() {
    let home = TempDir::new().expect("temporary home should be created");
    let repo = home.path().join("repo");
    write_rollout(home.path(), "S", "T1", &repo);

    let output = Command::cargo_bin("codex-route")
        .expect("binary should be available")
        .args([
            "resolve",
            "--codex-home",
            home.path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--session-id",
            "S",
        ])
        .output()
        .expect("command should run");
    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("successful output should be JSON");
    assert_eq!(value["session_id"], "S");
    assert_eq!(value["workspace"], repo.to_string_lossy().as_ref());

    let missing = Command::cargo_bin("codex-route")
        .expect("binary should be available")
        .args([
            "resolve",
            "--codex-home",
            home.path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--session-id",
            "missing",
        ])
        .output()
        .expect("command should run");
    assert_eq!(missing.status.code(), Some(3));
    assert!(missing.stdout.is_empty());
}
