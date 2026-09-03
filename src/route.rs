use axum::body::{to_bytes, Body};
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, State};
use axum::http::{header, HeaderMap, HeaderValue, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::Router;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::codex_provider::{
    extract_active_base_url, extract_codex_api_key, is_responses_wire_api,
};
use crate::provider::{Provider, ProviderSummary};
use crate::provider_store::{ProviderStore, ProviderStoreError};
use crate::workspace_rule::normalize_workspace_path;
use crate::{config::ScanConfig, index::SessionWorkspaceIndex};

pub const DEFAULT_ROUTE_PORT: u16 = 16_729;

#[derive(Clone)]
pub struct RouteState {
    pub(crate) store: Arc<ProviderStore>,
    pub(crate) provider_id: Option<String>,
    pub(crate) client: reqwest::Client,
    scan_config: Option<ScanConfig>,
}

impl RouteState {
    pub fn new(
        store: Arc<ProviderStore>,
        provider_id: Option<String>,
    ) -> Result<Self, RouteStartupError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            store,
            provider_id,
            client,
            scan_config: None,
        })
    }

    pub fn with_scan_config(
        store: Arc<ProviderStore>,
        provider_id: Option<String>,
        scan_config: ScanConfig,
    ) -> Result<Self, RouteStartupError> {
        let mut state = Self::new(store, provider_id)?;
        state.scan_config = Some(scan_config);
        Ok(state)
    }

    pub fn validate_selection(&self) -> Result<(), RouteStartupError> {
        let provider = self.selected_provider_startup()?;
        provider_configuration_startup(&provider)?;
        Ok(())
    }

    fn configured_provider(&self) -> Result<Option<Provider>, ProviderStoreError> {
        match self.provider_id.as_deref() {
            Some(provider_id) => self.store.get(provider_id),
            None => self
                .store
                .list()
                .map(|providers| providers.into_iter().find(|provider| provider.is_current)),
        }
    }

    fn selected_provider_startup(&self) -> Result<Provider, RouteStartupError> {
        self.configured_provider()?
            .ok_or_else(|| match self.provider_id.as_deref() {
                Some(id) => RouteStartupError::ProviderNotFound(id.to_string()),
                None => RouteStartupError::NoCurrentProvider,
            })
    }

    fn selected_provider(
        &self,
        headers: &HeaderMap,
        body: Option<&Value>,
    ) -> Result<Provider, RouteRequestError> {
        if self.provider_id.is_some() {
            return self
                .configured_provider()
                .map_err(|_| RouteRequestError::ProviderUnavailable)?
                .ok_or(RouteRequestError::ProviderUnavailable);
        }

        if let Some(workspace) = self.resolve_request_workspace(headers, body) {
            if let Some(rule) = self
                .store
                .get_route_rule(&workspace)
                .map_err(|_| RouteRequestError::ProviderUnavailable)?
            {
                if let Some(provider) = self
                    .store
                    .get(&rule.provider_id)
                    .map_err(|_| RouteRequestError::ProviderUnavailable)?
                {
                    return Ok(provider);
                }
            }
        }

        self.configured_provider()
            .map_err(|_| RouteRequestError::ProviderUnavailable)?
            .ok_or(RouteRequestError::ProviderUnavailable)
    }

    fn resolve_request_workspace(
        &self,
        headers: &HeaderMap,
        body: Option<&Value>,
    ) -> Option<PathBuf> {
        let session_id = extract_codex_session_id(headers, body?)?;
        let config = self.scan_config.as_ref()?;
        let index = SessionWorkspaceIndex::build(config).ok()?;
        let lookup = index.resolve(&session_id).ok()?;
        if lookup.conflicting_workspaces || !lookup.workspace_exists {
            return None;
        }
        normalize_workspace_path(&lookup.workspace).ok()
    }
}

