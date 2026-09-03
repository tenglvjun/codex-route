use assert_cmd::prelude::*;
use codex_route::config::ScanConfig;
use codex_route::lifecycle::{EmbeddedRouteService, LifecyclePaths};
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::ProviderStore;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

static NEXT_TEST_PORT: AtomicU32 = AtomicU32::new(35_000);

fn terminate_test_process(pid: u64) {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("kill");
        command.args(["-TERM", &pid.to_string()]);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        command
    };
    command.status().expect("test process should be terminable");
}

fn setup() -> (TempDir, String, String, u16) {
    let directory = TempDir::new().unwrap();
    let data_dir = directory.path().join("data");
    let codex_home = directory.path().join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let store = ProviderStore::open(data_dir.join("codex-route.db")).unwrap();
    store
        .insert(&Provider {
            id: "provider-a".into(),
            name: "Provider A".into(),
            settings_config: json!({
                "auth": {"OPENAI_API_KEY": "sk-test"},
                "config": "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"http://127.0.0.1:1/v1\"\n"
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
    let port = loop {
        let candidate = 30_000 + (NEXT_TEST_PORT.fetch_add(1, Ordering::Relaxed) % 20_000);
        let candidate = candidate as u16;
        if TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, candidate)).is_ok() {
            break candidate;
        }
    };
    (
        directory,
        data_dir.to_string_lossy().into_owned(),
        codex_home.to_string_lossy().into_owned(),
        port,
    )
}

fn command(data_dir: &str, codex_home: &str, subcommand: &str) -> Command {
    let mut command = Command::cargo_bin("codex-route").unwrap();
    command.args([
        "route",
        subcommand,
        "--data-dir",
        data_dir,
        "--codex-home",
        codex_home,
    ]);
    command
}

fn command_without_home(data_dir: &str, subcommand: &str) -> Command {
    let mut command = Command::cargo_bin("codex-route").unwrap();
    command.args(["route", subcommand, "--data-dir", data_dir]);
    command
}

#[tokio::test]
async fn embedded_route_service_activates_and_restores_config() {
    let (_directory, data_dir, codex_home, port) = setup();
    let data_dir = Path::new(&data_dir).to_path_buf();
    let codex_home = Path::new(&codex_home).to_path_buf();
    let config_path = codex_home.join("config.toml");
    let original_config = "model = \"gpt-5-codex\"\n";
    fs::write(&config_path, original_config).unwrap();

    let store = Arc::new(ProviderStore::open(data_dir.join("codex-route.db")).unwrap());
    let scan_config = ScanConfig {
        codex_home: codex_home.clone(),
        max_rollout_bytes: 64 * 1024,
    };
    let paths = LifecyclePaths::new(data_dir, codex_home);
    let mut service = EmbeddedRouteService::new(paths, store, scan_config, None, port);

    let activation = service.activate().await.unwrap();
    assert_eq!(activation.status, "active");
    assert_eq!(activation.pid, std::process::id());
    assert_eq!(service.status().unwrap().status, "active");
    assert_eq!(
        reqwest::get(format!("http://127.0.0.1:{port}/healthz"))
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::OK
    );
    assert!(fs::read_to_string(&config_path)
        .unwrap()
        .contains("codex-route-managed: v1"));

    let deactivation = service.deactivate().await.unwrap();
    assert_eq!(deactivation.status, "inactive");
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
    assert_eq!(service.status().unwrap().status, "inactive");
    assert!(!config_path.exists() || fs::read_to_string(&config_path).unwrap() == original_config);
}

#[tokio::test]
async fn embedded_route_service_rejects_invalid_activation_without_poisoning_state() {
    let (_directory, data_dir, codex_home, port) = setup();
    let data_dir = Path::new(&data_dir).to_path_buf();
    let codex_home = Path::new(&codex_home).to_path_buf();
    let store = Arc::new(ProviderStore::open(data_dir.join("codex-route.db")).unwrap());
    let scan_config = ScanConfig {
        codex_home: codex_home.clone(),
        max_rollout_bytes: 64 * 1024,
    };
    let paths = LifecyclePaths::new(data_dir, codex_home);
    let mut service = EmbeddedRouteService::new(paths, store, scan_config, None, port);

    assert!(matches!(
        service.activate_with(None, Some(0)).await,
        Err(codex_route::lifecycle::LifecycleError::InvalidPort)
    ));
    assert!(matches!(
        service.activate_with(Some("  ".into()), None).await,
        Err(codex_route::lifecycle::LifecycleError::InvalidProviderId)
    ));

    let activation = service.activate().await.unwrap();
    assert_eq!(activation.port, port);
    service.deactivate().await.unwrap();
}

#[test]
fn activate_status_deactivate_round_trip_preserves_auth_json() {
    let (_directory, data_dir, codex_home, port) = setup();
    let config_path = Path::new(&codex_home).join("config.toml");
    let auth_path = Path::new(&codex_home).join("auth.json");
    let original_config = "model = \"gpt-5-codex\"\n\n[model_providers.custom]\nbase_url = \"https://example.test/v1\"\n";
    let original_auth = "{\"access_token\":\"official-token\"}\n";
    fs::write(&config_path, original_config).unwrap();
    fs::write(&auth_path, original_auth).unwrap();

    let activated = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(
        activated.status.success(),
        "{}",
        String::from_utf8_lossy(&activated.stderr)
    );
    let activation: serde_json::Value = serde_json::from_slice(&activated.stdout).unwrap();
    assert_eq!(activation["status"], "active");
    assert!(activation["pid"].as_u64().unwrap() > 0);
    let managed = fs::read_to_string(&config_path).unwrap();
    assert!(managed.starts_with(codex_route::lifecycle::CONFIG_MARKER));
    let parsed = managed.parse::<toml::Value>().unwrap();
    assert_eq!(parsed["model_provider"].as_str(), Some("codex-route"));
    assert_eq!(
        parsed["model_providers"]["codex-route"]["requires_openai_auth"].as_bool(),
        Some(false)
    );
    let expected_url = format!("http://127.0.0.1:{port}/v1");
    assert_eq!(
        parsed["model_providers"]["codex-route"]["base_url"].as_str(),
        Some(expected_url.as_str())
    );
    assert_eq!(fs::read_to_string(&auth_path).unwrap(), original_auth);

    let status = command_without_home(&data_dir, "status").output().unwrap();
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["status"], "active");
    assert_eq!(status["serverReachable"], true);
    assert_eq!(status["configManaged"], true);

    let deactivated = command_without_home(&data_dir, "deactivate")
        .output()
        .unwrap();
    assert!(
        deactivated.status.success(),
        "{}",
        String::from_utf8_lossy(&deactivated.stderr)
    );
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
    assert_eq!(fs::read_to_string(&auth_path).unwrap(), original_auth);
    assert!(!Path::new(&data_dir).join("route-state.json").exists());
    assert!(!Path::new(&data_dir).join("route.lock").exists());
}

#[test]
fn lifecycle_recovers_a_live_service_when_state_pid_is_missing() {
    let (_directory, data_dir, codex_home, port) = setup();
    let config_path = Path::new(&codex_home).join("config.toml");
    let state_path = Path::new(&data_dir).join("route-state.json");
    let original = "model = \"gpt-5-codex\"\n";
    fs::write(&config_path, original).unwrap();

    command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .assert()
        .success();
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    let pid = state["pid"].as_u64().unwrap();
    state["pid"] = serde_json::Value::Null;
    fs::write(&state_path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

    let status = command_without_home(&data_dir, "status").output().unwrap();
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["status"], "active");
    assert_eq!(status["pid"].as_u64(), Some(pid));

    command_without_home(&data_dir, "deactivate")
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
}

#[test]
fn activation_rejects_port_conflicts_and_duplicate_processes() {
    let (_directory, data_dir, codex_home, port) = setup();
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
    let blocked = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("already in use"));
    drop(listener);
    assert!(!Path::new(&data_dir).join("route-state.json").exists());
    assert!(!Path::new(&codex_home).join("config.toml").exists());

    let first = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("already active"));
    let _ = command(&data_dir, &codex_home, "deactivate").output();
    drop(second);
    drop(blocked);
}

