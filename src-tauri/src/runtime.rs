use codex_route::lifecycle::{EmbeddedRouteService, LifecycleError, LifecycleStatus};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;

const HEALTH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const MAX_RESTARTS: u32 = 5;
const HEALTHY_RESET_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    Stopped,
    Starting,
    Running,
    Degraded,
    Recovering,
    BlockedExternalModification,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub phase: RuntimePhase,
    pub active: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub server_reachable: bool,
    pub config_managed: bool,
    pub external_modification: bool,
    pub last_error: Option<String>,
    pub restart_count: u32,
    pub updated_at: i64,
    pub sequence: u64,
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            phase: RuntimePhase::Stopped,
            active: false,
            pid: None,
            port: None,
            server_reachable: false,
            config_managed: false,
            external_modification: false,
            last_error: None,
            restart_count: 0,
            updated_at: now_unix_seconds(),
            sequence: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeEvent {
    StatusChanged { snapshot: RuntimeSnapshot },
}

#[derive(Debug, Error, Clone)]
pub enum RuntimeError {
    #[error("route service is not active")]
    NotActive,
    #[error("runtime is blocked by an external Codex config modification")]
    ExternalModification,
    #[error("runtime restart limit reached")]
    RestartLimitReached,
    #[error("route lifecycle failed: {0}")]
    Lifecycle(String),
}

impl From<LifecycleError> for RuntimeError {
    fn from(error: LifecycleError) -> Self {
        match error {
            LifecycleError::ExternalModification => Self::ExternalModification,
            LifecycleError::NotActive => Self::NotActive,
            other => Self::Lifecycle(other.to_string()),
        }
    }
}

/// Owns the embedded route and keeps it alive independently of the renderer.
///
/// The supervisor is deliberately the only module that decides whether a
/// route should be running. Commands and tray actions call this interface;
/// they do not manipulate `EmbeddedRouteService` directly.
pub struct RouteSupervisor {
    route: Arc<Mutex<EmbeddedRouteService>>,
    snapshot: Arc<RwLock<RuntimeSnapshot>>,
    events: broadcast::Sender<RuntimeEvent>,
    desired_running: AtomicBool,
    restart_count: AtomicU32,
    sequence: AtomicU64,
    healthy_since: Mutex<Option<Instant>>,
    diagnostics: RwLock<Option<Arc<crate::diagnostics::DiagnosticsStore>>>,
    monitor: Mutex<Option<JoinHandle<()>>>,
}

impl RouteSupervisor {
    pub fn new(route: Arc<Mutex<EmbeddedRouteService>>) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            route,
            snapshot: Arc::new(RwLock::new(RuntimeSnapshot::default())),
            events,
            desired_running: AtomicBool::new(false),
            restart_count: AtomicU32::new(0),
            sequence: AtomicU64::new(0),
            healthy_since: Mutex::new(None),
            diagnostics: RwLock::new(None),
            monitor: Mutex::new(None),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    pub async fn attach_diagnostics(&self, diagnostics: Arc<crate::diagnostics::DiagnosticsStore>) {
        *self.diagnostics.write().await = Some(diagnostics);
    }

    pub async fn snapshot(&self) -> RuntimeSnapshot {
        self.snapshot.read().await.clone()
    }

    pub async fn mark_degraded(&self, message: impl Into<String>) {
        self.publish_phase(RuntimePhase::Degraded, Some(message.into()), false)
            .await;
    }

