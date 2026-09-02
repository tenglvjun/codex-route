use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use thiserror::Error;
use walkdir::WalkDir;

const ACTIVE_SESSIONS_DIR: &str = "sessions";
const ARCHIVED_SESSIONS_DIR: &str = "archived_sessions";
const ROLLOUT_PREFIX: &str = "rollout-";
const PLAIN_ROLLOUT_SUFFIX: &str = ".jsonl";
const COMPRESSED_ROLLOUT_SUFFIX: &str = ".jsonl.zst";

#[derive(Debug, Clone)]
pub struct RolloutSessionMeta {
    pub session_id: String,
    pub thread_id: String,
    pub workspace: PathBuf,
    pub timestamp: Option<String>,
    pub rollout_path: PathBuf,
    pub archived: bool,
    pub(crate) modified_at: Option<SystemTime>,
}

#[derive(Debug, Error)]
pub enum RolloutError {
    #[error("Codex home is unavailable at {path}: {source}")]
    CodexHomeUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to scan rollouts under {path}: {source}")]
    Scan {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read rollout {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("rollout {path} exceeded the {limit} byte scan limit")]
    ScanLimitExceeded { path: PathBuf, limit: u64 },
}

pub fn discover_rollouts(codex_home: &Path) -> Result<Vec<PathBuf>, RolloutError> {
    if !codex_home.is_dir() {
        return Err(RolloutError::CodexHomeUnavailable {
            path: codex_home.to_path_buf(),
            source: io::Error::new(io::ErrorKind::NotFound, "directory does not exist"),
        });
    }

    let mut paths = Vec::new();
    for directory in [ACTIVE_SESSIONS_DIR, ARCHIVED_SESSIONS_DIR] {
        let root = codex_home.join(directory);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root).follow_links(false).into_iter() {
            let entry = entry.map_err(|error| RolloutError::Scan {
                path: root.clone(),
                source: io::Error::other(error),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy();
            if is_rollout_name(&file_name) {
                paths.push(entry.into_path());
            }
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn read_session_meta(
    path: &Path,
    archived: bool,
    max_rollout_bytes: u64,
) -> Result<Option<RolloutSessionMeta>, RolloutError> {
    let file = fs::File::open(path).map_err(|source| RolloutError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let reader: Box<dyn Read> = if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(COMPRESSED_ROLLOUT_SUFFIX))
    {
        Box::new(
            zstd::stream::read::Decoder::new(file).map_err(|source| RolloutError::Read {
                path: path.to_path_buf(),
                source,
            })?,
        )
    } else {
        Box::new(file)
    };

    let modified_at = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    read_session_meta_from_reader(reader, path, archived, max_rollout_bytes, modified_at)
}

fn read_session_meta_from_reader(
    reader: Box<dyn Read>,
    path: &Path,
    archived: bool,
    max_rollout_bytes: u64,
    modified_at: Option<SystemTime>,
) -> Result<Option<RolloutSessionMeta>, RolloutError> {
    let mut reader = BufReader::new(reader.take(max_rollout_bytes.saturating_add(1)));
    let mut scanned_bytes = 0u64;
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes_read =
            reader
                .read_until(b'\n', &mut line)
                .map_err(|source| RolloutError::Read {
                    path: path.to_path_buf(),
                    source,
                })?;
        if bytes_read == 0 {
            return Ok(None);
        }
        scanned_bytes = scanned_bytes.saturating_add(bytes_read as u64);
        if scanned_bytes > max_rollout_bytes {
            return Err(RolloutError::ScanLimitExceeded {
                path: path.to_path_buf(),
                limit: max_rollout_bytes,
            });
        }
        if let Some(meta) = parse_session_meta_line(&line, path, archived, modified_at) {
            return Ok(Some(meta));
        }
    }
}

fn parse_session_meta_line(
    line: &[u8],
    path: &Path,
    archived: bool,
    modified_at: Option<SystemTime>,
) -> Option<RolloutSessionMeta> {
    let value = serde_json::from_slice::<Value>(line).ok()?;
    let object = value.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("session_meta") {
        return None;
    }

    let payload = object
        .get("payload")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let session_id = non_empty_string(payload.get("session_id")?)?;
    let thread_id = non_empty_string(payload.get("id")?)?;
    let workspace = PathBuf::from(non_empty_string(payload.get("cwd")?)?);
    if !workspace.is_absolute() {
        return None;
    }
    let timestamp = payload
        .get("timestamp")
        .and_then(Value::as_str)
        .or_else(|| object.get("timestamp").and_then(Value::as_str))
        .map(ToOwned::to_owned);

    Some(RolloutSessionMeta {
        session_id,
        thread_id,
        workspace,
        timestamp,
        rollout_path: path.to_path_buf(),
        archived,
        modified_at,
    })
}

fn non_empty_string(value: &Value) -> Option<String> {
    let value = value.as_str()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn is_rollout_name(file_name: &str) -> bool {
    file_name.starts_with(ROLLOUT_PREFIX)
        && (file_name.ends_with(PLAIN_ROLLOUT_SUFFIX)
            || file_name.ends_with(COMPRESSED_ROLLOUT_SUFFIX))
}

pub(crate) fn is_archived_path(codex_home: &Path, path: &Path) -> bool {
    path.starts_with(codex_home.join(ARCHIVED_SESSIONS_DIR))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("codex-route-rollout-tests")
            .join(name)
    }

    fn metadata_line(session_id: &str, thread_id: &str, cwd: &Path) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "timestamp": "2026-09-02T12:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "session_id": session_id,
                "id": thread_id,
                "timestamp": "2026-09-02T12:00:00Z",
                "cwd": cwd.to_string_lossy(),
                "originator": "codex",
                "cli_version": "test"
            }
        }))
        .expect("fixture should serialize")
    }

    #[test]
    fn parses_enveloped_session_metadata() {
        let workspace = fixture_path("project");
        let rollout = fixture_path("rollout.jsonl");
        let line = metadata_line("session-1", "thread-1", &workspace);
        let result = read_session_meta_from_reader(
            Box::new(Cursor::new(line)),
            &rollout,
            false,
            64 * 1024,
            None,
        )
        .expect("fixture should parse")
        .expect("metadata should be present");

        assert_eq!(result.session_id, "session-1");
        assert_eq!(result.thread_id, "thread-1");
        assert_eq!(result.workspace, workspace);
        assert!(!result.archived);
    }

    #[test]
    fn skips_invalid_lines_until_metadata() {
        let mut contents = b"not json\n".to_vec();
        let workspace = fixture_path("project");
        let rollout = fixture_path("rollout.jsonl");
        contents.extend(metadata_line("session-1", "thread-1", &workspace));
        let result = read_session_meta_from_reader(
            Box::new(Cursor::new(contents)),
            &rollout,
            true,
            64 * 1024,
            None,
        )
        .expect("fixture should parse")
        .expect("metadata should be present");

        assert!(result.archived);
    }

    #[test]
    fn rejects_relative_workspace_and_oversized_prefix() {
        let rollout = fixture_path("rollout.jsonl");
        let relative = metadata_line("session-1", "thread-1", Path::new("."));
        assert!(read_session_meta_from_reader(
            Box::new(Cursor::new(relative)),
            &rollout,
            false,
            64 * 1024,
            None,
        )
        .expect("relative paths should be ignored")
        .is_none());

        let oversized = vec![b'x'; 65];
        assert!(matches!(
            read_session_meta_from_reader(
                Box::new(Cursor::new(oversized)),
                &rollout,
                false,
                64,
                None,
            ),
            Err(RolloutError::ScanLimitExceeded { limit: 64, .. })
        ));
    }
}
