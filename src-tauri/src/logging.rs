use crate::diagnostics::{DiagnosticRecord, DiagnosticSeverity};
use crate::state::AppState;
use std::collections::BTreeMap;

/// Emit a structured diagnostic and mirror it to the native logger. Secrets are
/// redacted by `DiagnosticsStore` before the record is retained or emitted.
pub async fn record(
    state: &AppState,
    severity: DiagnosticSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    source: impl Into<String>,
    context: BTreeMap<String, String>,
    secrets: &[String],
) -> DiagnosticRecord {
    let code = code.into();
    let message = message.into();
    let record = if secrets.is_empty() {
        state
            .diagnostics
            .record(severity, code.clone(), message, source, context)
            .await
    } else {
        state
            .diagnostics
            .record_with_secrets(severity, code.clone(), message, source, context, secrets)
            .await
    };
    match severity {
        DiagnosticSeverity::Info => {
            log::info!(target: "codex-route", "[{code}] {}", record.message)
        }
        DiagnosticSeverity::Warning => {
            log::warn!(target: "codex-route", "[{code}] {}", record.message)
        }
        DiagnosticSeverity::Error => {
            log::error!(target: "codex-route", "[{code}] {}", record.message)
        }
    }
    record
}