#[test]
fn deactivate_refuses_to_overwrite_external_config_changes() {
    let (_directory, data_dir, codex_home, port) = setup();
    let config_path = Path::new(&codex_home).join("config.toml");
    let original = "model = \"gpt-5-codex\"\n";
    fs::write(&config_path, original).unwrap();
    let activated = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(activated.status.success());
    let managed = fs::read_to_string(&config_path).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .unwrap()
        .write_all(b"# changed outside codex-route\n")
        .unwrap();
    command(&data_dir, &codex_home, "deactivate")
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("externally modified"));
    let status = command_without_home(&data_dir, "status").output().unwrap();
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["status"], "external_modified");
    assert_eq!(status["externalModification"], true);
    assert!(fs::read_to_string(&config_path)
        .unwrap()
        .contains("changed outside codex-route"));

    // Restore the managed bytes so the child can be shut down and the fixture cleaned up.
    fs::write(&config_path, managed).unwrap();
    command(&data_dir, &codex_home, "deactivate")
        .assert()
        .success();
}

#[test]
fn deactivate_refuses_to_restore_a_tampered_backup() {
    let (_directory, data_dir, codex_home, port) = setup();
    let config_path = Path::new(&codex_home).join("config.toml");
    let backup_path = Path::new(&data_dir).join("codex-config.toml.bak");
    let original = "model = \"gpt-5-codex\"\n";
    fs::write(&config_path, original).unwrap();

    command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .assert()
        .success();
    let managed = fs::read_to_string(&config_path).unwrap();
    fs::write(&backup_path, "model = \"tampered\"\n").unwrap();

    command_without_home(&data_dir, "deactivate")
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("externally modified"));
    assert_eq!(fs::read_to_string(&config_path).unwrap(), managed);
    assert!(Path::new(&data_dir).join("route-state.json").exists());

    // Restore the backup fixture so the route process can be stopped and the
    // temporary directory can be cleaned up normally.
    fs::write(&backup_path, original).unwrap();
    command_without_home(&data_dir, "deactivate")
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
}

