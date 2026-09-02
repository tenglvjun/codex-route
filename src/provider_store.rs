use crate::provider::{Provider, ProviderSource};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Error)]
pub enum ProviderStoreError {
    #[error("provider database I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("provider database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("provider JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("provider database lock is poisoned")]
    LockPoisoned,
    #[error("provider '{0}' already exists")]
    AlreadyExists(String),
    #[error("provider '{0}' is not a cc-switch import")]
    NotImported(String),
    #[error("unsupported provider database schema version {0}")]
    UnsupportedSchemaVersion(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    Replaced,
    Skipped,
    Renamed,
}

pub struct ProviderStore {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl ProviderStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProviderStoreError> {
        let path = path.as_ref().to_path_buf();
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;

        let connection = Connection::open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        create_schema(&connection)?;

        Ok(Self {
            path,
            connection: Mutex::new(connection),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> Result<Vec<Provider>, ProviderStoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, settings_config, website_url, category,
                    created_at, sort_index, notes, icon, icon_color, meta,
                    in_failover_queue, is_current, source, source_id,
                    source_updated_at
             FROM providers
             ORDER BY COALESCE(sort_index, 999999), COALESCE(created_at, 9999999999999), id",
        )?;
        let rows = statement.query_map([], row_to_provider)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get(&self, id: &str) -> Result<Option<Provider>, ProviderStoreError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT id, name, settings_config, website_url, category,
                        created_at, sort_index, notes, icon, icon_color, meta,
                        in_failover_queue, is_current, source, source_id,
                        source_updated_at
                 FROM providers WHERE id = ?1",
                params![id],
                row_to_provider,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert(&self, provider: &Provider) -> Result<(), ProviderStoreError> {
        let connection = self.lock_connection()?;
        insert_provider(&connection, provider, now_unix_seconds())
    }

    pub fn upsert_imported(
        &self,
        provider: &Provider,
        replace: bool,
    ) -> Result<UpsertOutcome, ProviderStoreError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction()?;
        let source_id = provider
            .source
            .source_id()
            .ok_or_else(|| ProviderStoreError::NotImported(provider.id.clone()))?;
        if let Some(existing) = find_imported_in_transaction(&tx, source_id)? {
            if !replace {
                tx.commit()?;
                return Ok(UpsertOutcome::Skipped);
            }
            update_imported_in_transaction(&tx, &existing.id, provider)?;
            tx.commit()?;
            return Ok(UpsertOutcome::Replaced);
        }

        let mut provider = provider.clone();
        let renamed = provider_id_exists(&tx, &provider.id)?;
        if renamed {
            provider.id = available_ccswitch_id(&tx, source_id)?;
        }
        insert_provider(&tx, &provider, now_unix_seconds())?;
        tx.commit()?;
        Ok(if renamed {
            UpsertOutcome::Renamed
        } else {
            UpsertOutcome::Inserted
        })
    }

    pub fn has_current(&self) -> Result<bool, ProviderStoreError> {
        let connection = self.lock_connection()?;
        let current: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM providers WHERE is_current = 1 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(current.is_some())
    }

    pub fn find_imported(&self, source_id: &str) -> Result<Option<Provider>, ProviderStoreError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT id, name, settings_config, website_url, category,
                        created_at, sort_index, notes, icon, icon_color, meta,
                        in_failover_queue, is_current, source, source_id,
                        source_updated_at
                 FROM providers WHERE source = 'cc-switch' AND source_id = ?1",
                params![source_id],
                row_to_provider,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn import_transaction(
        &self,
        candidates: &[crate::cc_switch_import::ImportCandidate],
        policy: crate::cc_switch_import::ConflictPolicy,
    ) -> Result<crate::cc_switch_import::ImportReport, ProviderStoreError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction()?;
        let mut report = crate::cc_switch_import::ImportReport::new(PathBuf::new());
        let had_current = has_current_in_transaction(&tx)?;

