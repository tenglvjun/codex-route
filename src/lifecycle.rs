use crate::config::ScanConfig;
use crate::provider_store::ProviderStore;
use crate::route::{RouteStartupError, RouteState};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const CONFIG_MARKER: &str = "# codex-route-managed: v1";
const ROUTE_PROVIDER_ID: &str = "codex-route";
const ROUTE_BEARER_TOKEN: &str = "codex-route-managed";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct LifecyclePaths {
    pub data_dir: PathBuf,
    pub codex_home: PathBuf,
}

impl LifecyclePaths {
    pub fn new(data_dir: PathBuf, codex_home: PathBuf) -> Self {
        Self {
            data_dir,
            codex_home,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.data_dir.join("route-state.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.data_dir.join("route.lock")
    }

    fn operation_lock_path(&self) -> PathBuf {
        self.data_dir.join("route.operation.lock")
    }

    pub fn backup_path(&self) -> PathBuf {
        self.data_dir.join("codex-config.toml.bak")
    }

    pub fn log_path(&self) -> PathBuf {
        self.data_dir.join("route.log")
    }
}

#[derive(Debug, Clone)]
pub struct ActivateOptions {
    pub paths: LifecyclePaths,
    pub provider_id: Option<String>,
    pub port: u16,
    pub scan_config: ScanConfig,
}

#[derive(Debug, Clone)]
pub struct DeactivateOptions {
    pub paths: LifecyclePaths,
}

#[derive(Debug, Clone)]
pub struct StatusOptions {
    pub paths: LifecyclePaths,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivationResult {
    pub status: &'static str,
    pub pid: u32,
    pub port: u16,
    #[serde(rename = "routeUrl")]
    pub route_url: String,
    #[serde(rename = "configPath")]
    pub config_path: PathBuf,
    #[serde(rename = "statePath")]
    pub state_path: PathBuf,
    #[serde(rename = "lockPath")]
    pub lock_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeactivationResult {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(rename = "configRestored")]
    pub config_restored: bool,
    #[serde(rename = "configPath")]
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleStatus {
    pub status: &'static str,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(rename = "serverReachable")]
    pub server_reachable: bool,
    #[serde(rename = "configManaged")]
    pub config_managed: bool,
    #[serde(rename = "externalModification")]
    pub external_modification: bool,
    #[serde(rename = "configPath")]
    pub config_path: PathBuf,
    #[serde(rename = "statePath")]
    pub state_path: PathBuf,
    #[serde(rename = "lockPath")]
    pub lock_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("route is already active (pid {0})")]
    AlreadyActive(u32),
    #[error("route service is not active")]
    NotActive,
    #[error("route port {0} is already in use")]
    PortInUse(u16),
    #[error("route port must be between 1 and 65535")]
    InvalidPort,
    #[error("route service did not become healthy within {0} seconds")]
    StartupTimeout(u64),
    #[error("route service exited before becoming healthy")]
    StartupExited,
    #[error("Codex config.toml is externally modified; refusing to overwrite it")]
    ExternalModification,
    #[error("Codex config.toml contains an unowned '{ROUTE_PROVIDER_ID}' provider")]
    UnownedRouteProvider,
    #[error("managed route state is invalid: {0}")]
    InvalidState(String),
    #[error("managed config backup is missing: {0}")]
    MissingBackup(PathBuf),
    #[error("cannot parse Codex config.toml: {0}")]
    InvalidConfigToml(String),
    #[error("Codex config.toml has an invalid model_providers table")]
    InvalidModelProviders,
    #[error("failed to launch route service: {0}")]
    Launch(#[source] io::Error),
    #[error("route service failed to stop: {0}")]
    Stop(String),
    #[error("filesystem operation failed: {0}")]
    Io(#[source] io::Error),
    #[error("failed to serialize lifecycle state: {0}")]
    Json(#[source] serde_json::Error),
    #[error("route startup validation failed: {0}")]
    RouteStartup(#[source] RouteStartupError),
}

impl From<io::Error> for LifecycleError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleState {
    version: u8,
    config_path: PathBuf,
    backup_path: PathBuf,
    original_exists: bool,
    #[serde(default)]
    original_hash: Option<String>,
    managed_hash: String,
    port: u16,
    provider_id: Option<String>,
    pid: Option<u32>,
}

#[derive(Debug)]
pub struct DaemonLock {
    path: PathBuf,
}

impl DaemonLock {
    pub fn acquire(path: &Path) -> Result<Self, LifecycleError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    LifecycleError::AlreadyActive(read_pid(path).unwrap_or(0))
                } else {
                    LifecycleError::Io(error)
                }
            })?;
        writeln!(file, "{}", std::process::id())?;
        file.sync_all()?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn activate(options: ActivateOptions) -> Result<ActivationResult, LifecycleError> {
    if options.port == 0 {
        return Err(LifecycleError::InvalidPort);
    }
    fs::create_dir_all(&options.paths.data_dir)?;
    fs::create_dir_all(&options.paths.codex_home)?;
    let _operation_lock = acquire_operation_lock(&options.paths.operation_lock_path())?;

    let store = Arc::new(
        ProviderStore::open(options.paths.data_dir.join("codex-route.db"))
            .map_err(|error| LifecycleError::InvalidState(error.to_string()))?,
    );
    let route_state = RouteState::with_scan_config(
        store,
        options.provider_id.clone(),
        options.scan_config.clone(),
    )
    .map_err(LifecycleError::RouteStartup)?;
    route_state
        .validate_selection()
        .map_err(LifecycleError::RouteStartup)?;

    if options.paths.lock_path().exists() {
        match read_pid(&options.paths.lock_path()) {
            Some(pid) if process_is_alive(pid) => {
                return Err(LifecycleError::AlreadyActive(pid));
            }
            _ => {
                let _ = fs::remove_file(options.paths.lock_path());
            }
        }
    }

    let mut existing_config_needs_projection = false;
    let existing_state = read_state(&options.paths.state_path())?;
    if let Some(state) = existing_state.as_ref() {
        validate_state_paths(state, &options.paths)?;
        let current = read_optional(&state.config_path)?;
        let matches_managed = current
            .as_deref()
            .is_some_and(|contents| content_hash(contents.as_bytes()) == state.managed_hash);
        let matches_original = current.as_deref().is_some_and(|contents| {
            state
                .original_hash
                .as_deref()
                .is_some_and(|hash| content_hash(contents.as_bytes()) == hash)
        }) || (!state.original_exists && current.is_none());
        if !matches_managed && !matches_original {
            return Err(LifecycleError::ExternalModification);
        }
        if state.original_exists && !state.backup_path.is_file() {
            return Err(LifecycleError::MissingBackup(state.backup_path.clone()));
        }
        existing_config_needs_projection = !matches_managed;
        if let Some(pid) = state.pid.filter(|pid| process_is_alive(*pid)) {
            if server_is_reachable(state.port) {
                return Err(LifecycleError::AlreadyActive(pid));
            }
        }
        if state.port != options.port {
            return Err(LifecycleError::InvalidState(format!(
                "stale activation uses port {}, requested {}",
                state.port, options.port
            )));
        }
    }

    if let Ok(listener) = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port)) {
        drop(listener);
    } else {
        return Err(LifecycleError::PortInUse(options.port));
    }

    let state_path = options.paths.state_path();
    let config_path = options.paths.config_path();
    let (state, changed_config) = match existing_state {
        Some(mut state) => {
            if existing_config_needs_projection {
                let original = if state.original_exists {
                    if !state.backup_path.is_file() {
                        return Err(LifecycleError::MissingBackup(state.backup_path));
                    }
                    let original = fs::read_to_string(&state.backup_path)?;
                    if state
                        .original_hash
                        .as_deref()
                        .is_some_and(|hash| content_hash(original.as_bytes()) != hash)
                    {
                        return Err(LifecycleError::ExternalModification);
                    }
                    Some(original)
                } else {
                    None
                };
                let managed = project_config(original.as_deref(), options.port)?;
                state.managed_hash = content_hash(managed.as_bytes());
                state.port = options.port;
                if let Err(error) = write_state(&state_path, &state) {
                    rollback_activation(&options.paths, &state, true);
                    return Err(error);
                }
                if let Err(error) = atomic_write(&config_path, managed.as_bytes()) {
                    rollback_activation(&options.paths, &state, true);
                    return Err(error);
                }
            }
            state.provider_id = options.provider_id.clone();
            state.pid = None;
            if let Err(error) = write_state(&state_path, &state) {
                if existing_config_needs_projection {
                    rollback_activation(&options.paths, &state, true);
                }
                return Err(error);
            }
            (state, existing_config_needs_projection)
        }
        None => {
            let original = read_optional(&config_path)?;
            if original
                .as_deref()
                .is_some_and(|contents| contents.lines().any(|line| line.trim() == CONFIG_MARKER))
            {
                return Err(LifecycleError::InvalidState(
                    "managed config marker exists without lifecycle state".into(),
                ));
            }
            let managed = project_config(original.as_deref(), options.port)?;
            if original
                .as_deref()
                .is_some_and(contains_unowned_route_provider)
            {
                return Err(LifecycleError::UnownedRouteProvider);
            }
            let backup_path = options.paths.backup_path();
            if let Some(original) = original.as_deref() {
                atomic_write(&backup_path, original.as_bytes())?;
                set_private_permissions(&backup_path)?;
            }
            let state = LifecycleState {
                version: 1,
                config_path: config_path.clone(),
                backup_path,
                original_exists: original.is_some(),
                original_hash: original.map(|contents| content_hash(contents.as_bytes())),
                managed_hash: content_hash(managed.as_bytes()),
                port: options.port,
                provider_id: options.provider_id.clone(),
                pid: None,
            };
            if let Err(error) = write_state(&state_path, &state) {
                rollback_activation(&options.paths, &state, true);
                return Err(error);
            }
            if let Err(error) = atomic_write(&config_path, managed.as_bytes()) {
                rollback_activation(&options.paths, &state, true);
                return Err(error);
            }
            (state, true)
        }
    };

    let mut child = match spawn_service(
        &options.paths,
        &options.provider_id,
        &options.scan_config,
        options.port,
    ) {
        Ok(child) => child,
        Err(error) => {
            rollback_activation(&options.paths, &state, changed_config);
            return Err(error);
        }
    };
    // Persist the child PID before health polling so a crash in the parent
    // cannot leave a healthy route process untracked by lifecycle state.
    let mut state = state;
    state.pid = Some(child.id());
    if let Err(error) = write_state(&state_path, &state) {
        if let Err(stop_error) = stop_child(&mut child) {
            let _ = write_state(&state_path, &state);
            return Err(LifecycleError::Stop(stop_error));
        }
        let _ = child.wait();
        rollback_activation(&options.paths, &state, changed_config);
        return Err(error);
    }
    if !wait_for_health(&mut child, options.port) {
        let exited = child.try_wait().ok().flatten().is_some();
        if !exited {
            if let Err(error) = stop_child(&mut child) {
                let mut failed_state = state.clone();
                failed_state.pid = Some(child.id());
                let _ = write_state(&state_path, &failed_state);
                return Err(LifecycleError::Stop(error));
            }
        }
        let _ = child.wait();
        rollback_activation(&options.paths, &state, changed_config);
        return Err(if exited {
            LifecycleError::StartupExited
        } else {
            LifecycleError::StartupTimeout(STARTUP_TIMEOUT.as_secs())
        });
    }

    if let Err(error) = write_state(&state_path, &state) {
        if let Err(stop_error) = stop_child(&mut child) {
            let _ = write_state(&state_path, &state);
            return Err(LifecycleError::Stop(stop_error));
        }
        let _ = child.wait();
        rollback_activation(&options.paths, &state, changed_config);
        return Err(error);
    }
    Ok(ActivationResult {
        status: "active",
        pid: child.id(),
        port: options.port,
        route_url: format!("http://127.0.0.1:{}/v1", options.port),
        config_path,
        state_path,
        lock_path: options.paths.lock_path(),
    })
}

pub fn deactivate(options: DeactivateOptions) -> Result<DeactivationResult, LifecycleError> {
    fs::create_dir_all(&options.paths.data_dir)?;
    let _operation_lock = acquire_operation_lock(&options.paths.operation_lock_path())?;
    let state_path = options.paths.state_path();
    let Some(state) = read_state(&state_path)? else {
        return Err(LifecycleError::NotActive);
    };
    validate_state_paths(&state, &options.paths)?;
    let pid = lifecycle_pid(&state, &options.paths.lock_path());
    let current = read_optional(&state.config_path)?;
    let matches_managed = current
        .as_deref()
        .is_some_and(|contents| content_hash(contents.as_bytes()) == state.managed_hash);
    let matches_original = current.as_deref().is_some_and(|contents| {
        state
            .original_hash
            .as_deref()
            .is_some_and(|hash| content_hash(contents.as_bytes()) == hash)
    }) || (!state.original_exists && current.is_none());
    if !matches_managed && (!matches_original || pid.is_some()) {
        return Err(LifecycleError::ExternalModification);
    }

    // Verify the backup before stopping the service. This prevents a damaged
    // backup from turning a successful stop into a destructive config restore.
    if matches_managed && state.original_exists {
        let _ = read_verified_backup(&state)?;
    }

    if let Some(pid) = pid {
        let lock_pid = read_pid(&options.paths.lock_path()).ok_or_else(|| {
            LifecycleError::Stop("route lock is missing; refusing to signal an unowned PID".into())
        })?;
        if lock_pid != pid {
            return Err(LifecycleError::Stop(
                "route lock PID does not match lifecycle state".into(),
            ));
        }
        if !process_is_route_service(pid) {
            return Err(LifecycleError::Stop(
                "route lock PID is not a codex-route service".into(),
            ));
        }
        stop_process(pid).map_err(LifecycleError::Stop)?;
    }

    // The service can touch the Codex config while it is shutting down. Do not
    // overwrite those changes; leave lifecycle state in place for recovery.
    if matches_managed {
        let current_after_stop = read_optional(&state.config_path)?;
        let still_managed = current_after_stop
            .as_deref()
            .is_some_and(|contents| content_hash(contents.as_bytes()) == state.managed_hash);
        if !still_managed {
            return Err(LifecycleError::ExternalModification);
        }
    }

    let backup = if matches_managed && state.original_exists {
        // Re-read and re-validate immediately before writing. The initial
        // verification protects the stop path; this one protects the restore
        // path if another process replaced the backup while stopping.
        Some(read_verified_backup(&state)?)
    } else {
        None
    };
    if let Some(backup) = backup {
        atomic_write(&state.config_path, &backup)?;
    } else if matches_managed && state.config_path.exists() {
        fs::remove_file(&state.config_path)?;
    }
    let _ = fs::remove_file(options.paths.lock_path());
    let _ = fs::remove_file(&state.backup_path);
    let _ = fs::remove_file(&state_path);
    Ok(DeactivationResult {
        status: "inactive",
        pid: pid.or(state.pid),
        config_restored: true,
        config_path: state.config_path,
    })
}

pub fn status(options: StatusOptions) -> Result<LifecycleStatus, LifecycleError> {
    let state_path = options.paths.state_path();
    let lock_path = options.paths.lock_path();
    let state = read_state(&state_path)?;
    let config_path = state
        .as_ref()
        .map(|state| state.config_path.clone())
        .unwrap_or_else(|| options.paths.config_path());
    let pid = state
        .as_ref()
        .and_then(|state| lifecycle_pid(state, &lock_path));
    let port = state.as_ref().map(|state| state.port);
    let server_reachable = port.is_some_and(server_is_reachable);
    let active = pid.is_some_and(process_is_alive) && server_reachable && lock_path.exists();
    let external_modification = state.as_ref().is_some_and(|state| {
        let current = read_optional(&state.config_path).ok().flatten();
        let matches_managed = current
            .as_deref()
            .is_some_and(|contents| content_hash(contents.as_bytes()) == state.managed_hash);
        let matches_original = current.as_deref().is_some_and(|contents| {
            state
                .original_hash
                .as_deref()
                .is_some_and(|hash| content_hash(contents.as_bytes()) == hash)
        }) || (!state.original_exists && current.is_none());
        !matches_managed && (!matches_original || active)
    });
    let status = if external_modification {
        "external_modified"
    } else if active {
        "active"
    } else {
        "inactive"
    };
    Ok(LifecycleStatus {
        status,
        active,
        pid,
        port,
        server_reachable,
        config_managed: state.is_some(),
        external_modification,
        config_path,
        state_path,
        lock_path,
    })
}

/// Returns the Codex home recorded by an existing activation, if any.
/// This lets `route status` and `route deactivate` work without repeating a
/// custom `--codex-home` used during activation.
pub fn codex_home_from_state(data_dir: &Path) -> Option<PathBuf> {
    let state = read_state(&data_dir.join("route-state.json")).ok()??;
    state.config_path.parent().map(Path::to_path_buf)
}

fn spawn_service(
    paths: &LifecyclePaths,
    provider_id: &Option<String>,
    scan_config: &ScanConfig,
    port: u16,
) -> Result<Child, LifecycleError> {
    let executable = std::env::current_exe().map_err(LifecycleError::Launch)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log_path())
        .map_err(LifecycleError::Launch)?;
    let stderr = log.try_clone().map_err(LifecycleError::Launch)?;
    let mut command = Command::new(executable);
    command
        .args(["route", "serve"])
        .arg("--data-dir")
        .arg(&paths.data_dir)
        .arg("--codex-home")
        .arg(&scan_config.codex_home)
        .arg("--max-rollout-bytes")
        .arg(scan_config.max_rollout_bytes.to_string())
        .arg("--port")
        .arg(port.to_string())
        .arg("--lifecycle-lock")
        .arg(paths.lock_path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        // Keep the long-lived route daemon out of the activation command's
        // console/process group. In particular, this prevents Windows test
        // harness output pipes from remaining open after `activate` returns.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    if let Some(provider_id) = provider_id {
        command.arg("--provider").arg(provider_id);
    }
    #[cfg(windows)]
    prevent_standard_handle_inheritance();
    command.spawn().map_err(LifecycleError::Launch)
}

fn wait_for_health(child: &mut Child, port: u16) -> bool {
    let deadline = std::time::Instant::now() + STARTUP_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return false;
        }
        if server_is_reachable(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn server_is_reachable(port: u16) -> bool {
    let Ok(mut stream) = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)) else {
        return false;
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    stream
        .write_all(
            "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".as_bytes(),
        )
        .is_ok()
        && {
            let mut response = [0_u8; 64];
            stream.read(&mut response).is_ok_and(|size| {
                std::str::from_utf8(&response[..size])
                    .is_ok_and(|text| text.starts_with("HTTP/1.1 200"))
            })
        }
}

fn terminate_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    let result = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    #[cfg(windows)]
    let result = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .status();
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("failed to signal pid {pid}: {status}")),
        Err(error) => Err(format!("failed to signal pid {pid}: {error}")),
    }
}

