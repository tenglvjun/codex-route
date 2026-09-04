use codex_route::cc_switch_import::{CcSwitchImportError, CcSwitchImporter, ConflictPolicy};
use codex_route::provider_store::ProviderStore;
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn create_source(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE providers (
                id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                name TEXT NOT NULL,
                settings_config TEXT NOT NULL,
                website_url TEXT,
                category TEXT,
                created_at INTEGER,
                sort_index INTEGER,
                notes TEXT,
                icon TEXT,
                icon_color TEXT,
                meta TEXT NOT NULL DEFAULT '{}',
                is_current INTEGER NOT NULL DEFAULT 0,
                in_failover_queue INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (id, app_type)
            );",
        )
        .unwrap();
    let good = json!({
        "auth": {"OPENAI_API_KEY": "sk-source-secret"},
        "config": "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"https://example.test/v1\""
    });
    let keyless = json!({"config": "model = \"gpt-5-codex\""});
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta, is_current)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["claude-row", "claude", "Claude", "{}", "{}", 0],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta, is_current)
             VALUES (?1, 'codex', ?2, ?3, '{}', 1)",
            params!["good", "Good", serde_json::to_string(&good).unwrap()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES (?1, 'codex', ?2, ?3, ?4)",
            params![
                "keyless",
                "Keyless",
                serde_json::to_string(&keyless).unwrap(),
                r#"{"providerType":"codex_oauth"}"#
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES ('broken', 'codex', 'Broken', '{\"config\":\"not = [valid\"', '{}')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES ('placeholder', 'codex', 'Placeholder', ?1, '{}')",
            params![r#"{"auth":{"OPENAI_API_KEY":"PROXY_MANAGED"}}"#],
        )
        .unwrap();
}

#[test]
fn scans_only_codex_rows_and_leaves_source_unchanged() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("cc-switch.db");
    create_source(&source);
    let before = fs::read(&source).unwrap();

    let scan = CcSwitchImporter::new(&source)
        .read_codex_providers()
        .unwrap();
    assert_eq!(scan.candidates.len(), 2);
    assert_eq!(scan.rejected.len(), 2);
    assert!(scan.candidates.iter().all(|candidate| {
        candidate
            .provider
            .source
            .source_id()
            .is_some_and(|id| id != "claude-row")
    }));
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn imports_only_selected_cc_switch_providers() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("cc-switch.db");
    create_source(&source);
    let scan = CcSwitchImporter::new(&source)
        .read_codex_providers()
        .unwrap();

    let selected = scan.select_provider_ids(&["keyless".to_string()]).unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    let report = store
        .import_scan_transaction(&selected, ConflictPolicy::Skip)
        .unwrap();

    assert_eq!(report.imported, 1);
    assert_eq!(report.rejected.len(), 2);
    let providers = store.list().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].source.source_id(), Some("keyless"));
}

#[test]
fn rejects_provider_ids_that_are_not_in_the_latest_scan() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("cc-switch.db");
    create_source(&source);
    let scan = CcSwitchImporter::new(&source)
        .read_codex_providers()
        .unwrap();

    let error = scan
        .select_provider_ids(&["missing".to_string()])
        .unwrap_err();

    assert!(matches!(
        error,
        CcSwitchImportError::UnknownProviders(ids) if ids == vec!["missing".to_string()]
    ));
}
