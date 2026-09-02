use serde_json::Value;
use thiserror::Error;

const PROXY_MANAGED: &str = "PROXY_MANAGED";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexValidation {
    pub active_model_provider: Option<String>,
    pub base_url: Option<String>,
    pub has_api_key: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodexValidationError {
    #[error("settings_config must be a JSON object")]
    SettingsNotObject,
    #[error("config must be a string")]
    ConfigNotString,
    #[error("invalid Codex config.toml")]
    InvalidConfigToml,
    #[error("model_provider must be a string")]
    ModelProviderNotString,
    #[error("model_providers must be a table")]
    ModelProvidersNotTable,
    #[error("active model provider must be a table")]
    ActiveModelProviderNotTable,
    #[error("PROXY_MANAGED is not an importable credential")]
    ProxyManagedCredential,
    #[error("active route is a cc-switch proxy placeholder")]
    ProxyPlaceholderRoute,
}

pub fn extract_active_base_url(config_text: &str) -> Option<String> {
    let document = config_text.parse::<toml::Value>().ok()?;
    let active = document.get("model_provider").and_then(toml::Value::as_str);
    if let Some(active) = active {
        if let Some(url) = document
            .get("model_providers")
            .and_then(|providers| providers.get(active))
            .and_then(|provider| provider.get("base_url"))
            .and_then(toml::Value::as_str)
        {
            return non_empty(url);
        }
    }
    document
        .get("base_url")
        .and_then(toml::Value::as_str)
        .and_then(non_empty)
}

/// Returns the wire protocol configured for the active Codex model provider.
///
/// Codex provider tables are authoritative when `model_provider` selects one;
/// the top-level value is retained as a compatibility fallback for older
/// configurations.
pub fn extract_active_wire_api(config_text: &str) -> Option<String> {
    let document = config_text.parse::<toml::Value>().ok()?;
    let active_id = document.get("model_provider").and_then(toml::Value::as_str);
    if let Some(active_id) = active_id {
        if let Some(value) = document
            .get("model_providers")
            .and_then(|providers| providers.get(active_id))
            .and_then(|provider| provider.get("wire_api"))
            .and_then(toml::Value::as_str)
        {
            return non_empty(value).map(|value| value.to_ascii_lowercase());
        }
    }
    document
        .get("wire_api")
        .and_then(toml::Value::as_str)
        .and_then(non_empty)
        .map(|value| value.to_ascii_lowercase())
}

/// Returns whether a Codex config uses the native Responses protocol.
/// Missing `wire_api` is the native Responses default.
pub fn is_responses_wire_api(config_text: &str) -> bool {
    extract_active_wire_api(config_text).is_none_or(|wire_api| wire_api == "responses")
}

pub fn extract_codex_api_key(settings: &Value, config_text: Option<&str>) -> Option<String> {
    let auth = settings.get("auth").unwrap_or(settings);
    let from_auth = auth
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .and_then(non_empty);
    from_auth.or_else(|| config_text.and_then(extract_active_experimental_bearer_token))
}

pub fn validate_codex_provider(settings: &Value) -> Result<CodexValidation, CodexValidationError> {
    let object = settings
        .as_object()
        .ok_or(CodexValidationError::SettingsNotObject)?;
    let config_text = match object.get("config") {
        None | Some(Value::Null) => None,
        Some(Value::String(config)) if config.trim().is_empty() => None,
        Some(Value::String(config)) => Some(config.as_str()),
        Some(_) => return Err(CodexValidationError::ConfigNotString),
    };

    let document = config_text
        .map(|text| {
            text.parse::<toml::Value>()
                .map_err(|_| CodexValidationError::InvalidConfigToml)
        })
        .transpose()?;

    let active_model_provider = document
        .as_ref()
        .and_then(|doc| doc.get("model_provider"))
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or(CodexValidationError::ModelProviderNotString)
        })
        .transpose()?;

    let active_table = if let Some(document) = document.as_ref() {
        if let Some(providers) = document.get("model_providers") {
            let providers = providers
                .as_table()
                .ok_or(CodexValidationError::ModelProvidersNotTable)?;
            if let Some(active) = active_model_provider.as_deref() {
                if let Some(provider) = providers.get(active) {
                    Some(
                        provider
                            .as_table()
                            .ok_or(CodexValidationError::ActiveModelProviderNotTable)?,
                    )
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let base_url = config_text.and_then(extract_active_base_url);
    let api_key = extract_codex_api_key(settings, config_text);
    if api_key.as_deref().is_some_and(|key| key == PROXY_MANAGED) {
        return Err(CodexValidationError::ProxyManagedCredential);
    }

    let active_is_cc_switch_proxy = active_model_provider
        .as_deref()
        .is_some_and(|id| id == "cc-switch-official" || id.starts_with("cc-switch-"));
    let url_is_cc_switch_proxy = base_url.as_deref().is_some_and(is_cc_switch_proxy_url);
    if active_is_cc_switch_proxy || url_is_cc_switch_proxy {
        return Err(CodexValidationError::ProxyPlaceholderRoute);
    }

    let has_api_key = api_key.is_some()
        || active_table
            .and_then(|table| table.get("env_key"))
            .and_then(toml::Value::as_str)
            .is_some_and(|key| !key.trim().is_empty());

    Ok(CodexValidation {
        active_model_provider,
        base_url,
        has_api_key,
    })
}

fn extract_active_experimental_bearer_token(config_text: &str) -> Option<String> {
    let document = config_text.parse::<toml::Value>().ok()?;
    let active = document.get("model_provider").and_then(toml::Value::as_str);
    if let Some(active) = active {
        if let Some(token) = document
            .get("model_providers")
            .and_then(|providers| providers.get(active))
            .and_then(|provider| provider.get("experimental_bearer_token"))
            .and_then(toml::Value::as_str)
            .and_then(non_empty)
        {
            return Some(token);
        }
    }
    document
        .get("experimental_bearer_token")
        .and_then(toml::Value::as_str)
        .and_then(non_empty)
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn is_cc_switch_proxy_url(url: &str) -> bool {
    let normalized = url.trim().to_ascii_lowercase();
    normalized.contains("127.0.0.1:15721")
        || normalized.contains("localhost:15721")
        || normalized.contains("/cc-switch-proxy")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn active_provider_wins_over_inactive_provider() {
        let config = r#"
model_provider = "active"
[model_providers.active]
base_url = "https://active.example/v1"
[model_providers.inactive]
base_url = "https://inactive.example/v1"
"#;
        assert_eq!(
            extract_active_base_url(config).as_deref(),
            Some("https://active.example/v1")
        );
    }

    #[test]
    fn rejects_proxy_placeholder_but_accepts_loopback_with_key() {
        let placeholder = json!({
            "auth": {"OPENAI_API_KEY": "PROXY_MANAGED"},
            "config": "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"https://example.test/v1\""
        });
        assert!(matches!(
            validate_codex_provider(&placeholder),
            Err(CodexValidationError::ProxyManagedCredential)
        ));

        let local = json!({
            "auth": {"OPENAI_API_KEY": "sk-local"},
            "config": "base_url = \"http://127.0.0.1:9000/v1\""
        });
        assert!(validate_codex_provider(&local).is_ok());
    }
}