    pub async fn start_health_monitor(self: &Arc<Self>) {
        let mut monitor = self.monitor.lock().await;
        if monitor.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let supervisor = Arc::clone(self);
        *monitor = Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(HEALTH_INTERVAL);
            loop {
                ticker.tick().await;
                if !supervisor.desired_running.load(Ordering::SeqCst) {
                    break;
                }
                supervisor.reconcile().await;
            }
        }));
    }

    pub async fn ensure_running(
        self: &Arc<Self>,
        provider_id: Option<String>,
        port: Option<u16>,
    ) -> Result<RuntimeSnapshot, RuntimeError> {
        self.desired_running.store(true, Ordering::SeqCst);
        self.publish_phase(RuntimePhase::Starting, None, false)
            .await;

        let result = {
            let mut route = self.route.lock().await;
            let active = route.status().map_err(RuntimeError::from)?.active;
            if active {
                Ok(())
            } else {
                route
                    .activate_with(provider_id, port)
                    .await
                    .map(|_| ())
                    .map_err(RuntimeError::from)
            }
        };

        match result {
            Ok(()) => {
                self.restart_count.store(0, Ordering::SeqCst);
                *self.healthy_since.lock().await = Some(Instant::now());
                self.publish_from_route(RuntimePhase::Running, None).await;
                self.start_health_monitor().await;
                Ok(self.snapshot().await)
            }
            Err(error) => {
                self.desired_running.store(false, Ordering::SeqCst);
                let phase = error_phase(&error);
                self.publish_from_route(phase, Some(error.to_string()))
                    .await;
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        self.desired_running.store(false, Ordering::SeqCst);
        if let Some(task) = self.monitor.lock().await.take() {
            task.abort();
        }

        let result = {
            let mut route = self.route.lock().await;
            route.deactivate().await
        };
        match result {
            Ok(_) | Err(LifecycleError::NotActive) => {
                self.publish_from_route(RuntimePhase::Stopped, None).await;
                Ok(self.snapshot().await)
            }
            Err(error) => {
                let runtime_error = RuntimeError::from(error);
                let phase = if matches!(runtime_error, RuntimeError::ExternalModification) {
                    RuntimePhase::BlockedExternalModification
                } else {
                    RuntimePhase::Degraded
                };
                self.publish_from_route(phase, Some(runtime_error.to_string()))
                    .await;
                Err(runtime_error)
            }
        }
    }

    async fn reconcile(self: &Arc<Self>) {
        let status = {
            let route = self.route.lock().await;
            route.status()
        };
        let mut status = match status {
            Ok(status) => status,
            Err(error) => {
                self.publish_phase(RuntimePhase::Degraded, Some(error.to_string()), false)
                    .await;
                return;
            }
        };

        if status.active && !runtime_health_is_reachable(status.port).await {
            self.publish_from_status(
                RuntimePhase::Degraded,
                status.clone(),
                Some("Route health endpoint is unreachable".to_string()),
            )
            .await;
            self.record_diagnostic(
                crate::diagnostics::DiagnosticSeverity::Warning,
                "runtime.health_unreachable",
                "Route health endpoint is unreachable",
            )
            .await;
            let mut route = self.route.lock().await;
            match route.deactivate().await {
                Ok(_) | Err(LifecycleError::NotActive) => {
                    status.active = false;
                    status.pid = None;
                    status.server_reachable = false;
                    status.config_managed = false;
                }
                Err(LifecycleError::ExternalModification) => {
                    status.external_modification = true;
                    self.desired_running.store(false, Ordering::SeqCst);
                    self.publish_from_status(
                        RuntimePhase::BlockedExternalModification,
                        status,
                        Some("Codex config changed outside Codex Route".to_string()),
                    )
                    .await;
                    self.record_diagnostic(
                        crate::diagnostics::DiagnosticSeverity::Warning,
                        "runtime.external_modification",
                        "Codex config changed outside Codex Route",
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    self.publish_from_status(
                        RuntimePhase::Degraded,
                        status,
                        Some(error.to_string()),
                    )
                    .await;
                    self.record_diagnostic(
                        crate::diagnostics::DiagnosticSeverity::Error,
                        "runtime.health_recovery_failed",
                        error.to_string(),
                    )
                    .await;
                    return;
                }
            }
        }

        if status.external_modification {
            self.desired_running.store(false, Ordering::SeqCst);
            self.publish_from_status(
                RuntimePhase::BlockedExternalModification,
                status,
                Some("Codex config changed outside Codex Route".to_string()),
            )
            .await;
            self.record_diagnostic(
                crate::diagnostics::DiagnosticSeverity::Warning,
                "runtime.external_modification",
                "Codex config changed outside Codex Route",
            )
            .await;
            return;
        }
        if status.active {
            {
                let mut healthy_since = self.healthy_since.lock().await;
                match *healthy_since {
                    Some(started) if started.elapsed() >= HEALTHY_RESET_AFTER => {
                        self.restart_count.store(0, Ordering::SeqCst);
                        *healthy_since = Some(Instant::now());
                    }
                    None => *healthy_since = Some(Instant::now()),
                    _ => {}
                }
            }
            self.publish_from_status(RuntimePhase::Running, status, None)
                .await;
            return;
        }

        *self.healthy_since.lock().await = None;

        let next_count = self.restart_count.fetch_add(1, Ordering::SeqCst) + 1;
        if next_count > MAX_RESTARTS {
            self.desired_running.store(false, Ordering::SeqCst);
            self.publish_from_status(
                RuntimePhase::Failed,
                status,
                Some(RuntimeError::RestartLimitReached.to_string()),
            )
            .await;
            return;
        }

        self.publish_from_status(RuntimePhase::Recovering, status, None)
            .await;
        self.record_diagnostic(
            crate::diagnostics::DiagnosticSeverity::Warning,
            "runtime.recovering",
            format!("Route stopped unexpectedly; restart attempt {next_count}/{MAX_RESTARTS}"),
        )
        .await;
        tokio::time::sleep(restart_delay(next_count)).await;
        let result = {
            let mut route = self.route.lock().await;
            route
                .activate()
                .await
                .map(|_| ())
                .map_err(RuntimeError::from)
        };
        match result {
            Ok(()) => {
                *self.healthy_since.lock().await = Some(Instant::now());
                self.publish_from_route(RuntimePhase::Running, None).await;
            }
            Err(error) => {
                let phase = if matches!(error, RuntimeError::ExternalModification) {
                    self.desired_running.store(false, Ordering::SeqCst);
                    RuntimePhase::BlockedExternalModification
                } else {
                    RuntimePhase::Degraded
                };
                self.publish_from_route(phase, Some(error.to_string()))
                    .await;
                self.record_diagnostic(
                    crate::diagnostics::DiagnosticSeverity::Error,
                    "runtime.recovery_failed",
                    error.to_string(),
                )
                .await;
            }
        }
    }

    async fn record_diagnostic(
        &self,
        severity: crate::diagnostics::DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        if let Some(diagnostics) = self.diagnostics.read().await.clone() {
            diagnostics
                .record(severity, code, message, "runtime", BTreeMap::new())
                .await;
        }
    }

    async fn publish_phase(&self, phase: RuntimePhase, error: Option<String>, active: bool) {
        let current = self.snapshot.read().await.clone();
        self.publish(RuntimeSnapshot {
            phase,
            active,
            pid: if active { current.pid } else { None },
            port: current.port,
            server_reachable: active,
            config_managed: current.config_managed,
            external_modification: phase == RuntimePhase::BlockedExternalModification,
            last_error: error,
            restart_count: self.restart_count.load(Ordering::SeqCst),
            updated_at: now_unix_seconds(),
            sequence: 0,
        })
        .await;
    }

    async fn publish_from_route(&self, phase: RuntimePhase, error: Option<String>) {
        let status = {
            let route = self.route.lock().await;
            route.status()
        };
        match status {
            Ok(status) => self.publish_from_status(phase, status, error).await,
            Err(status_error) => {
                self.publish_phase(phase, Some(status_error.to_string()), false)
                    .await;
            }
        }
    }

    async fn publish_from_status(
        &self,
        phase: RuntimePhase,
        status: LifecycleStatus,
        error: Option<String>,
    ) {
        self.publish(RuntimeSnapshot {
            phase,
            active: status.active,
            pid: status.pid,
            port: status.port,
            server_reachable: status.server_reachable,
            config_managed: status.config_managed,
            external_modification: status.external_modification,
            last_error: error,
            restart_count: self.restart_count.load(Ordering::SeqCst),
            updated_at: now_unix_seconds(),
            sequence: 0,
        })
        .await;
    }

    async fn publish(&self, mut snapshot: RuntimeSnapshot) {
        let current = self.snapshot.read().await.clone();
        if same_snapshot(&current, &snapshot) {
            return;
        }
        snapshot.sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        *self.snapshot.write().await = snapshot.clone();
        let _ = self.events.send(RuntimeEvent::StatusChanged { snapshot });
    }
}

fn same_snapshot(left: &RuntimeSnapshot, right: &RuntimeSnapshot) -> bool {
    left.phase == right.phase
        && left.active == right.active
        && left.pid == right.pid
        && left.port == right.port
        && left.server_reachable == right.server_reachable
        && left.config_managed == right.config_managed
        && left.external_modification == right.external_modification
        && left.last_error == right.last_error
        && left.restart_count == right.restart_count
}

fn error_phase(error: &RuntimeError) -> RuntimePhase {
    if matches!(error, RuntimeError::ExternalModification) {
        return RuntimePhase::BlockedExternalModification;
    }
    if let RuntimeError::Lifecycle(message) = error {
        let message = message.to_ascii_lowercase();
        if message.contains("no current provider")
            || message.contains("provider configuration")
            || message.contains("provider not found")
        {
            return RuntimePhase::Degraded;
        }
    }
    RuntimePhase::Failed
}

fn restart_delay(attempt: u32) -> Duration {
    Duration::from_secs(1_u64 << attempt.saturating_sub(1).min(3))
}

async fn runtime_health_is_reachable(port: Option<u16>) -> bool {
    let Some(port) = port else { return false };
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(300))
        .timeout(Duration::from_millis(600))
        .build()
    else {
        return false;
    };
    client
        .get(format!("http://127.0.0.1:{port}/healthz"))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

impl Drop for RouteSupervisor {
    fn drop(&mut self) {
        if let Ok(mut monitor) = self.monitor.try_lock() {
            if let Some(task) = monitor.take() {
                task.abort();
            }
        }
    }
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{restart_delay, same_snapshot, RuntimePhase, RuntimeSnapshot};

    #[test]
    fn runtime_snapshot_starts_stopped_and_tracks_restart_count() {
        let snapshot = RuntimeSnapshot::default();
        assert_eq!(snapshot.phase, RuntimePhase::Stopped);
        assert!(!snapshot.active);
        assert_eq!(snapshot.restart_count, 0);
    }

    #[test]
    fn health_poll_does_not_publish_when_only_time_and_sequence_changed() {
        let first = RuntimeSnapshot {
            updated_at: 10,
            sequence: 1,
            ..RuntimeSnapshot::default()
        };
        let second = RuntimeSnapshot {
            updated_at: 20,
            sequence: 99,
            ..RuntimeSnapshot::default()
        };
        assert!(same_snapshot(&first, &second));
    }

    #[test]
    fn restart_backoff_caps_at_eight_seconds() {
        assert_eq!(restart_delay(1).as_secs(), 1);
        assert_eq!(restart_delay(2).as_secs(), 2);
        assert_eq!(restart_delay(3).as_secs(), 4);
        assert_eq!(restart_delay(4).as_secs(), 8);
        assert_eq!(restart_delay(9).as_secs(), 8);
    }
}
