use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

pub const DEFAULT_MAX_ROLLOUT_BYTES: u64 = 64 * 1024;
const MAX_ALLOWED_ROLLOUT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    pub codex_home: PathBuf,
    pub max_rollout_bytes: u64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("--codex-home must be an absolute path: {0}")]
    RelativeCodexHome(PathBuf),
    #[error("CODEX_HOME must be an absolute path: {0}")]
    RelativeEnvironmentHome(PathBuf),
    #[error("cannot determine the user's home directory")]
    HomeDirectoryUnavailable,
    #[error("max-rollout-bytes must be between 1 and {MAX_ALLOWED_ROLLOUT_BYTES} bytes")]
    InvalidMaxRolloutBytes(u64),
}

impl ScanConfig {
    pub fn from_cli(
        codex_home: Option<PathBuf>,
        max_rollout_bytes: Option<u64>,
    ) -> Result<Self, ConfigError> {
        Self::from_sources(codex_home, max_rollout_bytes, |key| env::var_os(key))
    }

    pub fn from_sources<F>(
        codex_home: Option<PathBuf>,
        max_rollout_bytes: Option<u64>,
        env_value: F,
    ) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<OsString>,
    {
        let codex_home = match codex_home {
            Some(path) => {
                if !path.is_absolute() {
                    return Err(ConfigError::RelativeCodexHome(path));
                }
                path
            }
            None => match env_value("CODEX_HOME") {
                Some(value) => {
                    let path = PathBuf::from(value);
                    if !path.is_absolute() {
                        return Err(ConfigError::RelativeEnvironmentHome(path));
                    }
                    path
                }
                None => {
                    let home = env_value("HOME")
                        .or_else(|| env_value("USERPROFILE"))
                        .map(PathBuf::from)
                        .ok_or(ConfigError::HomeDirectoryUnavailable)?;
                    home.join(".codex")
                }
            },
        };

        let max_rollout_bytes = max_rollout_bytes.unwrap_or(DEFAULT_MAX_ROLLOUT_BYTES);
        if max_rollout_bytes == 0 || max_rollout_bytes > MAX_ALLOWED_ROLLOUT_BYTES {
            return Err(ConfigError::InvalidMaxRolloutBytes(max_rollout_bytes));
        }

        Ok(Self {
            codex_home,
            max_rollout_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        env::temp_dir().join("codex-route-config-tests").join(name)
    }

    fn env(home: &Path, codex_home: Option<&Path>) -> impl Fn(&str) -> Option<OsString> {
        let home = home.as_os_str().to_os_string();
        let codex_home = codex_home.map(|path| path.as_os_str().to_os_string());
        move |key| match key {
            "HOME" => Some(home.clone()),
            "CODEX_HOME" => codex_home.clone(),
            _ => None,
        }
    }

    #[test]
    fn explicit_home_wins_over_environment() {
        let explicit = fixture_path("explicit/.codex");
        let environment = fixture_path("environment");
        let environment_codex = environment.join(".codex");
        let config = ScanConfig::from_sources(
            Some(explicit.clone()),
            None,
            env(&environment, Some(&environment_codex)),
        )
        .expect("configuration should be valid");

        assert_eq!(
            config,
            ScanConfig {
                codex_home: explicit,
                max_rollout_bytes: DEFAULT_MAX_ROLLOUT_BYTES,
            }
        );
    }

    #[test]
    fn environment_home_precedes_default_home() {
        let default = fixture_path("default");
        let environment = fixture_path("environment");
        let environment_codex = environment.join(".codex");
        let config = ScanConfig::from_sources(None, None, env(&default, Some(&environment_codex)))
            .expect("configuration should be valid");

        assert_eq!(config.codex_home, environment_codex);
    }

    #[test]
    fn default_home_is_used_when_environment_override_is_absent() {
        let default = fixture_path("default");
        let config = ScanConfig::from_sources(None, None, env(&default, None))
            .expect("configuration should be valid");

        assert_eq!(config.codex_home, default.join(".codex"));
    }

    #[test]
    fn relative_and_invalid_limits_are_rejected() {
        let home = fixture_path("home");
        assert!(matches!(
            ScanConfig::from_sources(Some(PathBuf::from(".codex")), None, env(&home, None)),
            Err(ConfigError::RelativeCodexHome(_))
        ));
        assert!(matches!(
            ScanConfig::from_sources(None, Some(0), env(&home, None)),
            Err(ConfigError::InvalidMaxRolloutBytes(0))
        ));
        assert!(matches!(
            ScanConfig::from_sources(None, Some(MAX_ALLOWED_ROLLOUT_BYTES + 1), env(&home, None)),
            Err(ConfigError::InvalidMaxRolloutBytes(_))
        ));
    }
}
