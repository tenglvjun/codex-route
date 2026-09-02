use codex_route::cc_switch_import::{ConflictPolicy, ImportCandidate};
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::{ProviderStore, UpsertOutcome};
use codex_route::provider_store::{ProviderStoreError, UpsertRouteRuleOutcome};
use codex_route::workspace_rule::{normalize_workspace_path, WorkspacePathError};
use serde_json::json;
use std::path::Path;
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

#[test]
fn route_rules_round_trip_and_enforce_provider_and_duplicate_checks() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    let provider_a = provider("provider-a", ProviderSource::Local);
    let provider_b = provider("provider-b", ProviderSource::Local);
    store.insert(&provider_a).unwrap();
    store.insert(&provider_b).unwrap();

    let workspace = directory.path().join("projects").join("app");
    let inserted = store
        .upsert_route_rule(&workspace, "provider-a", false)
        .unwrap();
    assert_eq!(inserted, UpsertRouteRuleOutcome::Inserted);

    let rules = store.list_route_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0].workspace,
        normalize_workspace_path(&workspace).unwrap()
    );
    assert_eq!(rules[0].provider_id, "provider-a");

    assert!(matches!(
        store.upsert_route_rule(&workspace, "provider-b", false),
        Err(ProviderStoreError::RouteRuleAlreadyExists(_))
    ));
    assert_eq!(
        store
            .upsert_route_rule(&workspace, "provider-b", true)
            .unwrap(),
        UpsertRouteRuleOutcome::Replaced
    );
    assert_eq!(
        store
            .get_route_rule(&workspace)
            .unwrap()
            .unwrap()
            .provider_id,
        "provider-b"
    );

    assert!(matches!(
        store.upsert_route_rule(&workspace, "missing", false),
        Err(ProviderStoreError::ProviderNotFound(id)) if id == "missing"
    ));
}

#[test]
fn route_rules_remove_returns_rule_and_rejects_missing_workspace() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    store
        .insert(&provider("provider-a", ProviderSource::Local))
        .unwrap();
    let workspace = directory.path().join("project");
    store
        .upsert_route_rule(&workspace, "provider-a", false)
        .unwrap();

    let removed = store.remove_route_rule(&workspace).unwrap();
    assert_eq!(removed.provider_id, "provider-a");
    assert!(store.list_route_rules().unwrap().is_empty());
    assert!(matches!(
        store.remove_route_rule(&workspace),
        Err(ProviderStoreError::RouteRuleNotFound(_))
    ));
}

#[test]
fn route_rules_reject_relative_paths() {
    let directory = tempdir().unwrap();
    let store = ProviderStore::open(directory.path().join("codex-route.db")).unwrap();
    store
        .insert(&provider("provider-a", ProviderSource::Local))
        .unwrap();

    assert!(matches!(
        store.upsert_route_rule(Path::new("relative"), "provider-a", false),
        Err(ProviderStoreError::InvalidWorkspace(
            WorkspacePathError::Relative(_)
        ))
    ));
}
