//! Shared log viewer: severity colouring, level filter, and copy support.
//!
//! Both the server output and the GUI event log are rendered through this
//! module so that they read the same way and can be copied with the mouse or
//! the "表示中をコピー" button.

use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use crate::theme;

/// Severity of a log line, ordered from least to most severe.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warning,
    Error,
}

impl LogLevel {
    /// Colour used for the line, or `None` to use the default text colour.
    pub fn color(self) -> Option<egui::Color32> {
        match self {
            Self::Debug => Some(theme::MUTED),
            Self::Info => None,
            Self::Warning => Some(theme::WARN),
            Self::Error => Some(theme::ERROR),
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "FINEST" | "FINER" | "FINE" | "CONFIG" | "DEBUG" | "TRACE" => Some(Self::Debug),
            "INFO" | "NOTICE" => Some(Self::Info),
            "WARNING" | "WARN" => Some(Self::Warning),
            "SEVERE" | "ERROR" | "FATAL" => Some(Self::Error),
            _ => None,
        }
    }
}

/// One log line with the severity parsed from its level token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub level: LogLevel,
    pub text: String,
}

impl LogEntry {
    /// Parses the level out of a line, keeping the text untouched so copying
    /// preserves exactly what the server or GUI printed.
    pub fn parse(text: impl Into<String>) -> Self {
        let text = text.into();
        let level = parse_level(&text);
        Self { level, text }
    }
}

/// Finds the first known `[LEVEL]` token in a line.
fn parse_level(line: &str) -> LogLevel {
    let stderr = line.starts_with("[err]");
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            break;
        };
        if let Some(level) = LogLevel::from_name(&after[..close]) {
            return level;
        }
        rest = &after[close + 1..];
    }
    if stderr {
        LogLevel::Warning
    } else {
        LogLevel::Info
    }
}

/// Which severities are shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LevelFilter {
    #[default]
    All,
    Warning,
    Error,
}

impl LevelFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "すべて",
            Self::Warning => "警告以上",
            Self::Error => "エラーのみ",
        }
    }

    pub fn matches(self, level: LogLevel) -> bool {
        match self {
            Self::All => true,
            Self::Warning => level >= LogLevel::Warning,
            Self::Error => level == LogLevel::Error,
        }
    }
}

/// Draws a filterable, selectable, copyable log view.
pub fn show(
    ui: &mut egui::Ui,
    id: &str,
    entries: &[LogEntry],
    filter: &mut LevelFilter,
    max_height: f32,
    empty: &str,
) {
    let visible: Vec<&LogEntry> = entries
        .iter()
        .filter(|entry| filter.matches(entry.level))
        .collect();

    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt((id, "filter"))
            .selected_text(filter.label())
            .width(100.0)
            .show_ui(ui, |ui| {
                for option in [LevelFilter::All, LevelFilter::Warning, LevelFilter::Error] {
                    ui.selectable_value(filter, option, option.label());
                }
            });
        ui.label(
            RichText::new(format!("{} / {} 行", visible.len(), entries.len()))
                .small()
                .color(theme::MUTED),
        );

        let copied_at = ui.make_persistent_id((id, "copied"));
        if ui
            .add(theme::text_button("表示中をコピー"))
            .on_hover_text(
                "表示中の行をすべてクリップボードへ。行をドラッグ選択して Ctrl+C も使えます",
            )
            .clicked()
        {
            let text = visible
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(text);
            ui.data_mut(|data| data.insert_temp(copied_at, Instant::now()));
        }
        let just_copied = ui
            .data(|data| data.get_temp::<Instant>(copied_at))
            .is_some_and(|at| at.elapsed() < Duration::from_millis(1500));
        if just_copied {
            ui.label(RichText::new("コピーしました").small().color(theme::OK));
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    });
    ui.add_space(4.0);

    if visible.is_empty() {
        ui.label(RichText::new(empty).small().color(theme::MUTED));
        return;
    }

    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    egui::ScrollArea::both()
        .id_salt(id)
        .max_height(max_height)
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show_rows(ui, row_height, visible.len(), |ui, range| {
            for entry in &visible[range] {
                let mut text = RichText::new(&entry.text).monospace();
                if let Some(color) = entry.level.color() {
                    text = text.color(color);
                }
                ui.add(
                    egui::Label::new(text)
                        .selectable(true)
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_util_logging_levels() {
        assert_eq!(
            LogEntry::parse("[out] 12:00:00 [SEVERE] boom").level,
            LogLevel::Error
        );
        assert_eq!(
            LogEntry::parse("[out] 12:00:00 [WARNING] careful").level,
            LogLevel::Warning
        );
        assert_eq!(
            LogEntry::parse("[out] 12:00:00 [INFO] hello").level,
            LogLevel::Info
        );
        assert_eq!(
            LogEntry::parse("[out] 12:00:00 [FINE] detail").level,
            LogLevel::Debug
        );
    }

    #[test]
    fn keeps_unrecognised_lines_as_info() {
        let entry = LogEntry::parse("[out] plain text");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.text, "[out] plain text");
    }

    #[test]
    fn treats_stderr_without_level_as_warning() {
        assert_eq!(
            LogEntry::parse("[err] java.lang.IllegalStateException").level,
            LogLevel::Warning
        );
    }

    #[test]
    fn filters_by_severity() {
        assert!(LevelFilter::All.matches(LogLevel::Debug));
        assert!(!LevelFilter::Warning.matches(LogLevel::Info));
        assert!(LevelFilter::Warning.matches(LogLevel::Warning));
        assert!(!LevelFilter::Error.matches(LogLevel::Warning));
        assert!(LevelFilter::Error.matches(LogLevel::Error));
    }

    #[test]
    fn text_is_preserved_for_copying() {
        let raw = "[out] 12:00:00 [SEVERE] failed to bind";
        assert_eq!(LogEntry::parse(raw).text, raw);
    }
}