#[test]
fn interrupted_activation_and_restore_states_are_recoverable() {
    let (_directory, data_dir, codex_home, port) = setup();
    let config_path = Path::new(&codex_home).join("config.toml");
    let original = "model = \"gpt-5-codex\"\n";
    fs::write(&config_path, original).unwrap();

    let first = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first_state: serde_json::Value =
        serde_json::from_slice(&fs::read(Path::new(&data_dir).join("route-state.json")).unwrap())
            .unwrap();
    let first_pid = first_state["pid"].as_u64().unwrap();
    terminate_test_process(first_pid);
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Simulate a crash after the state journal was written but before config projection.
    fs::write(&config_path, original).unwrap();
    let retry = command(&data_dir, &codex_home, "activate")
        .args(["--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );

    // Simulate a crash after restoring config but before deleting lifecycle state.
    let retry_state: serde_json::Value =
        serde_json::from_slice(&fs::read(Path::new(&data_dir).join("route-state.json")).unwrap())
            .unwrap();
    let retry_pid = retry_state["pid"].as_u64().unwrap();
    terminate_test_process(retry_pid);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let backup = fs::read(Path::new(&data_dir).join("codex-config.toml.bak")).unwrap();
    fs::write(&config_path, backup).unwrap();
    let cleaned = command_without_home(&data_dir, "deactivate")
        .output()
        .unwrap();
    assert!(
        cleaned.status.success(),
        "{}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    assert!(!Path::new(&data_dir).join("route-state.json").exists());
}
