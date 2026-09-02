use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use futures_util::TryStreamExt;
use serde_json::json;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::codex_provider::{
    extract_active_base_url, extract_codex_api_key, is_responses_wire_api,
};
use crate::provider::Provider;
use crate::provider_store::{ProviderStore, ProviderStoreError};

pub const DEFAULT_ROUTE_PORT: u16 = 16_729;

#[derive(Clone)]
pub struct RouteState {
    pub(crate) store: Arc<ProviderStore>,
    pub(crate) provider_id: Option<String>,
    pub(crate) client: reqwest::Client,
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
        })
    }

    pub fn validate_selection(&self) -> Result<(), RouteStartupError> {
        let provider = self.selected_provider_startup()?;
        provider_configuration_startup(&provider)?;
        Ok(())
    }

    fn selected_provider_startup(&self) -> Result<Provider, RouteStartupError> {
        if let Some(id) = self.provider_id.as_deref() {
            return self
                .store
                .get(id)?
                .ok_or_else(|| RouteStartupError::ProviderNotFound(id.to_string()));
        }

        self.store
            .list()?
            .into_iter()
            .find(|provider| provider.is_current)
            .ok_or(RouteStartupError::NoCurrentProvider)
    }

    fn selected_provider(&self) -> Result<Provider, RouteRequestError> {
        if let Some(id) = self.provider_id.as_deref() {
            return self
                .store
                .get(id)
                .map_err(|_| RouteRequestError::ProviderUnavailable)?
                .ok_or(RouteRequestError::ProviderUnavailable);
        }

        self.store
            .list()
            .map_err(|_| RouteRequestError::ProviderUnavailable)?
            .into_iter()
            .find(|provider| provider.is_current)
            .ok_or(RouteRequestError::ProviderUnavailable)
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
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
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

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, axum::Json(json!({"status": "ok"})))
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
    let provider = state.selected_provider()?;
    let (base_url, credential) = provider_configuration(&provider)?;
    let (parts, body) = request.into_parts();
    let url = upstream_endpoint_url(&base_url, endpoint, parts.uri.query())?;
    let headers = filter_request_headers(&parts.headers, Some(&credential))?;
    let body_stream = body
        .into_data_stream()
        .map_err(|error| std::io::Error::other(error.to_string()));
    let upstream = state
        .client
        .post(url)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body_stream))
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
