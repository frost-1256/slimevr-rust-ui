//! Transient notifications and the rolling event log they also feed.

use std::time::{Duration, Instant};

use crate::ui::log::{LogEntry, LogLevel};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

impl From<Severity> for LogLevel {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Info | Severity::Success => Self::Info,
            Severity::Warning => Self::Warning,
            Severity::Error => Self::Error,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub severity: Severity,
    pub created: Instant,
}

/// Toasts currently on screen plus a capped log of every notice. `Info`
/// notices stay out of the toast stack (they would dominate it) but are kept
/// in the log.
#[derive(Default, Debug)]
pub struct ToastCenter {
    pub toasts: Vec<Toast>,
    log: Vec<LogEntry>,
}

impl ToastCenter {
    pub fn push(&mut self, severity: Severity, text: impl Into<String>) {
        let text = text.into();
        log::info!("{text}");
        self.log.push(LogEntry {
            level: severity.into(),
            text: format!("[{}] {text}", timestamp()),
        });
        let excess = self.log.len().saturating_sub(500);
        if excess > 0 {
            self.log.drain(..excess);
        }
        if severity != Severity::Info {
            self.toasts.push(Toast {
                text,
                severity,
                created: Instant::now(),
            });
        }
    }

    pub fn expire(&mut self) {
        self.toasts
            .retain(|toast| toast.created.elapsed() < Duration::from_secs(6));
    }

    pub fn dismiss(&mut self, index: usize) {
        if index < self.toasts.len() {
            self.toasts.remove(index);
        }
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", now.as_secs() % 100_000, now.subsec_millis())
}