fn stop_process(pid: u32) -> Result<(), String> {
    if !process_is_alive(pid) {
        return Ok(());
    }
    if let Err(error) = terminate_process(pid) {
        if !process_is_alive(pid) {
            return Ok(());
        }
        return Err(error);
    }
    wait_for_exit(pid)
}

fn stop_child(child: &mut Child) -> Result<(), String> {
    if child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(());
    }
    let pid = child.id();
    if let Err(error) = terminate_process(pid) {
        if child
            .try_wait()
            .map_err(|wait_error| wait_error.to_string())?
            .is_some()
        {
            return Ok(());
        }
        return Err(error);
    }
    let deadline = std::time::Instant::now() + SHUTDOWN_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.kill().map_err(|error| error.to_string())?;
    child
        .wait()
        .map(|_| ())
        .map_err(|error| format!("process {pid} did not exit: {error}"))
}

fn wait_for_exit(pid: u32) -> Result<(), String> {
    let deadline = std::time::Instant::now() + SHUTDOWN_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if !process_is_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    force_terminate_process(pid)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while std::time::Instant::now() < deadline {
        if !process_is_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("process {pid} did not exit after termination"))
}

fn force_terminate_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    let result = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status();
    #[cfg(windows)]
    let result = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .status();
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("failed to terminate pid {pid}: {status}")),
        Err(error) => Err(format!("failed to terminate pid {pid}: {error}")),
    }
}

