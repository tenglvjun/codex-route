use axum::body::{to_bytes, Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use codex_route::provider::{Provider, ProviderSource};
use codex_route::provider_store::ProviderStore;
use codex_route::route::{
    build_router, filter_request_headers, upstream_responses_url, RouteStartupError, RouteState,
};
use futures_util::stream;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;

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
async fn passes_sse_bytes_and_health_check() {
    let upstream = Router::new().route("/v1/responses", post(stream_responses));
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
