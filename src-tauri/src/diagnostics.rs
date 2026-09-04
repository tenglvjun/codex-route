use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{broadcast, Mutex};

const MAX_RECORDS: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRecord {
    pub id: u64,
    pub timestamp: i64,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source: String,
    pub context: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct DiagnosticsStore {
    records: Mutex<VecDeque<DiagnosticRecord>>,
    events: broadcast::Sender<DiagnosticRecord>,
    next_id: AtomicU64,
}

impl DiagnosticsStore {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            records: Mutex::new(VecDeque::with_capacity(MAX_RECORDS)),
            events,
            next_id: AtomicU64::new(1),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DiagnosticRecord> {
        self.events.subscribe()
    }

    pub async fn record(
        &self,
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
        context: BTreeMap<String, String>,
    ) -> DiagnosticRecord {
        self.record_with_secrets(severity, code, message, source, context, &[])
            .await
    }

    pub async fn record_with_secrets(
        &self,
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
        context: BTreeMap<String, String>,
        secrets: &[String],
    ) -> DiagnosticRecord {
        let record = DiagnosticRecord {
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            timestamp: now_unix_seconds(),
            severity,
            code: code.into(),
            message: redact(&message.into(), secrets),
            source: source.into(),
            context: context
                .into_iter()
                .map(|(key, value)| (key, redact(&value, secrets)))
                .collect(),
        };
        let mut records = self.records.lock().await;
        records.push_front(record.clone());
        records.truncate(MAX_RECORDS);
        drop(records);
        let _ = self.events.send(record.clone());
        record
    }

    pub async fn recent(&self, limit: usize) -> Vec<DiagnosticRecord> {
        let mut records = self
            .records
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| right.id.cmp(&left.id))
        });
        records.truncate(limit.min(MAX_RECORDS));
        records
    }

    pub async fn clear(&self) {
        self.records.lock().await.clear();
    }
}

impl Default for DiagnosticsStore {
    fn default() -> Self {
        Self::new()
    }
}

fn redact(value: &str, secrets: &[String]) -> String {
    let mut redacted = value.to_string();
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        redacted = redacted.replace(secret, "[REDACTED]");
    }
    let mut output = Vec::with_capacity(redacted.len());
    let mut parts = redacted.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("bearer") {
            output.push("Bearer [REDACTED]".to_string());
            let _ = parts.next();
        } else if looks_like_api_key(part) {
            output.push("[REDACTED]".to_string());
        } else {
            output.push(redact_query_secrets(part));
        }
    }
    output.join(" ")
}

fn redact_query_secrets(value: &str) -> String {
    const SECRET_KEYS: [&str; 5] = ["api_key", "api-key", "key", "token", "access_token"];
    let mut redacted = value.to_string();
    for key in SECRET_KEYS {
        let marker = format!("{key}=");
        let mut search_from = 0;
        while let Some(offset) = redacted[search_from..].find(&marker) {
            let value_start = search_from + offset + marker.len();
            let value_end = redacted[value_start..]
                .find(|character| ['&', '#', ' '].contains(&character))
                .map(|end| value_start + end)
                .unwrap_or(redacted.len());
            redacted.replace_range(value_start..value_end, "[REDACTED]");
            search_from = value_start + "[REDACTED]".len();
        }
    }
    redacted
}

fn looks_like_api_key(value: &str) -> bool {
    let trimmed = value.trim_matches(|character: char| {
        matches!(character, ',' | ';' | ')' | ']' | '}' | '"' | '\'')
    });
    (trimmed.starts_with("sk-") || trimmed.starts_with("rk-")) && trimmed.len() >= 12
}

fn now_unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticSeverity, DiagnosticsStore};
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn redacts_bearer_and_known_secrets() {
        let store = DiagnosticsStore::new();
        let record = store
            .record_with_secrets(
                DiagnosticSeverity::Error,
                "upstream",
                "Authorization Bearer sk-secret",
                "route",
                BTreeMap::from([(
                    "url".to_string(),
                    "https://example.test?key=sk-secret".to_string(),
                )]),
                &["sk-secret".to_string()],
            )
            .await;
        assert!(!record.message.contains("sk-secret"));
        assert!(!record.context["url"].contains("sk-secret"));
        assert!(record.message.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn redacts_short_api_key_shapes_and_query_parameters() {
        let store = DiagnosticsStore::new();
        let record = store
            .record(
                DiagnosticSeverity::Error,
                "upstream",
                "request failed sk-1234567890 https://example.test?api_key=visible",
                "route",
                BTreeMap::new(),
            )
            .await;
        assert!(!record.message.contains("sk-1234567890"));
        assert!(!record.message.contains("api_key=visible"));
    }

    #[tokio::test]
    async fn keeps_only_the_newest_two_hundred_records() {
        let store = DiagnosticsStore::new();
        for index in 0..205 {
            store
                .record(
                    DiagnosticSeverity::Info,
                    "test",
                    index.to_string(),
                    "test",
                    BTreeMap::new(),
                )
                .await;
        }
        let records = store.recent(500).await;
        assert_eq!(records.len(), 200);
        assert_eq!(
            records.first().map(|record| record.message.as_str()),
            Some("204")
        );
    }
}
