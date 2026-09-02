use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A provider configuration owned by codex-route.
///
/// `settings_config` and `meta` intentionally remain opaque JSON values. Codex
/// and cc-switch add fields over time, so the store must not discard unknown
/// nested configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "settingsConfig")]
    pub settings_config: Value,
    #[serde(skip_serializing_if = "Option::is_none", rename = "websiteUrl")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "createdAt")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sortIndex")]
    pub sort_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "iconColor")]
    pub icon_color: Option<String>,
    #[serde(default)]
    pub meta: Value,
    #[serde(default, rename = "inFailoverQueue")]
    pub in_failover_queue: bool,
    #[serde(default)]
    pub is_current: bool,
    pub source: ProviderSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProviderSource {
    Local,
    CcSwitch {
        #[serde(rename = "sourceId")]
        source_id: String,
        #[serde(default, rename = "sourceUpdatedAt")]
        source_updated_at: Option<i64>,
    },
}

impl ProviderSource {
    pub fn source_name(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::CcSwitch { .. } => "cc-switch",
        }
    }

    pub fn source_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::CcSwitch { source_id, .. } => Some(source_id),
        }
    }

    pub fn source_updated_at(&self) -> Option<i64> {
        match self {
            Self::Local => None,
            Self::CcSwitch {
                source_updated_at, ..
            } => *source_updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub source: String,
    #[serde(rename = "isCurrent")]
    pub is_current: bool,
}

impl From<&Provider> for ProviderSummary {
    fn from(provider: &Provider) -> Self {
        Self {
            id: provider.id.clone(),
            name: provider.name.clone(),
            category: provider.category.clone(),
            source: provider.source.source_name().to_string(),
            is_current: provider.is_current,
        }
    }
}
