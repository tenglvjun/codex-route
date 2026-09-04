use axum::body::{to_bytes, Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use codex_route::config::ScanConfig;
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::ProviderStore;
use codex_route::route::{
    build_router, extract_codex_session_id, filter_request_headers, upstream_responses_url,
    RouteServer, RouteServerError, RouteStartupError, RouteState,
};
use futures_util::stream;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;

fn write_rollout(home: &std::path::Path, session_id: &str, thread_id: &str, cwd: &std::path::Path) {
    let directory = home.join("sessions/2026/09/03");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("rollout-{thread_id}.jsonl"));
    let line = json!({
        "timestamp": "2026-09-03T12:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "session_id": session_id,
            "id": thread_id,
            "timestamp": "2026-09-03T12:00:00Z",
            "cwd": cwd.to_string_lossy(),
            "originator": "codex",
            "cli_version": "test"
        }
    });
    std::fs::write(path, format!("{line}\n")).unwrap();
}

fn write_archived_rollout(
    home: &std::path::Path,
    session_id: &str,
    thread_id: &str,
    cwd: &std::path::Path,
) {
    let directory = home.join("archived_sessions/2026/09/03");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("rollout-{thread_id}.jsonl"));
    let line = json!({
        "timestamp": "2026-09-03T12:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "session_id": session_id,
            "id": thread_id,
            "timestamp": "2026-09-03T12:00:00Z",
            "cwd": cwd.to_string_lossy(),
            "originator": "codex",
            "cli_version": "test"
        }
    });
    std::fs::write(path, format!("{line}\n")).unwrap();
}

#[derive(Clone, Default)]
struct Capture {
    authorization: Arc<Mutex<Option<String>>>,
    request_id: Arc<Mutex<Option<String>>>,
    uri: Arc<Mutex<Option<String>>>,
    body: Arc<Mutex<Vec<u8>>>,
}

async fn spawn_router(router: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (address, task)
}

#[tokio::test]
async fn embedded_route_server_starts_serves_health_and_releases_port() {
    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "upstream",
            "https://api.example/v1",
            Some("responses"),
            Some("sk-test"),
        ),
    );
    let mut server = RouteServer::new(state, 0);

    let started = server.start().await.unwrap();
    assert!(started.active);
    let address = started.address.expect("server should expose its address");
    assert_eq!(started.port, Some(address.port()));

    let response = reqwest::Client::new()
        .get(format!("http://{address}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        json!({"status": "ok"})
    );

    server.stop().await.unwrap();
    assert!(!server.status().active);
    assert!(server.status().address.is_none());

    let listener = TcpListener::bind(address).await;
    assert!(
        listener.is_ok(),
        "stopping the route server should release its port"
    );
}

#[tokio::test]
async fn embedded_route_server_rejects_duplicate_start_and_not_running_stop() {
    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "upstream",
            "https://api.example/v1",
            Some("responses"),
            Some("sk-test"),
        ),
    );
    let mut server = RouteServer::new(state, 0);

    server.start().await.unwrap();
    assert!(matches!(
        server.start().await,
        Err(RouteServerError::AlreadyRunning)
    ));
    server.stop().await.unwrap();
    assert!(matches!(
        server.stop().await,
        Err(RouteServerError::NotRunning)
    ));
}

#[tokio::test]
async fn embedded_route_server_reports_port_conflicts() {
    let occupied = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = occupied.local_addr().unwrap().port();
    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "upstream",
            "https://api.example/v1",
            Some("responses"),
            Some("sk-test"),
        ),
    );
    let mut server = RouteServer::new(state, port);

    assert!(matches!(
        server.start().await,
        Err(RouteServerError::PortInUse(actual)) if actual == port
    ));
}