fn process_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    let result = Command::new("kill").args(["-0", &pid.to_string()]).status();
    #[cfg(windows)]
    let result = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}")])
        .output();
    #[cfg(unix)]
    {
        if !result.is_ok_and(|status| status.success()) {
            return false;
        }
        let Ok(output) = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "stat="])
            .output()
        else {
            return true;
        };
        let status = String::from_utf8_lossy(&output.stdout);
        !status.trim_start().starts_with('Z')
    }
    #[cfg(windows)]
    {
        result
            .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
    }
}

fn process_is_route_service(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(output) = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
        else {
            return false;
        };
        let command = String::from_utf8_lossy(&output.stdout);
        command.contains("codex-route") && command.contains("route serve")
    }
    #[cfg(windows)]
    {
        let Ok(output) = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
        else {
            return false;
        };
        let command = String::from_utf8_lossy(&output.stdout);
        command.to_ascii_lowercase().contains("codex-route")
    }
}

fn lifecycle_pid(state: &LifecycleState, lock_path: &Path) -> Option<u32> {
    state
        .pid
        .filter(|pid| process_is_alive(*pid))
        // Older/in-flight state files may not have persisted the PID yet. If
        // the lifecycle lock identifies a live route service, use it as the
        // recovery source rather than orphaning the child process.
        .or_else(|| {
            state
                .pid
                .is_none()
                .then(|| read_pid(lock_path))
                .flatten()
                .filter(|pid| process_is_alive(*pid))
        })
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn acquire_operation_lock(path: &Path) -> Result<DaemonLock, LifecycleError> {
    match DaemonLock::acquire(path) {
        Ok(lock) => Ok(lock),
        Err(LifecycleError::AlreadyActive(pid)) if pid == 0 || !process_is_alive(pid) => {
            let _ = fs::remove_file(path);
            DaemonLock::acquire(path)
        }
        Err(error) => Err(error),
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn read_verified_backup(state: &LifecycleState) -> Result<Vec<u8>, LifecycleError> {
    let backup = fs::read(&state.backup_path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            LifecycleError::MissingBackup(state.backup_path.clone())
        } else {
            LifecycleError::Io(error)
        }
    })?;
    let Some(expected_hash) = state.original_hash.as_deref() else {
        return Err(LifecycleError::InvalidState(
            "lifecycle state is missing the original config hash".into(),
        ));
    };
    if content_hash(&backup) != expected_hash {
        return Err(LifecycleError::ExternalModification);
    }
    Ok(backup)
}

fn read_state(path: &Path) -> Result<Option<LifecycleState>, LifecycleError> {
    let Some(contents) = read_optional(path)? else {
        return Ok(None);
    };
    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| LifecycleError::InvalidState(error.to_string()))
}

