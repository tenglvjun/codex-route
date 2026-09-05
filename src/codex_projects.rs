use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

use crate::workspace_rule::normalize_workspace_path;

const PROJECT_DATABASE_FILENAME: &str = "state_5.sqlite";

#[derive(Debug, Error)]
pub enum ProjectRootsError {
    #[error("cannot open Codex project database {path}: {source}")]
    Open {
        path: PathBuf,
        source: rusqlite::Error,
    },
    #[error("failed to query Codex project database: {0}")]
    Query(#[from] rusqlite::Error),
}

/// Read the project roots saved by Codex Desktop from its read-only state DB.
pub fn read_saved_project_roots(codex_home: &Path) -> Result<Vec<PathBuf>, ProjectRootsError> {
    let path = codex_home.join(PROJECT_DATABASE_FILENAME);
    let connection =
        Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|source| {
            ProjectRootsError::Open {
                path: path.clone(),
                source,
            }
        })?;
    connection.busy_timeout(Duration::from_secs(2))?;

    let mut statement = connection.prepare(
        "SELECT project_roots.path
         FROM project_roots
         JOIN projects ON projects.id = project_roots.project_id
         ORDER BY projects.position ASC, projects.id ASC, project_roots.position ASC",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

    let mut roots = BTreeSet::new();
    for row in rows {
        let path = PathBuf::from(row?);
        if !path.is_absolute() {
            continue;
        }
        if let Ok(path) = normalize_workspace_path(&path) {
            roots.insert(path);
        }
    }
    Ok(roots.into_iter().collect())
}

/// Load project roots for optional filtering. A missing, locked, or older
/// project database must not prevent the Dashboard from rendering.
pub fn load_saved_project_roots(codex_home: &Path) -> Vec<PathBuf> {
    read_saved_project_roots(codex_home).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

    fn create_database(home: &Path) {
        let connection = Connection::open(home.join(PROJECT_DATABASE_FILENAME)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE projects (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    metadata TEXT NOT NULL DEFAULT '{}',
                    position INTEGER NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE project_roots (
                    project_id TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    path TEXT NOT NULL,
                    PRIMARY KEY (project_id, position)
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO projects
                 (id, name, position, created_at_ms, updated_at_ms)
                 VALUES (?1, 'Project', 0, 0, 0)",
                params!["project-1"],
            )
            .unwrap();
    }

    #[test]
    fn reads_and_normalizes_saved_project_roots() {
        let home = TempDir::new().unwrap();
        create_database(home.path());
        let root = home.path().join("workspace");
        Connection::open(home.path().join(PROJECT_DATABASE_FILENAME))
            .unwrap()
            .execute(
                "INSERT INTO project_roots (project_id, position, path) VALUES (?1, 0, ?2)",
                params!["project-1", root.join("nested/..").to_string_lossy()],
            )
            .unwrap();

        assert_eq!(read_saved_project_roots(home.path()).unwrap(), vec![root]);
    }

    #[test]
    fn unavailable_or_invalid_databases_are_safe_to_ignore() {
        let home = TempDir::new().unwrap();
        assert!(read_saved_project_roots(home.path()).is_err());
        assert!(load_saved_project_roots(home.path()).is_empty());

        std::fs::write(home.path().join(PROJECT_DATABASE_FILENAME), b"not sqlite").unwrap();
        assert!(read_saved_project_roots(home.path()).is_err());
        assert!(load_saved_project_roots(home.path()).is_empty());
    }
}