        for candidate in candidates {
            let mut provider = candidate.provider.clone();
            let existing_source_id = provider.source.source_id().map(ToString::to_string);
            let existing_by_source = existing_source_id
                .as_deref()
                .map(|source_id| find_imported_in_transaction(&tx, source_id))
                .transpose()?
                .flatten();

            if let Some(existing) = existing_by_source {
                match policy {
                    crate::cc_switch_import::ConflictPolicy::Skip => {
                        report.skipped += 1;
                    }
                    crate::cc_switch_import::ConflictPolicy::Replace => {
                        update_imported_in_transaction(&tx, &existing.id, &provider)?;
                        report.replaced += 1;
                    }
                    crate::cc_switch_import::ConflictPolicy::Rename => {
                        // Source identity is authoritative once a row has been
                        // imported. Rename only applies to a first-import local
                        // ID collision; repeated imports update this row.
                        update_imported_in_transaction(&tx, &existing.id, &provider)?;
                        report.replaced += 1;
                    }
                }
                continue;
            }

            if provider_id_exists(&tx, &provider.id)? {
                match policy {
                    crate::cc_switch_import::ConflictPolicy::Skip => {
                        report.skipped += 1;
                        continue;
                    }
                    crate::cc_switch_import::ConflictPolicy::Replace
                    | crate::cc_switch_import::ConflictPolicy::Rename => {
                        provider.id = available_ccswitch_id(
                            &tx,
                            provider.source.source_id().unwrap_or(&provider.id),
                        )?;
                        report.renamed += 1;
                    }
                }
            }

            provider.is_current =
                !had_current && !has_current_in_transaction(&tx)? && candidate.source_is_current;
            insert_provider(&tx, &provider, now_unix_seconds())?;
            report.imported += 1;
        }

        tx.commit()?;
        Ok(report)
    }

    pub fn import_scan_transaction(
        &self,
        scan: &crate::cc_switch_import::ImportScan,
        policy: crate::cc_switch_import::ConflictPolicy,
    ) -> Result<crate::cc_switch_import::ImportReport, ProviderStoreError> {
        let mut report = self.import_transaction(&scan.candidates, policy)?;
        report.source = scan.source.clone();
        report.rejected = scan.rejected.clone();
        Ok(report)
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, ProviderStoreError> {
        self.connection
            .lock()
            .map_err(|_| ProviderStoreError::LockPoisoned)
    }
}

fn create_schema(connection: &Connection) -> Result<(), ProviderStoreError> {
    let version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(ProviderStoreError::UnsupportedSchemaVersion(version));
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
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
            in_failover_queue INTEGER NOT NULL DEFAULT 0,
            is_current INTEGER NOT NULL DEFAULT 0,
            source TEXT NOT NULL DEFAULT 'local',
            source_id TEXT,
            source_updated_at INTEGER,
            imported_at INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_source
            ON providers(source, source_id)
            WHERE source_id IS NOT NULL;
        ",
    )?;
    if version < SCHEMA_VERSION {
        connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

fn insert_provider(
    connection: &Connection,
    provider: &Provider,
    imported_at: i64,
) -> Result<(), ProviderStoreError> {
    let source_id = provider.source.source_id();
    let result = connection.execute(
        "INSERT INTO providers (
            id, name, settings_config, website_url, category, created_at,
            sort_index, notes, icon, icon_color, meta, in_failover_queue,
            is_current, source, source_id, source_updated_at, imported_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                  ?13, ?14, ?15, ?16, ?17)",
        params![
            provider.id,
            provider.name,
            serde_json::to_string(&provider.settings_config)?,
            provider.website_url,
            provider.category,
            provider.created_at,
            provider.sort_index,
            provider.notes,
            provider.icon,
            provider.icon_color,
            serde_json::to_string(&provider.meta)?,
            provider.in_failover_queue,
            provider.is_current,
            provider.source.source_name(),
            source_id,
            provider.source.source_updated_at(),
            imported_at,
        ],
    );
    result.map(|_| ()).map_err(|error| match error {
        rusqlite::Error::SqliteFailure(error, _)
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                || error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            ProviderStoreError::AlreadyExists(provider.id.clone())
        }
        other => other.into(),
    })
}

fn update_imported_in_transaction(
    tx: &Transaction<'_>,
    id: &str,
    provider: &Provider,
) -> Result<(), ProviderStoreError> {
    let existing = tx.query_row(
        "SELECT is_current, in_failover_queue FROM providers WHERE id = ?1",
        params![id],
        |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
    )?;
    tx.execute(
        "UPDATE providers SET name = ?1, settings_config = ?2, website_url = ?3,
            category = ?4, created_at = ?5, sort_index = ?6, notes = ?7,
            icon = ?8, icon_color = ?9, meta = ?10, source_updated_at = ?11,
            imported_at = ?12, is_current = ?13, in_failover_queue = ?14
         WHERE id = ?15",
        params![
            provider.name,
            serde_json::to_string(&provider.settings_config)?,
            provider.website_url,
            provider.category,
            provider.created_at,
            provider.sort_index,
            provider.notes,
            provider.icon,
            provider.icon_color,
            serde_json::to_string(&provider.meta)?,
            provider.source.source_updated_at(),
            now_unix_seconds(),
            existing.0,
            existing.1,
            id,
        ],
    )?;
    Ok(())
}

fn row_to_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<Provider> {
    let source: String = row.get(13)?;
    let source_id: Option<String> = row.get(14)?;
    let source_updated_at: Option<i64> = row.get(15)?;
    let source = match source.as_str() {
        "cc-switch" => ProviderSource::CcSwitch {
            source_id: source_id.unwrap_or_default(),
            source_updated_at,
        },
        _ => ProviderSource::Local,
    };
    let settings_config: String = row.get(2)?;
    let meta: String = row.get(10)?;
    Ok(Provider {
        id: row.get(0)?,
        name: row.get(1)?,
        settings_config: serde_json::from_str(&settings_config).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                "invalid settings_config JSON".into(),
            )
        })?,
        website_url: row.get(3)?,
        category: row.get(4)?,
        created_at: row.get(5)?,
        sort_index: row.get(6)?,
        notes: row.get(7)?,
        icon: row.get(8)?,
        icon_color: row.get(9)?,
        meta: serde_json::from_str(&meta).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                "invalid meta JSON".into(),
            )
        })?,
        in_failover_queue: row.get(11)?,
        is_current: row.get(12)?,
        source,
    })
}

