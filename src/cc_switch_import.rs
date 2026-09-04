use crate::codex_provider::{validate_codex_provider, CodexValidationError};
use crate::provider::{Provider, ProviderSource};
use clap::ValueEnum;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lower")]
pub enum ConflictPolicy {
    Skip,
    Replace,
    Rename,
}

#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub provider: Provider,
    pub source_is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RejectedProvider {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ImportScan {
    pub source: PathBuf,
    pub candidates: Vec<ImportCandidate>,
    pub rejected: Vec<RejectedProvider>,
}

impl ImportScan {
    pub fn select_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Self, CcSwitchImportError> {
        let requested: HashSet<&str> = provider_ids.iter().map(String::as_str).collect();
        let available: HashSet<&str> = self
            .candidates
            .iter()
            .filter_map(|candidate| candidate.provider.source.source_id())
            .collect();
        let mut unknown: Vec<String> = requested
            .difference(&available)
            .map(|id| (*id).to_string())
            .collect();
        unknown.sort();
        if !unknown.is_empty() {
            return Err(CcSwitchImportError::UnknownProviders(unknown));
        }

        Ok(Self {
            source: self.source.clone(),
            candidates: self
                .candidates
                .iter()
                .filter(|candidate| {
                    candidate
                        .provider
                        .source
                        .source_id()
                        .is_some_and(|id| requested.contains(id))
                })
                .cloned()
                .collect(),
            rejected: self.rejected.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    pub source: PathBuf,
    pub imported: usize,
    pub replaced: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub rejected: Vec<RejectedProvider>,
}

impl ImportReport {
    pub fn new(source: PathBuf) -> Self {
        Self {
            source,
            imported: 0,
            replaced: 0,
            renamed: 0,
            skipped: 0,
            rejected: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CcSwitchImportError {
    #[error("cannot open cc-switch database {path}: {source}")]
    Open {
        path: PathBuf,
        source: rusqlite::Error,
    },
    #[error("cc-switch database schema error: {0}")]
    Schema(String),
    #[error("cc-switch database query error: {0}")]
    Query(#[from] rusqlite::Error),
    #[error("selected cc-switch providers were not found: {}", .0.join(", "))]
    UnknownProviders(Vec<String>),
}

pub struct CcSwitchImporter {
    db_path: PathBuf,
}

impl CcSwitchImporter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            db_path: path.as_ref().to_path_buf(),
        }
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn discover_default_db() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cc-switch")
            .join("cc-switch.db")
    }

    pub fn read_codex_providers(&self) -> Result<ImportScan, CcSwitchImportError> {
        let connection =
            Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
                |source| CcSwitchImportError::Open {
                    path: self.db_path.clone(),
                    source,
                },
            )?;
        connection.busy_timeout(Duration::from_secs(2))?;
        let columns = provider_columns(&connection)?;
        let sql = build_query(&columns);
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(["codex"], |row| {
            Ok(SourceRow {
                id: row.get(0)?,
                name: row.get(1)?,
                settings_config: row.get(2)?,
                website_url: row.get(3)?,
                category: row.get(4)?,
                created_at: row.get(5)?,
                sort_index: row.get(6)?,
                notes: row.get(7)?,
                icon: row.get(8)?,
                icon_color: row.get(9)?,
                meta: row.get(10)?,
                in_failover_queue: row.get::<_, Option<bool>>(11)?.unwrap_or(false),
                is_current: row.get::<_, Option<bool>>(12)?.unwrap_or(false),
            })
        })?;

        let mut scan = ImportScan {
            source: self.db_path.clone(),
            candidates: Vec::new(),
            rejected: Vec::new(),
        };
        for row in rows {
            let row = row?;
            match normalize_row(row) {
                Ok(candidate) => scan.candidates.push(candidate),
                Err(rejection) => scan.rejected.push(rejection),
            }
        }
        Ok(scan)
    }
}

#[derive(Debug)]
struct SourceRow {
    id: String,
    name: String,
    settings_config: String,
    website_url: Option<String>,
    category: Option<String>,
    created_at: Option<i64>,
    sort_index: Option<i64>,
    notes: Option<String>,
    icon: Option<String>,
    icon_color: Option<String>,
    meta: String,
    in_failover_queue: bool,
    is_current: bool,
}

fn normalize_row(row: SourceRow) -> Result<ImportCandidate, RejectedProvider> {
    if row.id.trim().is_empty() {
        return Err(RejectedProvider {
            id: "<empty>".to_string(),
            reason: "provider id is empty".to_string(),
        });
    }
    let settings_config: Value =
        serde_json::from_str(&row.settings_config).map_err(|_| RejectedProvider {
            id: row.id.clone(),
            reason: "invalid settings_config JSON".to_string(),
        })?;
    if !settings_config.is_object() {
        return Err(RejectedProvider {
            id: row.id,
            reason: "settings_config must be a JSON object".to_string(),
        });
    }
    let meta: Value = serde_json::from_str(&row.meta).map_err(|_| RejectedProvider {
        id: row.id.clone(),
        reason: "invalid provider meta JSON".to_string(),
    })?;
    if !meta.is_object() {
        return Err(RejectedProvider {
            id: row.id,
            reason: "provider meta must be a JSON object".to_string(),
        });
    }
    validate_codex_provider(&settings_config).map_err(|error| RejectedProvider {
        id: row.id.clone(),
        reason: validation_reason(error),
    })?;
    Ok(ImportCandidate {
        provider: Provider {
            id: row.id.clone(),
            name: row.name,
            settings_config,
            website_url: row.website_url,
            category: row.category,
            created_at: row.created_at,
            sort_index: row.sort_index,
            notes: row.notes,
            icon: row.icon,
            icon_color: row.icon_color,
            meta,
            in_failover_queue: row.in_failover_queue,
            is_current: row.is_current,
            source: ProviderSource::CcSwitch {
                source_id: row.id,
                source_updated_at: row.created_at,
            },
        },
        source_is_current: row.is_current,
    })
}

fn validation_reason(error: CodexValidationError) -> String {
    error.to_string()
}

fn provider_columns(connection: &Connection) -> Result<HashSet<String>, CcSwitchImportError> {
    let mut statement = connection.prepare("PRAGMA table_info(providers)")?;
    let names = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for name in names {
        columns.insert(name?);
    }
    if columns.is_empty() {
        return Err(CcSwitchImportError::Schema(
            "providers table is missing".to_string(),
        ));
    }
    for required in ["id", "app_type", "name", "settings_config", "meta"] {
        if !columns.contains(required) {
            return Err(CcSwitchImportError::Schema(format!(
                "providers table is missing required column '{required}'"
            )));
        }
    }
    Ok(columns)
}

fn build_query(columns: &HashSet<String>) -> String {
    fn optional(columns: &HashSet<String>, name: &str) -> String {
        if columns.contains(name) {
            name.to_string()
        } else {
            format!("NULL AS {name}")
        }
    }
    fn optional_expression(columns: &HashSet<String>, name: &str) -> String {
        if columns.contains(name) {
            name.to_string()
        } else {
            "NULL".to_string()
        }
    }
    format!(
        "SELECT id, name, settings_config, {}, {}, {}, {}, {}, {}, {}, meta, {}, {} \
         FROM providers WHERE app_type = ?1 \
         ORDER BY COALESCE({}, 999999), COALESCE({}, 9999999999999), id",
        optional(columns, "website_url"),
        optional(columns, "category"),
        optional(columns, "created_at"),
        optional(columns, "sort_index"),
        optional(columns, "notes"),
        optional(columns, "icon"),
        optional(columns, "icon_color"),
        optional(columns, "in_failover_queue"),
        optional(columns, "is_current"),
        optional_expression(columns, "sort_index"),
        optional_expression(columns, "created_at"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_policy_uses_lowercase_wire_values() {
        assert_eq!(
            serde_json::to_string(&ConflictPolicy::Skip).unwrap(),
            "\"skip\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictPolicy::Replace).unwrap(),
            "\"replace\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictPolicy::Rename).unwrap(),
            "\"rename\""
        );
    }

    #[test]
    fn query_contains_codex_filter_and_optional_columns() {
        let columns = ["id", "app_type", "name", "settings_config", "meta"]
            .into_iter()
            .map(String::from)
            .collect();
        let query = build_query(&columns);
        assert!(query.contains("WHERE app_type = ?1"));
        assert!(query.contains("NULL AS website_url"));
    }
}