#[derive(Debug, Error)]
pub enum RouteStartupError {
    #[error("provider '{0}' was not found")]
    ProviderNotFound(String),
    #[error("no current provider is configured")]
    NoCurrentProvider,
    #[error("provider '{0}' has no usable Responses upstream URL")]
    MissingBaseUrl(String),
    #[error("provider '{0}' has no usable upstream credential")]
    MissingCredential(String),
    #[error("provider '{0}' does not use the Responses protocol")]
    UnsupportedWireApi(String),
    #[error("invalid provider '{0}' upstream URL")]
    InvalidBaseUrl(String),
    #[error("provider '{0}' has invalid configuration")]
    InvalidConfiguration(String),
    #[error("provider store error: {0}")]
    Store(#[from] ProviderStoreError),
    #[error("HTTP client initialization failed: {0}")]
    HttpClient(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum RouteRequestError {
    #[error("provider is unavailable")]
    ProviderUnavailable,
    #[error("provider configuration is invalid")]
    ProviderConfiguration,
    #[error("provider does not use the Responses protocol")]
    UnsupportedWireApi,
    #[error("invalid upstream URL")]
    InvalidUrl,
    #[error("invalid request header")]
    InvalidHeader,
    #[error("request body is too large or unreadable")]
    RequestBody,
    #[error("upstream request failed")]
    Upstream,
}

#[derive(Debug, Error)]
enum ProviderConfigError {
    #[error("provider configuration is invalid")]
    Invalid,
    #[error("provider does not use the Responses protocol")]
    UnsupportedWireApi,
    #[error("provider has no usable upstream URL")]
    MissingBaseUrl,
    #[error("provider has no usable credential")]
    MissingCredential,
    #[error("provider has an invalid upstream URL")]
    InvalidBaseUrl,
}

pub fn build_router(state: RouteState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/providers", get(api_providers))
        .route("/api/providers/current", put(api_set_current_provider))
        .route(
            "/api/route-rules",
            get(api_route_rules)
                .put(api_upsert_route_rule)
                .delete(api_remove_route_rule),
        )
        .route("/api/status", get(api_status))
        .route("/models", get(models))
        .route("/v1/models", get(models))
        .route("/responses/compact", post(responses_compact))
        .route("/v1/responses", post(responses))
        .route("/v1/responses/compact", post(responses_compact))
        .with_state(state)
}

pub async fn serve(state: RouteState, port: u16) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let ctrl_c = tokio::signal::ctrl_c();
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler should be installable");
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

pub fn upstream_responses_url(
    base_url: &str,
    query: Option<&str>,
) -> Result<reqwest::Url, RouteRequestError> {
    upstream_endpoint_url(base_url, "/responses", query)
}

fn upstream_endpoint_url(
    base_url: &str,
    endpoint: &str,
    query: Option<&str>,
) -> Result<reqwest::Url, RouteRequestError> {
    let mut url = reqwest::Url::parse(base_url).map_err(|_| RouteRequestError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(RouteRequestError::InvalidUrl);
    }

    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        endpoint.to_string()
    } else {
        format!("{path}{endpoint}")
    };
    url.set_path(&path);
    url.set_query(query);
    Ok(url)
}

pub fn filter_request_headers(
    input: &HeaderMap,
    credential: Option<&str>,
) -> Result<HeaderMap, RouteRequestError> {
    let mut output = HeaderMap::new();
    for (name, value) in input {
        if is_request_hop_by_hop(name) || *name == header::AUTHORIZATION {
            continue;
        }
        output.append(name.clone(), value.clone());
    }
    if let Some(credential) = credential.filter(|value| !value.trim().is_empty()) {
        let value = HeaderValue::from_str(&format!("Bearer {credential}"))
            .map_err(|_| RouteRequestError::InvalidHeader)?;
        output.insert(header::AUTHORIZATION, value);
    }
    Ok(output)
}

/// Extract the stable session identity fields used by Codex Responses clients.
/// `previous_response_id` is a response-chain cursor, not a session ID.
pub fn extract_codex_session_id(headers: &HeaderMap, body: &Value) -> Option<String> {
    for header_name in ["session_id", "x-session-id"] {
        if let Some(value) = headers
            .get(header_name)
            .and_then(|value| value.to_str().ok())
        {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    body.get("metadata")
        .and_then(|metadata| metadata.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, axum::Json(json!({"status": "ok"})))
}

async fn api_providers(State(state): State<RouteState>) -> Response<Body> {
    match state.store.list() {
        Ok(providers) => {
            let summaries: Vec<ProviderSummary> =
                providers.iter().map(ProviderSummary::from).collect();
            (StatusCode::OK, Json(summaries)).into_response()
        }
        Err(error) => management_store_error(error),
    }
}

#[derive(Debug, Deserialize)]
struct CurrentProviderRequest {
    #[serde(rename = "providerId", alias = "provider_id")]
    provider_id: String,
}

#[derive(Debug, Deserialize)]
struct RouteRuleRequest {
    workspace: PathBuf,
    #[serde(rename = "providerId", alias = "provider_id")]
    provider_id: String,
    #[serde(default)]
    replace: bool,
}

#[derive(Debug, Deserialize)]
struct WorkspaceRequest {
    workspace: PathBuf,
}

#[derive(Debug, Serialize)]
struct RouteStatus {
    status: &'static str,
    provider: Option<ProviderSummary>,
    #[serde(rename = "providerConfiguration")]
    provider_configuration: ProviderConfigurationStatus,
}

#[derive(Debug, Serialize)]
struct ProviderConfigurationStatus {
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

async fn api_set_current_provider(
    State(state): State<RouteState>,
    payload: Result<Json<CurrentProviderRequest>, JsonRejection>,
) -> Response<Body> {
    let payload = match parse_management_json(payload) {
        Ok(payload) => payload,
        Err(()) => return invalid_management_json(),
    };
    let provider_id = payload.provider_id.trim();
    if provider_id.is_empty() {
        return management_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "providerId must not be empty",
        );
    }
    match state.store.set_current(provider_id) {
        Ok(provider) => (StatusCode::OK, Json(ProviderSummary::from(&provider))).into_response(),
        Err(error) => management_store_error(error),
    }
}

async fn api_route_rules(State(state): State<RouteState>) -> Response<Body> {
    match state.store.list_route_rules() {
        Ok(rules) => (StatusCode::OK, Json(rules)).into_response(),
        Err(error) => management_store_error(error),
    }
}

async fn api_upsert_route_rule(
    State(state): State<RouteState>,
    payload: Result<Json<RouteRuleRequest>, JsonRejection>,
) -> Response<Body> {
    let payload = match parse_management_json(payload) {
        Ok(payload) => payload,
        Err(()) => return invalid_management_json(),
    };
    let provider_id = payload.provider_id.trim();
    if provider_id.is_empty() {
        return management_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "providerId must not be empty",
        );
    }
    let outcome =
        match state
            .store
            .upsert_route_rule(&payload.workspace, provider_id, payload.replace)
        {
            Ok(outcome) => outcome,
            Err(error) => return management_store_error(error),
        };
    let rule = match state.store.get_route_rule(&payload.workspace) {
        Ok(Some(rule)) => rule,
        Ok(None) => {
            return management_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
                "route rule was not available after update",
            )
        }
        Err(error) => return management_store_error(error),
    };
    let action = match outcome {
        crate::provider_store::UpsertRouteRuleOutcome::Inserted => "inserted",
        crate::provider_store::UpsertRouteRuleOutcome::Replaced => "replaced",
    };
    (
        StatusCode::OK,
        Json(json!({"action": action, "rule": rule})),
    )
        .into_response()
}

async fn api_remove_route_rule(
    State(state): State<RouteState>,
    payload: Result<Json<WorkspaceRequest>, JsonRejection>,
) -> Response<Body> {
    let payload = match parse_management_json(payload) {
        Ok(payload) => payload,
        Err(()) => return invalid_management_json(),
    };
    match state.store.remove_route_rule(&payload.workspace) {
        Ok(rule) => (StatusCode::OK, Json(rule)).into_response(),
        Err(error) => management_store_error(error),
    }
}

async fn api_status(State(state): State<RouteState>) -> Response<Body> {
    let provider = match state.configured_provider() {
        Ok(provider) => provider,
        Err(error) => return management_store_error(error),
    };
    let provider_configuration = match provider.as_ref() {
        Some(provider) => match provider_configuration_inner(provider) {
            Ok(_) => ProviderConfigurationStatus {
                valid: true,
                error: None,
            },
            Err(error) => ProviderConfigurationStatus {
                valid: false,
                error: Some(provider_configuration_error_code(&error)),
            },
        },
        None => ProviderConfigurationStatus {
            valid: false,
            error: Some(if state.provider_id.is_some() {
                "provider_not_found"
            } else {
                "no_current_provider"
            }),
        },
    };
    let status = if provider_configuration.valid {
        "ok"
    } else {
        "degraded"
    };
    let provider = provider.as_ref().map(ProviderSummary::from);
    (
        StatusCode::OK,
        Json(RouteStatus {
            status,
            provider,
            provider_configuration,
        }),
    )
        .into_response()
}

async fn models() -> impl IntoResponse {
    // Codex probes this endpoint during startup and expects a top-level models
    // catalog. The MVP does not own a Codex model catalog yet, so return the
    // same empty catalog shape used by cc-switch when no catalog is active.
    (StatusCode::OK, axum::Json(json!({"models": []})))
}

async fn responses(State(state): State<RouteState>, request: Request<Body>) -> Response<Body> {
    match forward_endpoint(state, request, "/responses").await {
        Ok(response) => response,
        Err(error) => route_error_response(error),
    }
}

async fn responses_compact(
    State(state): State<RouteState>,
    request: Request<Body>,
) -> Response<Body> {
    match forward_endpoint(state, request, "/responses/compact").await {
        Ok(response) => response,
        Err(error) => route_error_response(error),
    }
}

async fn forward_endpoint(
    state: RouteState,
    request: Request<Body>,
    endpoint: &str,
) -> Result<Response<Body>, RouteRequestError> {
    let (parts, body) = request.into_parts();
    let (provider, request_body) = if state.provider_id.is_some() {
        let provider = state.selected_provider(&parts.headers, None)?;
        let body_stream = body
            .into_data_stream()
            .map_err(|error| std::io::Error::other(error.to_string()));
        (provider, reqwest::Body::wrap_stream(body_stream))
    } else {
        let body = to_bytes(body, 64 * 1024 * 1024)
            .await
            .map_err(|_| RouteRequestError::RequestBody)?;
        let body_json = serde_json::from_slice::<Value>(&body).ok();
        let provider = state.selected_provider(&parts.headers, body_json.as_ref())?;
        (provider, reqwest::Body::from(body))
    };
    let (base_url, credential) = provider_configuration(&provider)?;
    let url = upstream_endpoint_url(&base_url, endpoint, parts.uri.query())?;
    let headers = filter_request_headers(&parts.headers, Some(&credential))?;
    let upstream = state
        .client
        .post(url)
        .headers(headers)
        .body(request_body)
        .send()
        .await
        .map_err(|_| RouteRequestError::Upstream)?;

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let mut response = Response::builder()
        .status(status)
        .body(Body::from_stream(
            upstream
                .bytes_stream()
                .map_err(|error| std::io::Error::other(error.to_string())),
        ))
        .map_err(|_| RouteRequestError::Upstream)?;
    for (name, value) in &upstream_headers {
        if is_response_hop_by_hop(name) {
            continue;
        }
        response.headers_mut().append(name.clone(), value.clone());
    }
    Ok(response)
}

fn provider_configuration(provider: &Provider) -> Result<(String, String), RouteRequestError> {
    provider_configuration_inner(provider).map_err(|error| match error {
        ProviderConfigError::UnsupportedWireApi => RouteRequestError::UnsupportedWireApi,
        ProviderConfigError::InvalidBaseUrl => RouteRequestError::InvalidUrl,
        ProviderConfigError::Invalid
        | ProviderConfigError::MissingBaseUrl
        | ProviderConfigError::MissingCredential => RouteRequestError::ProviderConfiguration,
    })
}

fn provider_configuration_startup(
    provider: &Provider,
) -> Result<(String, String), RouteStartupError> {
    provider_configuration_inner(provider).map_err(|error| match error {
        ProviderConfigError::UnsupportedWireApi => {
            RouteStartupError::UnsupportedWireApi(provider.id.clone())
        }
        ProviderConfigError::MissingBaseUrl => {
            RouteStartupError::MissingBaseUrl(provider.id.clone())
        }
        ProviderConfigError::MissingCredential => {
            RouteStartupError::MissingCredential(provider.id.clone())
        }
        ProviderConfigError::InvalidBaseUrl => {
            RouteStartupError::InvalidBaseUrl(provider.id.clone())
        }
        ProviderConfigError::Invalid => {
            RouteStartupError::InvalidConfiguration(provider.id.clone())
        }
    })
}

fn provider_configuration_inner(
    provider: &Provider,
) -> Result<(String, String), ProviderConfigError> {
    let settings = provider
        .settings_config
        .as_object()
        .ok_or(ProviderConfigError::Invalid)?;
    let config = settings
        .get("config")
        .and_then(serde_json::Value::as_str)
        .filter(|config| !config.trim().is_empty())
        .ok_or(ProviderConfigError::Invalid)?;
    config
        .parse::<toml::Value>()
        .map_err(|_| ProviderConfigError::Invalid)?;
    if !is_responses_wire_api(config) {
        return Err(ProviderConfigError::UnsupportedWireApi);
    }
    let base_url = extract_active_base_url(config).ok_or(ProviderConfigError::MissingBaseUrl)?;
    let credential = extract_codex_api_key(&provider.settings_config, Some(config))
        .filter(|credential| credential != "PROXY_MANAGED")
        .ok_or(ProviderConfigError::MissingCredential)?;
    if is_proxy_placeholder(&base_url) {
        return Err(ProviderConfigError::MissingBaseUrl);
    }
    upstream_responses_url(&base_url, None).map_err(|_| ProviderConfigError::InvalidBaseUrl)?;
    Ok((base_url, credential))
}

fn is_proxy_placeholder(base_url: &str) -> bool {
    let normalized = base_url.trim().to_ascii_lowercase();
    normalized.contains("127.0.0.1:15721")
        || normalized.contains("localhost:15721")
        || normalized.contains("/cc-switch-proxy")
}

fn route_error_response(error: RouteRequestError) -> Response<Body> {
    let (status, code, message) = match error {
        RouteRequestError::ProviderUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_unavailable",
            "provider is unavailable",
        ),
        RouteRequestError::ProviderConfiguration
        | RouteRequestError::InvalidUrl
        | RouteRequestError::InvalidHeader => (
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_configuration_error",
            "provider configuration is invalid",
        ),
        RouteRequestError::UnsupportedWireApi => (
            StatusCode::NOT_IMPLEMENTED,
            "responses_only",
            "only the Responses protocol is supported",
        ),
        RouteRequestError::RequestBody => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_body_error",
            "request body is too large or unreadable",
        ),
        RouteRequestError::Upstream => (
            StatusCode::BAD_GATEWAY,
            "upstream_unavailable",
            "upstream request failed",
        ),
    };
    (
        status,
        axum::Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

fn management_store_error(error: ProviderStoreError) -> Response<Body> {
    match error {
        ProviderStoreError::ProviderNotFound(id) => management_error(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            format!("provider '{id}' was not found"),
        ),
        ProviderStoreError::RouteRuleAlreadyExists(workspace) => management_error(
            StatusCode::CONFLICT,
            "route_rule_exists",
            format!("workspace route already exists: {}", workspace.display()),
        ),
        ProviderStoreError::RouteRuleNotFound(workspace) => management_error(
            StatusCode::NOT_FOUND,
            "route_rule_not_found",
            format!("workspace route was not found: {}", workspace.display()),
        ),
        ProviderStoreError::InvalidWorkspace(error) => management_error(
            StatusCode::BAD_REQUEST,
            "invalid_workspace",
            error.to_string(),
        ),
        _ => management_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_error",
            "provider store operation failed",
        ),
    }
}

fn parse_management_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ()> {
    payload.map(|Json(payload)| payload).map_err(|_| ())
}

fn invalid_management_json() -> Response<Body> {
    management_error(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "request body must be valid JSON",
    )
}

fn management_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response<Body> {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message.into(),
            }
        })),
    )
        .into_response()
}

fn provider_configuration_error_code(error: &ProviderConfigError) -> &'static str {
    match error {
        ProviderConfigError::Invalid => "invalid_configuration",
        ProviderConfigError::UnsupportedWireApi => "unsupported_wire_api",
        ProviderConfigError::MissingBaseUrl => "missing_base_url",
        ProviderConfigError::MissingCredential => "missing_credential",
        ProviderConfigError::InvalidBaseUrl => "invalid_base_url",
    }
}

fn is_request_hop_by_hop(name: &axum::http::HeaderName) -> bool {
    matches!(
        name.as_str(),
        "authorization"
            | "host"
            | "content-length"
            | "connection"
            | "transfer-encoding"
            | "te"
            | "trailer"
            | "upgrade"
            | "proxy-authenticate"
            | "proxy-authorization"
    )
}

fn is_response_hop_by_hop(name: &axum::http::HeaderName) -> bool {
    matches!(
        name.as_str(),
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "te"
            | "trailer"
            | "upgrade"
            | "proxy-authenticate"
            | "proxy-authorization"
    )
}