fn write_state(path: &Path, state: &LifecycleState) -> Result<(), LifecycleError> {
    let contents = serde_json::to_vec_pretty(state).map_err(LifecycleError::Json)?;
    atomic_write(path, &contents)
}

fn validate_state_paths(
    state: &LifecycleState,
    paths: &LifecyclePaths,
) -> Result<(), LifecycleError> {
    if state.version != 1 {
        return Err(LifecycleError::InvalidState(format!(
            "unsupported lifecycle state version {}",
            state.version
        )));
    }
    if state.config_path != paths.config_path() {
        return Err(LifecycleError::InvalidState(
            "Codex home does not match active state".into(),
        ));
    }
    if state.backup_path != paths.backup_path() {
        return Err(LifecycleError::InvalidState(
            "lifecycle backup path does not match the data directory".into(),
        ));
    }
    if state.port == 0 {
        return Err(LifecycleError::InvalidState(
            "lifecycle state contains an invalid port".into(),
        ));
    }
    if state.original_exists && state.original_hash.is_none() {
        return Err(LifecycleError::InvalidState(
            "lifecycle state is missing the original config hash".into(),
        ));
    }
    Ok(())
}

fn rollback_activation(paths: &LifecyclePaths, state: &LifecycleState, changed_config: bool) {
    if changed_config {
        if state.original_exists {
            if let Ok(contents) = fs::read(&state.backup_path) {
                let _ = atomic_write(&state.config_path, &contents);
            }
        } else {
            let _ = fs::remove_file(&state.config_path);
        }
        let _ = fs::remove_file(&state.backup_path);
    }
    if changed_config {
        let _ = fs::remove_file(paths.state_path());
    } else {
        let _ = write_state(&paths.state_path(), state);
    }
}