async fn capture_responses(
    State(capture): State<Capture>,
    request: Request<Body>,
) -> Response<Body> {
    let (parts, body) = request.into_parts();
    *capture.authorization.lock().unwrap() = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    *capture.request_id.lock().unwrap() = parts
        .headers
        .get("x-client-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    *capture.uri.lock().unwrap() = Some(parts.uri.to_string());
    *capture.body.lock().unwrap() = to_bytes(body, 1024 * 1024).await.unwrap().to_vec();

    let mut response = Response::new(Body::from(r#"{"id":"resp_test"}"#));
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

async fn stream_responses() -> Response<Body> {
    let chunks = stream::iter(vec![
        Ok::<_, Infallible>(Bytes::from_static(b"event: response.created\n\n")),
        Ok::<_, Infallible>(Bytes::from_static(b"data: [DONE]\n\n")),
    ]);
    let mut response = Response::new(Body::from_stream(chunks));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
}

async fn compressed_stream_response() -> Response<Body> {
    let mut response = Response::new(Body::from(Bytes::from_static(b"encoded-sse")));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
    response
}

fn provider(
    id: &str,
    base_url: &str,
    wire_api: Option<&str>,
    credential: Option<&str>,
) -> Provider {
    let mut config = format!(
        "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"{base_url}\"\n"
    );
    if let Some(wire_api) = wire_api {
        config.push_str(&format!("wire_api = \"{wire_api}\"\n"));
    }
    let mut settings = json!({"config": config});
    if let Some(credential) = credential {
        settings["auth"] = json!({"OPENAI_API_KEY": credential});
    }
    Provider {
        id: id.to_string(),
        name: id.to_string(),
        settings_config: settings,
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
    }
}

fn route_state(directory: &TempDir, provider: Provider) -> RouteState {
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    store.insert(&provider).unwrap();
    RouteState::new(store, None).unwrap()
}

fn dynamic_route_state(
    directory: &TempDir,
    providers: &[Provider],
    codex_home: &std::path::Path,
) -> (RouteState, Arc<ProviderStore>) {
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    for provider in providers {
        store.insert(provider).unwrap();
    }
    let state = RouteState::with_scan_config(
        store.clone(),
        None,
        ScanConfig {
            codex_home: codex_home.to_path_buf(),
            max_rollout_bytes: 64 * 1024,
        },
    )
    .unwrap();
    (state, store)
}

#[test]
fn upstream_url_appends_responses_without_double_slashes() {
    assert_eq!(
        upstream_responses_url("https://api.example/v1/", Some("stream=true"))
            .unwrap()
            .as_str(),
        "https://api.example/v1/responses?stream=true"
    );
}

#[test]
fn extracts_codex_session_from_header_or_metadata_without_using_previous_response_id() {
    let mut headers = HeaderMap::new();
    headers.insert("session_id", HeaderValue::from_static("header-session"));
    assert_eq!(
        extract_codex_session_id(
            &headers,
            &json!({"metadata": {"session_id": "body-session"}})
        ),
        Some("header-session".to_string())
    );

    let empty = HeaderMap::new();
    assert_eq!(
        extract_codex_session_id(
            &empty,
            &json!({"metadata": {"session_id": "body-session"}, "previous_response_id": "resp-1"})
        ),
        Some("body-session".to_string())
    );
    assert_eq!(
        extract_codex_session_id(&empty, &json!({"previous_response_id": "resp-1"})),
        None
    );
}

#[tokio::test]
async fn routes_session_workspace_to_rule_provider_and_falls_back_to_current() {
    let home = TempDir::new().unwrap();
    let workspace_a = home.path().join("project-a");
    let workspace_b = home.path().join("project-b");
    std::fs::create_dir_all(&workspace_a).unwrap();
    std::fs::create_dir_all(&workspace_b).unwrap();
    write_rollout(home.path(), "session-a", "thread-a", &workspace_a);
    write_rollout(home.path(), "session-b", "thread-b", &workspace_b);

    let capture_a = Capture::default();
    let capture_b = Capture::default();
    let upstream_a = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(capture_a.clone());
    let upstream_b = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(capture_b.clone());
    let (address_a, task_a) = spawn_router(upstream_a).await;
    let (address_b, task_b) = spawn_router(upstream_b).await;

    let directory = TempDir::new().unwrap();
    let mut provider_a = provider(
        "provider-a",
        &format!("http://{address_a}/v1"),
        Some("responses"),
        Some("key-a"),
    );
    provider_a.is_current = true;
    let provider_b = provider(
        "provider-b",
        &format!("http://{address_b}/v1"),
        Some("responses"),
        Some("key-b"),
    );
    let mut provider_b = provider_b;
    provider_b.is_current = false;
    let (state, store) = dynamic_route_state(&directory, &[provider_a, provider_b], home.path());
    store
        .upsert_route_rule(&workspace_a, "provider-a", false)
        .unwrap();
    store
        .upsert_route_rule(&workspace_b, "provider-b", false)
        .unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let client = reqwest::Client::new();

    let response_a = client
        .post(format!("http://{route_address}/v1/responses"))
        .header("session_id", "session-a")
        .json(&json!({"model": "gpt-5"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response_a.status(), StatusCode::CREATED);
    assert_eq!(
        capture_a.authorization.lock().unwrap().as_deref(),
        Some("Bearer key-a")
    );
    assert!(capture_b.body.lock().unwrap().is_empty());

    let response_b = client
        .post(format!("http://{route_address}/v1/responses"))
        .json(&json!({"metadata": {"session_id": "session-b"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response_b.status(), StatusCode::CREATED);
    assert_eq!(
        capture_b.authorization.lock().unwrap().as_deref(),
        Some("Bearer key-b")
    );

    let response_fallback = client
        .post(format!("http://{route_address}/v1/responses"))
        .header("session_id", "unknown-session")
        .json(&json!({"model": "gpt-5"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response_fallback.status(), StatusCode::CREATED);
    assert_eq!(
        *capture_a.authorization.lock().unwrap(),
        Some("Bearer key-a".to_string())
    );

    route_task.abort();
    task_a.abort();
    task_b.abort();
}

#[tokio::test]
async fn fixed_provider_overrides_workspace_route() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("project");
    std::fs::create_dir_all(&workspace).unwrap();
    write_rollout(home.path(), "session-a", "thread-a", &workspace);
    let capture = Capture::default();
    let upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(capture.clone());
    let (upstream_address, upstream_task) = spawn_router(upstream).await;
    let directory = TempDir::new().unwrap();
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-key"),
    );
    current.is_current = true;
    let fixed = provider(
        "fixed",
        &format!("http://{upstream_address}/v1"),
        Some("responses"),
        Some("fixed-key"),
    );
    let mut fixed = fixed;
    fixed.is_current = false;
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    store.insert(&current).unwrap();
    store.insert(&fixed).unwrap();
    store
        .upsert_route_rule(&workspace, "current", false)
        .unwrap();
    let state = RouteState::with_scan_config(
        store,
        Some("fixed".to_string()),
        ScanConfig {
            codex_home: home.path().to_path_buf(),
            max_rollout_bytes: 64 * 1024,
        },
    )
    .unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .header("session_id", "session-a")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        capture.authorization.lock().unwrap().as_deref(),
        Some("Bearer fixed-key")
    );

    route_task.abort();
    upstream_task.abort();
}

#[tokio::test]
async fn conflicting_or_missing_workspaces_fall_back_to_current_provider() {
    let home = TempDir::new().unwrap();
    let workspace_a = home.path().join("project-a");
    let workspace_b = home.path().join("project-b");
    let missing_workspace = home.path().join("missing-project");
    std::fs::create_dir_all(&workspace_a).unwrap();
    std::fs::create_dir_all(&workspace_b).unwrap();
    write_rollout(home.path(), "conflicting", "thread-a", &workspace_a);
    write_rollout(home.path(), "conflicting", "thread-b", &workspace_b);
    write_rollout(home.path(), "missing", "thread-missing", &missing_workspace);

    let current_capture = Capture::default();
    let mapped_capture = Capture::default();
    let current_upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(current_capture.clone());
    let mapped_upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(mapped_capture.clone());
    let (current_address, current_task) = spawn_router(current_upstream).await;
    let (mapped_address, mapped_task) = spawn_router(mapped_upstream).await;

    let directory = TempDir::new().unwrap();
    let mut current = provider(
        "current",
        &format!("http://{current_address}/v1"),
        Some("responses"),
        Some("current-key"),
    );
    current.is_current = true;
    let mapped = provider(
        "mapped",
        &format!("http://{mapped_address}/v1"),
        Some("responses"),
        Some("mapped-key"),
    );
    let mut mapped = mapped;
    mapped.is_current = false;
    let (state, store) = dynamic_route_state(&directory, &[current, mapped], home.path());
    store
        .upsert_route_rule(&workspace_a, "mapped", false)
        .unwrap();
    store
        .upsert_route_rule(&missing_workspace, "mapped", false)
        .unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let client = reqwest::Client::new();

    for session_id in ["conflicting", "missing"] {
        let response = client
            .post(format!("http://{route_address}/v1/responses"))
            .header("session_id", session_id)
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    assert_eq!(
        *current_capture.authorization.lock().unwrap(),
        Some("Bearer current-key".to_string())
    );
    assert!(mapped_capture.body.lock().unwrap().is_empty());

    route_task.abort();
    current_task.abort();
    mapped_task.abort();
}

#[tokio::test]
async fn archived_session_falls_back_to_current_provider() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("archived-project");
    std::fs::create_dir_all(&workspace).unwrap();
    write_archived_rollout(
        home.path(),
        "archived-session",
        "thread-archived",
        &workspace,
    );

    let current_capture = Capture::default();
    let mapped_capture = Capture::default();
    let current_upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(current_capture.clone());
    let mapped_upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(mapped_capture.clone());
    let (current_address, current_task) = spawn_router(current_upstream).await;
    let (mapped_address, mapped_task) = spawn_router(mapped_upstream).await;

    let directory = TempDir::new().unwrap();
    let mut current = provider(
        "current",
        &format!("http://{current_address}/v1"),
        Some("responses"),
        Some("current-key"),
    );
    current.is_current = true;
    let mapped = provider(
        "mapped",
        &format!("http://{mapped_address}/v1"),
        Some("responses"),
        Some("mapped-key"),
    );
    let (state, store) = dynamic_route_state(&directory, &[current, mapped], home.path());
    store
        .upsert_route_rule(&workspace, "mapped", false)
        .unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .header("session_id", "archived-session")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        *current_capture.authorization.lock().unwrap(),
        Some("Bearer current-key".to_string())
    );
    assert!(mapped_capture.body.lock().unwrap().is_empty());

    route_task.abort();
    current_task.abort();
    mapped_task.abort();
}

#[tokio::test]
async fn matching_rule_provider_configuration_errors_are_not_silently_skipped() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("project");
    std::fs::create_dir_all(&workspace).unwrap();
    write_rollout(home.path(), "session-a", "thread-a", &workspace);

    let directory = TempDir::new().unwrap();
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-key"),
    );
    current.is_current = true;
    let mut invalid = provider(
        "invalid",
        "https://invalid.example/v1",
        Some("chat"),
        Some("invalid-key"),
    );
    invalid.is_current = false;
    let (state, store) = dynamic_route_state(&directory, &[current, invalid], home.path());
    store
        .upsert_route_rule(&workspace, "invalid", false)
        .unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .header("session_id", "session-a")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "responses_only"
    );

    route_task.abort();
}

#[test]
fn request_filter_removes_client_auth_and_hop_by_hop_headers() {
    let mut input = HeaderMap::new();
    input.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer client"),
    );
    input.insert(header::HOST, HeaderValue::from_static("localhost"));
    input.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    input.insert("x-request-id", HeaderValue::from_static("request-1"));
    let output = filter_request_headers(&input, Some("provider-secret")).unwrap();
    assert!(!output.contains_key(header::HOST));
    assert_eq!(output[header::CONTENT_TYPE], "application/json");
    assert_eq!(output["x-request-id"], "request-1");
    assert_eq!(output[header::AUTHORIZATION], "Bearer provider-secret");
}

#[test]
fn startup_validation_reports_provider_configuration_categories() {
    let directory = TempDir::new().unwrap();
    let missing_url = route_state(
        &directory,
        provider("missing-url", "", Some("responses"), Some("sk-test")),
    );
    assert!(matches!(
        missing_url.validate_selection(),
        Err(RouteStartupError::MissingBaseUrl(id)) if id == "missing-url"
    ));

    let directory = TempDir::new().unwrap();
    let missing_credential = route_state(
        &directory,
        provider(
            "missing-credential",
            "https://api.example/v1",
            Some("responses"),
            None,
        ),
    );
    assert!(matches!(
        missing_credential.validate_selection(),
        Err(RouteStartupError::MissingCredential(id)) if id == "missing-credential"
    ));

    let directory = TempDir::new().unwrap();
    let invalid_url = route_state(
        &directory,
        provider(
            "invalid-url",
            "not-an-url",
            Some("responses"),
            Some("sk-test"),
        ),
    );
    assert!(matches!(
        invalid_url.validate_selection(),
        Err(RouteStartupError::InvalidBaseUrl(id)) if id == "invalid-url"
    ));
}

#[tokio::test]
async fn forwards_responses_request_and_replaces_client_credential() {
    let capture = Capture::default();
    let upstream = Router::new()
        .route("/v1/responses", post(capture_responses))
        .with_state(capture.clone());
    let (upstream_address, upstream_task) = spawn_router(upstream).await;

    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "upstream",
            &format!("http://{upstream_address}/v1/"),
            Some("responses"),
            Some("sk-upstream-test"),
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses?stream=true"))
        .header(header::AUTHORIZATION, "Bearer client-secret")
        .header("x-client-request-id", "request-1")
        .json(&json!({"model": "gpt-5", "stream": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.text().await.unwrap(), r#"{"id":"resp_test"}"#);
    assert_eq!(
        capture.authorization.lock().unwrap().as_deref(),
        Some("Bearer sk-upstream-test")
    );
    assert_eq!(
        capture.request_id.lock().unwrap().as_deref(),
        Some("request-1")
    );
    assert_eq!(
        capture.uri.lock().unwrap().as_deref(),
        Some("/v1/responses?stream=true")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&capture.body.lock().unwrap()).unwrap(),
        json!({"model": "gpt-5", "stream": true})
    );

    route_task.abort();
    upstream_task.abort();
}

#[tokio::test]
async fn forwards_compact_request_to_compact_upstream_path() {
    let capture = Capture::default();
    let upstream = Router::new()
        .route("/v1/responses/compact", post(capture_responses))
        .with_state(capture.clone());
    let (upstream_address, upstream_task) = spawn_router(upstream).await;

    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "upstream",
            &format!("http://{upstream_address}/v1/"),
            Some("responses"),
            Some("sk-upstream-test"),
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!(
            "http://{route_address}/responses/compact?stream=false"
        ))
        .header(header::AUTHORIZATION, "Bearer client-secret")
        .json(&json!({"model": "gpt-5", "input": "compact"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        capture.authorization.lock().unwrap().as_deref(),
        Some("Bearer sk-upstream-test")
    );
    assert_eq!(
        capture.uri.lock().unwrap().as_deref(),
        Some("/v1/responses/compact?stream=false")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&capture.body.lock().unwrap()).unwrap(),
        json!({"model": "gpt-5", "input": "compact"})
    );

    route_task.abort();
    upstream_task.abort();
}

#[tokio::test]
async fn serves_codex_models_catalog_shape() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    for path in ["/models", "/v1/models"] {
        let response = reqwest::Client::new()
            .get(format!("http://{route_address}{path}"))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK, "path {path}");
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap(),
            json!({"models": []}),
            "path {path}"
        );
    }

    route_task.abort();
}

#[tokio::test]
async fn management_lists_redacted_provider_summaries() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-secret"),
    );
    current.is_current = true;
    let mut other = provider(
        "other",
        "https://other.example/v1",
        Some("responses"),
        Some("other-secret"),
    );
    other.is_current = false;
    store.insert(&current).unwrap();
    store.insert(&other).unwrap();
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .get(format!("http://{route_address}/api/providers"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        json!([
            {
                "id": "current",
                "name": "current",
                "category": null,
                "source": "local",
                "isCurrent": true
            },
            {
                "id": "other",
                "name": "other",
                "category": null,
                "source": "local",
                "isCurrent": false
            }
        ])
    );

    route_task.abort();
}

#[tokio::test]
async fn management_manages_workspace_route_rules() {
    let directory = TempDir::new().unwrap();
    let workspace = directory.path().join("project");
    std::fs::create_dir_all(&workspace).unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-secret"),
    );
    current.is_current = true;
    let mut mapped = provider(
        "mapped",
        "https://mapped.example/v1",
        Some("responses"),
        Some("mapped-secret"),
    );
    mapped.is_current = false;
    store.insert(&current).unwrap();
    store.insert(&mapped).unwrap();
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let client = reqwest::Client::new();
    let workspace_text = workspace.to_string_lossy().to_string();

    let inserted = client
        .put(format!("http://{route_address}/api/route-rules"))
        .json(&json!({"workspace": workspace_text, "providerId": "mapped"}))
        .send()
        .await
        .unwrap();
    assert_eq!(inserted.status(), StatusCode::OK);
    assert_eq!(
        inserted.json::<serde_json::Value>().await.unwrap()["action"],
        "inserted"
    );

    let listed = client
        .get(format!("http://{route_address}/api/route-rules"))
        .send()
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_rules = listed.json::<serde_json::Value>().await.unwrap();
    assert_eq!(listed_rules.as_array().unwrap().len(), 1);
    assert_eq!(listed_rules[0]["providerId"], "mapped");

    let duplicate = client
        .put(format!("http://{route_address}/api/route-rules"))
        .json(&json!({"workspace": workspace_text, "providerId": "current"}))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    assert_eq!(
        duplicate.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "route_rule_exists"
    );

    let replaced = client
        .put(format!("http://{route_address}/api/route-rules"))
        .json(&json!({
            "workspace": workspace_text,
            "providerId": "current",
            "replace": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(replaced.status(), StatusCode::OK);
    assert_eq!(
        replaced.json::<serde_json::Value>().await.unwrap()["action"],
        "replaced"
    );

    let removed = client
        .delete(format!("http://{route_address}/api/route-rules"))
        .json(&json!({"workspace": workspace_text}))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(
        removed.json::<serde_json::Value>().await.unwrap()["providerId"],
        "current"
    );

    let missing = client
        .delete(format!("http://{route_address}/api/route-rules"))
        .json(&json!({"workspace": workspace_text}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "route_rule_not_found"
    );

    let relative = client
        .put(format!("http://{route_address}/api/route-rules"))
        .json(&json!({"workspace": "relative", "providerId": "current"}))
        .send()
        .await
        .unwrap();
    assert_eq!(relative.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        relative.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "invalid_workspace"
    );

    route_task.abort();
}

#[tokio::test]
async fn management_switches_current_provider_atomically() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-secret"),
    );
    current.is_current = true;
    let mut next = provider(
        "next",
        "https://next.example/v1",
        Some("responses"),
        Some("next-secret"),
    );
    next.is_current = false;
    store.insert(&current).unwrap();
    store.insert(&next).unwrap();
    let state = RouteState::new(store.clone(), None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let client = reqwest::Client::new();

    let switched = client
        .put(format!("http://{route_address}/api/providers/current"))
        .json(&json!({"providerId": "next"}))
        .send()
        .await
        .unwrap();
    assert_eq!(switched.status(), StatusCode::OK);
    assert_eq!(
        switched.json::<serde_json::Value>().await.unwrap(),
        json!({
            "id": "next",
            "name": "next",
            "category": null,
            "source": "local",
            "isCurrent": true
        })
    );

    let providers = store.list().unwrap();
    assert!(!providers[0].is_current);
    assert!(providers[1].is_current);
    assert_eq!(providers[1].id, "next");

    let missing = client
        .put(format!("http://{route_address}/api/providers/current"))
        .json(&json!({"providerId": "missing"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "provider_not_found"
    );
    assert_eq!(
        store
            .list()
            .unwrap()
            .iter()
            .filter(|provider| provider.is_current)
            .count(),
        1
    );
    assert_eq!(
        store
            .list()
            .unwrap()
            .into_iter()
            .find(|provider| provider.is_current)
            .unwrap()
            .id,
        "next"
    );

    route_task.abort();
}

#[tokio::test]
async fn management_reports_route_status_without_credentials() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let mut current = provider(
        "current",
        "https://current.example/v1",
        Some("responses"),
        Some("current-secret"),
    );
    current.is_current = true;
    store.insert(&current).unwrap();
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let status = reqwest::Client::new()
        .get(format!("http://{route_address}/api/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        status.json::<serde_json::Value>().await.unwrap(),
        json!({
            "status": "ok",
            "provider": {
                "id": "current",
                "name": "current",
                "category": null,
                "source": "local",
                "isCurrent": true
            },
            "providerConfiguration": {"valid": true}
        })
    );

    route_task.abort();

    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let mut invalid = provider(
        "invalid",
        "https://invalid.example/v1",
        Some("responses"),
        None,
    );
    invalid.is_current = true;
    store.insert(&invalid).unwrap();
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let status = reqwest::Client::new()
        .get(format!("http://{route_address}/api/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        status.json::<serde_json::Value>().await.unwrap()["providerConfiguration"],
        json!({"valid": false, "error": "missing_credential"})
    );

    route_task.abort();

    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let state = RouteState::new(store, None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let status = reqwest::Client::new()
        .get(format!("http://{route_address}/api/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        status.json::<serde_json::Value>().await.unwrap(),
        json!({
            "status": "degraded",
            "provider": null,
            "providerConfiguration": {
                "valid": false,
                "error": "no_current_provider"
            }
        })
    );

    route_task.abort();
}

#[tokio::test]
async fn passes_sse_bytes_and_health_check() {
    let upstream = Router::new()
        .route("/v1/responses", post(stream_responses))
        .route("/v1/responses/compact", post(stream_responses));
    let (upstream_address, upstream_task) = spawn_router(upstream).await;
    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "streaming",
            &format!("http://{upstream_address}/v1"),
            None,
            Some("sk-stream-test"),
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let client = reqwest::Client::new();

    let health = client
        .get(format!("http://{route_address}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(
        health.json::<serde_json::Value>().await.unwrap(),
        json!({"status": "ok"})
    );

    let response = client
        .post(format!("http://{route_address}/v1/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    assert_eq!(
        response.text().await.unwrap(),
        "event: response.created\n\ndata: [DONE]\n\n"
    );

    let compact_response = client
        .post(format!("http://{route_address}/v1/responses/compact"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(compact_response.status(), StatusCode::OK);
    assert_eq!(
        compact_response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    assert_eq!(
        compact_response.text().await.unwrap(),
        "event: response.created\n\ndata: [DONE]\n\n"
    );

    route_task.abort();
    upstream_task.abort();
}

#[tokio::test]
async fn preserves_content_encoding_for_streaming_response() {
    let upstream = Router::new().route("/v1/responses", post(compressed_stream_response));
    let (upstream_address, upstream_task) = spawn_router(upstream).await;
    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "compressed",
            &format!("http://{upstream_address}/v1"),
            None,
            Some("sk-compressed-test"),
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "zstd");
    assert_eq!(
        response.bytes().await.unwrap(),
        Bytes::from_static(b"encoded-sse")
    );

    route_task.abort();
    upstream_task.abort();
}

#[tokio::test]
async fn maps_provider_and_upstream_errors() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(ProviderStore::open(directory.path().join("codex-route.db")).unwrap());
    let state = RouteState::new(store.clone(), None).unwrap();
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "provider_unavailable"
    );
    route_task.abort();

    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "chat",
            "https://api.example/v1",
            Some("chat"),
            Some("sk-test"),
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "responses_only"
    );
    route_task.abort();

    let directory = TempDir::new().unwrap();
    let state = route_state(
        &directory,
        provider(
            "missing-key",
            "https://api.example/v1",
            Some("responses"),
            None,
        ),
    );
    let (route_address, route_task) = spawn_router(build_router(state)).await;
    let response = reqwest::Client::new()
        .post(format!("http://{route_address}/v1/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"]["code"],
        "provider_configuration_error"
    );
    route_task.abort();
}