fn provider_id_exists(tx: &Transaction<'_>, id: &str) -> Result<bool, ProviderStoreError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM providers WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}

fn find_imported_in_transaction(
    tx: &Transaction<'_>,
    source_id: &str,
) -> Result<Option<Provider>, ProviderStoreError> {
    tx.query_row(
        "SELECT id, name, settings_config, website_url, category,
                created_at, sort_index, notes, icon, icon_color, meta,
                in_failover_queue, is_current, source, source_id,
                source_updated_at
         FROM providers WHERE source = 'cc-switch' AND source_id = ?1",
        params![source_id],
        row_to_provider,
    )
    .optional()
    .map_err(Into::into)
}

fn available_ccswitch_id(
    tx: &Transaction<'_>,
    source_id: &str,
) -> Result<String, ProviderStoreError> {
    let base = format!("ccswitch-{source_id}");
    if !provider_id_exists(tx, &base)? {
        return Ok(base);
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if !provider_id_exists(tx, &candidate)? {
            return Ok(candidate);
        }
    }
    unreachable!("integer suffix range is exhausted")
}

fn has_current_in_transaction(tx: &Transaction<'_>) -> Result<bool, ProviderStoreError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM providers WHERE is_current = 1 LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderSource;
    use serde_json::json;
    use tempfile::tempdir;

    fn provider(id: &str, source: ProviderSource) -> Provider {
        Provider {
            id: id.to_string(),
            name: format!("Provider {id}"),
            settings_config: json!({"config": "model_provider = \"custom\""}),
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
    fn store_round_trips_json_and_source_identity() {
        let dir = tempdir().unwrap();
        let store = ProviderStore::open(dir.path().join("codex-route.db")).unwrap();
        store
            .insert(&provider(
                "p1",
                ProviderSource::CcSwitch {
                    source_id: "p1".to_string(),
                    source_updated_at: None,
                },
            ))
            .unwrap();

        let loaded = store.get("p1").unwrap().unwrap();
        assert_eq!(loaded.meta["unknown"]["kept"], json!(true));
        assert!(
            matches!(loaded.source, ProviderSource::CcSwitch { ref source_id, .. } if source_id == "p1")
        );
    }

    #[test]
    fn source_index_finds_imported_provider() {
        let dir = tempdir().unwrap();
        let store = ProviderStore::open(dir.path().join("codex-route.db")).unwrap();
        store
            .insert(&provider(
                "p1",
                ProviderSource::CcSwitch {
                    source_id: "source-p1".to_string(),
                    source_updated_at: None,
                },
            ))
            .unwrap();
        assert_eq!(store.find_imported("source-p1").unwrap().unwrap().id, "p1");
    }
}