fn project_config(original: Option<&str>, port: u16) -> Result<String, LifecycleError> {
    let mut document = match original {
        Some(text) => text
            .parse::<toml::Value>()
            .map_err(|error| LifecycleError::InvalidConfigToml(error.to_string()))?,
        None => toml::Value::Table(toml::map::Map::new()),
    };
    let table = document.as_table_mut().ok_or_else(|| {
        LifecycleError::InvalidConfigToml("top-level value must be a table".into())
    })?;
    table.insert(
        "model_provider".into(),
        toml::Value::String(ROUTE_PROVIDER_ID.into()),
    );
    let providers = table
        .entry("model_providers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let providers = providers
        .as_table_mut()
        .ok_or(LifecycleError::InvalidModelProviders)?;
    let mut route_provider = toml::map::Map::new();
    route_provider.insert("name".into(), toml::Value::String("codex-route".into()));
    route_provider.insert(
        "base_url".into(),
        toml::Value::String(format!("http://127.0.0.1:{port}/v1")),
    );
    route_provider.insert("wire_api".into(), toml::Value::String("responses".into()));
    route_provider.insert("requires_openai_auth".into(), toml::Value::Boolean(false));
    route_provider.insert(
        "experimental_bearer_token".into(),
        toml::Value::String(ROUTE_BEARER_TOKEN.into()),
    );
    providers.insert(ROUTE_PROVIDER_ID.into(), toml::Value::Table(route_provider));
    let serialized = toml::to_string(&document)
        .map_err(|error| LifecycleError::InvalidConfigToml(error.to_string()))?;
    Ok(format!("{CONFIG_MARKER}\n{serialized}"))
}

fn contains_unowned_route_provider(contents: &str) -> bool {
    contents
        .parse::<toml::Value>()
        .ok()
        .is_some_and(|document| {
            document
                .get("model_providers")
                .and_then(|providers| providers.get(ROUTE_PROVIDER_ID))
                .is_some()
                && !contents.lines().any(|line| line.trim() == CONFIG_MARKER)
        })
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), LifecycleError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = path.with_extension(format!("tmp-{}-{suffix}", std::process::id()));
    let existing_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    if let Some(permissions) = existing_permissions {
        fs::set_permissions(&temp, permissions)?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        #[cfg(windows)]
        {
            if path.exists() {
                fs::remove_file(path)?;
                fs::rename(&temp, path)?;
                return Ok(());
            }
        }
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}

fn set_private_permissions(path: &Path) -> Result<(), LifecycleError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(windows)]
fn prevent_standard_handle_inheritance() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};

    // `Command` inherits every inheritable handle on Windows. The activation
    // command may itself have stdout/stderr pipes owned by a caller (for
    // example `assert_cmd` or a shell); if the detached daemon keeps those
    // handles open, the caller waits forever for EOF. Keep the daemon's
    // explicitly configured log handles inheritable, but make the parent's
    // standard handles ineligible before spawning it.
    let handles = [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ];
    for handle in handles {
        if !handle.is_null() {
            // SAFETY: the handles are borrowed standard handles owned by the
            // current process; SetHandleInformation only changes inheritance.
            unsafe {
                let _ = SetHandleInformation(handle as HANDLE, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

fn content_hash(contents: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in contents {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths() -> (TempDir, LifecyclePaths) {
        let directory = TempDir::new().unwrap();
        let codex_home = directory.path().join(".codex");
        let data_dir = directory.path().join("data");
        fs::create_dir_all(&codex_home).unwrap();
        (directory, LifecyclePaths::new(data_dir, codex_home))
    }

    #[test]
    fn projects_route_provider_and_preserves_existing_config_values() {
        let config = "model = \"gpt-5-codex\"\n[model_providers.other]\nbase_url = \"https://example.test/v1\"\n";
        let projected = project_config(Some(config), 16729).unwrap();
        assert!(projected.starts_with(CONFIG_MARKER));
        let document = projected.parse::<toml::Value>().unwrap();
        assert_eq!(document["model"].as_str(), Some("gpt-5-codex"));
        assert_eq!(document["model_provider"].as_str(), Some(ROUTE_PROVIDER_ID));
        assert_eq!(
            document["model_providers"][ROUTE_PROVIDER_ID]["base_url"].as_str(),
            Some("http://127.0.0.1:16729/v1")
        );
    }

    #[test]
    fn atomic_config_restore_round_trip() {
        let (_directory, paths) = paths();
        let original = "model_provider = \"custom\"\n";
        fs::write(paths.config_path(), original).unwrap();
        let managed = project_config(Some(original), 16729).unwrap();
        let backup = paths.backup_path();
        atomic_write(&backup, original.as_bytes()).unwrap();
        atomic_write(&paths.config_path(), managed.as_bytes()).unwrap();
        assert_eq!(fs::read_to_string(paths.config_path()).unwrap(), managed);
        atomic_write(&paths.config_path(), &fs::read(&backup).unwrap()).unwrap();
        assert_eq!(fs::read_to_string(paths.config_path()).unwrap(), original);
    }

    #[test]
    fn daemon_lock_is_exclusive_and_released() {
        let (_directory, paths) = paths();
        let first = DaemonLock::acquire(&paths.lock_path()).unwrap();
        assert!(matches!(
            DaemonLock::acquire(&paths.lock_path()),
            Err(LifecycleError::AlreadyActive(_))
        ));
        drop(first);
        DaemonLock::acquire(&paths.lock_path()).unwrap();
    }
}
