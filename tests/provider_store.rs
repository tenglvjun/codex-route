use codex_route::cc_switch_import::{ConflictPolicy, ImportCandidate};
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::{ProviderStore, UpsertOutcome};
use serde_json::json;
use tempfile::tempdir;

fn provider(id: &str, source: ProviderSource) -> Provider {
    Provider {
        id: id.to_string(),
        name: format!("Provider {id}"),
        settings_config: json!({
            "auth": {"OPENAI_API_KEY": "sk-test-secret"},
            "config": "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"https://example.test/v1\""
        }),
        website_url: None,
        category: Some("custom".to_string()),
        created_at: Some(1),
        sort_index: Some(0),
        notes: None,
        icon: None,
        icon_color: None,
        meta: json!({"unknown": {"kept": true}}),
        in_failover_queue: false,
        is_current: false,
        source,
    }
}

#[test]
fn store_round_trips_unknown_json_and_source_identity() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    let source = ProviderSource::CcSwitch {
        source_id: "source-p1".to_string(),
        source_updated_at: Some(42),
    };
    store.insert(&provider("p1", source)).unwrap();

    let loaded = store.get("p1").unwrap().unwrap();
    assert_eq!(loaded.meta["unknown"]["kept"], json!(true));
    assert_eq!(store.find_imported("source-p1").unwrap().unwrap().id, "p1");
}

#[test]
fn import_preserves_existing_current_state_and_is_idempotent() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    let mut current = provider("local", ProviderSource::Local);
    current.is_current = true;
    current.in_failover_queue = true;
    store.insert(&current).unwrap();

    let candidate = ImportCandidate {
        provider: provider(
            "imported",
            ProviderSource::CcSwitch {
                source_id: "source-imported".to_string(),
                source_updated_at: None,
            },
        ),
        source_is_current: true,
    };
    let first = store
        .import_transaction(std::slice::from_ref(&candidate), ConflictPolicy::Skip)
        .unwrap();
    assert_eq!(first.imported, 1);
    assert!(!store.get("imported").unwrap().unwrap().is_current);

    let second = store
        .import_transaction(std::slice::from_ref(&candidate), ConflictPolicy::Skip)
        .unwrap();
    assert_eq!(second.skipped, 1);

    let replacement = store.upsert_imported(&candidate.provider, true).unwrap();
    assert_eq!(replacement, UpsertOutcome::Replaced);
    let local = store.get("local").unwrap().unwrap();
    assert!(local.is_current);
    assert!(local.in_failover_queue);
}

#[test]
fn local_id_collision_uses_stable_ccswitch_namespace() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    store
        .insert(&provider("same-id", ProviderSource::Local))
        .unwrap();
    let candidate = ImportCandidate {
        provider: provider(
            "same-id",
            ProviderSource::CcSwitch {
                source_id: "same-id".to_string(),
                source_updated_at: None,
            },
        ),
        source_is_current: false,
    };
    let report = store
        .import_transaction(std::slice::from_ref(&candidate), ConflictPolicy::Rename)
        .unwrap();
    assert_eq!(report.renamed, 1);
    assert!(store.get("same-id").unwrap().unwrap().source == ProviderSource::Local);
    assert!(store.get("ccswitch-same-id").unwrap().is_some());

    let repeat = store
        .import_transaction(std::slice::from_ref(&candidate), ConflictPolicy::Rename)
        .unwrap();
    assert_eq!(repeat.replaced, 1);
    assert!(store.get("ccswitch-same-id-2").unwrap().is_none());
}
