use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkspacePathError {
    #[error("workspace path must not be empty")]
    Empty,
    #[error("workspace path must be absolute: {0}")]
    Relative(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceRouteRule {
    pub workspace: PathBuf,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Canonicalize existing workspaces and lexically normalize missing paths.
pub fn normalize_workspace_path(path: &Path) -> Result<PathBuf, WorkspacePathError> {
    if path.as_os_str().is_empty() {
        return Err(WorkspacePathError::Empty);
    }
    if !path.is_absolute() {
        return Err(WorkspacePathError::Relative(path.to_path_buf()));
    }
    if let Ok(canonical) = fs::canonicalize(path) {
        return Ok(canonical);
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalizes_existing_workspace_to_canonical_path() {
        let directory = tempdir().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let path = nested.join("..").join("nested");

        assert_eq!(normalize_workspace_path(&path).unwrap(), nested);
    }

    #[test]
    fn normalizes_missing_absolute_workspace_lexically() {
        let path = PathBuf::from("/tmp/codex-route-missing/child/../project");
        assert_eq!(
            normalize_workspace_path(&path).unwrap(),
            PathBuf::from("/tmp/codex-route-missing/project")
        );
    }

    #[test]
    fn rejects_empty_and_relative_workspaces() {
        assert_eq!(
            normalize_workspace_path(Path::new("")),
            Err(WorkspacePathError::Empty)
        );
        assert!(matches!(
            normalize_workspace_path(Path::new("project")),
            Err(WorkspacePathError::Relative(_))
        ));
    }
}
