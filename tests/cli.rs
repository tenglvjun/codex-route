use std::fs;
use std::path::Path;

use assert_cmd::prelude::*;
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::ProviderStore;
use predicates::prelude::*;
use rusqlite::{params, Connection};
use serde_json::json;
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
fn route_help_exposes_responses_server_options() {
    Command::cargo_bin("codex-route")
        .unwrap()
        .args(["route", "serve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("16729"))
        .stdout(predicate::str::contains("--provider"));
}

#[test]
fn route_rule_commands_manage_workspace_rules() {
    let data_dir = tempfile::tempdir().unwrap();
    let store = ProviderStore::open(data_dir.path().join("codex-route.db")).unwrap();
    store
        .insert(&Provider {
            id: "provider-a".to_string(),
            name: "Provider A".to_string(),
            settings_config: json!({
                "auth": {"OPENAI_API_KEY": "sk-test"},
                "config": "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"https://example.test/v1\""
            }),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            icon: None,
            icon_color: None,
            meta: json!({}),
            in_failover_queue: false,
            is_current: true,
            source: ProviderSource::Local,
        })
        .unwrap();
    let workspace = data_dir.path().join("project");
    std::fs::create_dir_all(&workspace).unwrap();
    let data_text = data_dir.path().to_str().unwrap();
    let workspace_text = workspace.to_str().unwrap();

    Command::cargo_bin("codex-route")
        .unwrap()
        .args(["route", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rule"));

    let add = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "route",
            "rule",
            "add",
            "--data-dir",
            data_text,
            "--workspace",
            workspace_text,
            "--provider",
            "provider-a",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());
    let added: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(added["action"], "inserted");
    assert_eq!(added["rule"]["providerId"], "provider-a");

    Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "route",
            "rule",
            "add",
            "--data-dir",
            data_text,
            "--workspace",
            workspace_text,
            "--provider",
            "provider-a",
        ])
        .assert()
        .failure();

    let replace = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "route",
            "rule",
            "add",
            "--data-dir",
            data_text,
            "--workspace",
            workspace_text,
            "--provider",
            "provider-a",
            "--replace",
        ])
        .output()
        .unwrap();
    assert!(replace.status.success());
    let replaced: serde_json::Value = serde_json::from_slice(&replace.stdout).unwrap();
    assert_eq!(replaced["action"], "replaced");

    let list = Command::cargo_bin("codex-route")
        .unwrap()
        .args(["route", "rule", "list", "--data-dir", data_text])
        .output()
        .unwrap();
    assert!(list.status.success());
    let rules: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(rules.as_array().unwrap().len(), 1);
    assert_eq!(rules[0]["providerId"], "provider-a");

    let remove = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "route",
            "rule",
            "remove",
            "--data-dir",
            data_text,
            "--workspace",
            workspace_text,
        ])
        .output()
        .unwrap();
    assert!(remove.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&remove.stdout).unwrap()["providerId"],
        "provider-a"
    );
}

#[test]
fn route_serve_rejects_missing_current_provider() {
    let data_dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "route",
            "serve",
            "--data-dir",
            data_dir.path().to_str().unwrap(),
            "--port",
            "0",
        ])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("no current provider"));
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

#[test]
fn provider_import_list_and_show_redact_secrets() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let source = directory.path().join("cc-switch.db");
    let data = directory.path().join("data");
    let connection = Connection::open(&source).expect("source database should open");
    connection
        .execute_batch(
            "CREATE TABLE providers (
                id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                name TEXT NOT NULL,
                settings_config TEXT NOT NULL,
                meta TEXT NOT NULL DEFAULT '{}',
                PRIMARY KEY (id, app_type)
            );",
        )
        .unwrap();
    let settings = serde_json::json!({
        "auth": {"OPENAI_API_KEY": "sk-cli-secret"},
        "config": "model = \"gpt-5-codex\"\nexperimental_bearer_token = \"toml-cli-secret\""
    });
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "codex-provider",
                "codex",
                "Codex Provider",
                serde_json::to_string(&settings).unwrap(),
                "{}"
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES ('claude-provider', 'claude', 'Claude', '{}', '{}')",
            [],
        )
        .unwrap();

    let source_text = source.to_str().unwrap();
    let data_text = data.to_str().unwrap();
    let import = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "provider",
            "import-cc-switch",
            "--data-dir",
            data_text,
            "--cc-switch-db",
            source_text,
        ])
        .output()
        .unwrap();
    assert!(import.status.success());
    let report: serde_json::Value = serde_json::from_slice(&import.stdout).unwrap();
    assert_eq!(report["imported"], 1);

    let list = Command::cargo_bin("codex-route")
        .unwrap()
        .args(["provider", "list", "--data-dir", data_text])
        .output()
        .unwrap();
    let providers: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(providers[0]["id"], "codex-provider");
    assert!(providers[0].get("settingsConfig").is_none());

    let show = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "provider",
            "show",
            "codex-provider",
            "--data-dir",
            data_text,
        ])
        .output()
        .unwrap();
    let redacted: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(
        redacted["settingsConfig"]["auth"]["OPENAI_API_KEY"],
        "[REDACTED]"
    );
    assert!(!String::from_utf8_lossy(&show.stdout).contains("sk-cli-secret"));
    assert!(!String::from_utf8_lossy(&show.stdout).contains("toml-cli-secret"));

    let reveal = Command::cargo_bin("codex-route")
        .unwrap()
        .args([
            "provider",
            "show",
            "codex-provider",
            "--data-dir",
            data_text,
            "--reveal-secrets",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&reveal.stdout).contains("sk-cli-secret"));
    assert!(String::from_utf8_lossy(&reveal.stdout).contains("toml-cli-secret"));
}
