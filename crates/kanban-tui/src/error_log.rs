use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone)]
pub enum LogLevel {
    Error,
    Warn,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub target: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

#[derive(Debug, Default)]
pub struct ErrorLogState {
    pub entries: VecDeque<LogEntry>,
    pub has_unread_errors: bool,
    pub unread_count: usize,
    pub error_count: usize,
}

impl ErrorLogState {
    pub fn push(&mut self, message: String, target: String, level: LogLevel) {
        let is_error = matches!(level, LogLevel::Error);
        self.entries.push_back(LogEntry {
            level,
            message,
            target,
            timestamp: chrono::Local::now(),
        });
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.has_unread_errors = true;
        self.unread_count += 1;
        if is_error {
            self.error_count += 1;
        }
    }

    pub fn clear_unread(&mut self) {
        self.has_unread_errors = false;
        self.unread_count = 0;
    }

    pub fn to_clipboard_text(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                let label = match entry.level {
                    LogLevel::Error => "ERROR",
                    LogLevel::Warn => "WARN",
                };
                format!(
                    "[{}] {} {} {}",
                    label,
                    entry.timestamp.format("%H:%M:%S"),
                    entry.target,
                    entry.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct InMemoryLogLayer {
    pub state: Arc<Mutex<ErrorLogState>>,
}

impl InMemoryLogLayer {
    pub fn new(state: Arc<Mutex<ErrorLogState>>) -> Self {
        Self { state }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for InMemoryLogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = event.metadata().level();
        if *level > tracing::Level::WARN {
            return;
        }
        let log_level = if *level == tracing::Level::ERROR {
            LogLevel::Error
        } else {
            LogLevel::Warn
        };
        let target = event.metadata().target().to_string();
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        if let Ok(mut state) = self.state.lock() {
            state.push(visitor.0, target, log_level);
        }
    }
}

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let formatted = format!("{value:?}");
            self.0 = formatted
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(&formatted)
                .to_string();
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_clipboard_text_with_no_entries_is_empty() {
        let log = ErrorLogState::default();
        assert_eq!(log.to_clipboard_text(), "");
    }

    #[test]
    fn test_to_clipboard_text_formats_entries_oldest_first() {
        let mut log = ErrorLogState::default();
        log.push("first".to_string(), "app::a".to_string(), LogLevel::Warn);
        log.push("second".to_string(), "app::b".to_string(), LogLevel::Error);

        let text = log.to_clipboard_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("[WARN]"));
        assert!(lines[0].contains("app::a"));
        assert!(lines[0].contains("first"));
        assert!(lines[1].starts_with("[ERROR]"));
        assert!(lines[1].contains("app::b"));
        assert!(lines[1].contains("second"));
    }
}
