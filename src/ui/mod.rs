pub mod checklist;
pub mod console;
pub mod dashboard;
pub mod proportions;
pub mod reset;
pub mod server;
pub mod settings;
pub mod trackers;

use eframe::egui::{self, RichText};

use crate::app::{Severity, SlimeVrApp};
use crate::model::TrackerStatus;
use crate::net::ConnState;
use crate::theme;

pub const CARD_MIN_WIDTH: f32 = 380.0;

/// Top bar: identity, connection state, and the actions that are useful from
/// anywhere in the UI.
pub fn header(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("SlimeVR Native")
                .color(theme::ACCENT)
                .strong()
                .size(18.0),
        );
        ui.separator();

        let (text, color) = match app.conn.state() {
            ConnState::Connected => ("接続済み", theme::OK),
            ConnState::Connecting => ("接続中…", theme::WARN),
            ConnState::Disconnected => ("未接続", theme::ERROR),
        };
        ui.label(RichText::new("●").color(color));
        ui.label(text);
        if let Some(error) = &app.conn.last_error {
            ui.label(RichText::new(error).color(theme::ERROR).small());
        }
        ui.label(RichText::new(app.conn.url()).color(theme::MUTED).small());
        ui.label(RichText::new(app.page.label()).color(theme::ACCENT).small());

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("ヘルプ")
                .on_hover_text("キー操作と既知の制限")
                .clicked()
            {
                app.show_help = !app.show_help;
            }
            if ui.button("再接続").clicked() {
                app.conn.reconnect();
            }
            let (label, next) = if app.paused {
                ("トラッキング再開", false)
            } else {
                ("トラッキング一時停止", true)
            };
            if ui
                .button(label)
                .on_hover_text("サーバー全体のトラッキングを一時停止します")
                .clicked()
            {
                app.set_paused(next);
            }
        });
    });

    if !app.toasts.is_empty() {
        ui.add_space(2.0);
        let mut dismiss = None;
        ui.horizontal_wrapped(|ui| {
            for (index, toast) in app.toasts.iter().enumerate() {
                let color = match toast.severity {
                    Severity::Info => theme::MUTED,
                    Severity::Success => theme::OK,
                    Severity::Warning => theme::WARN,
                    Severity::Error => theme::ERROR,
                };
                if ui
                    .label(RichText::new(&toast.text).color(color).small())
                    .on_hover_text("クリックで閉じる")
                    .clicked()
                {
                    dismiss = Some(index);
                }
                ui.label(RichText::new("|").color(theme::BORDER).small());
            }
        });
        if let Some(index) = dismiss {
            app.toasts.remove(index);
        }
    }

    if app.show_help {
        ui.add_space(4.0);
        theme::card(ui, |ui| {
            ui.label(RichText::new("ヘルプ").strong());
            ui.label("・データは 100ms 間隔のデータフィードで取得しています。");
            ui.label("・設定はカードごとに「適用」を押したときだけサーバーへ送信します。");
            ui.label("・ファームウェア書き込みは URL 指定のみ対応 (リリースの自動取得は未移植)。");
            ui.label("・キー操作: Ctrl+R で設定再取得。");
        });
        if ui.input(|input| input.key_pressed(egui::Key::R) && input.modifiers.command) {
            app.request_settings();
        }
    }
}

pub fn nav(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    for (page, label) in crate::app::Page::ALL {
        let selected = app.page == page;
        let mut text = RichText::new(label);
        if selected {
            text = text.color(theme::ACCENT).strong();
        }
        let mut button = egui::Button::new(text).min_size(egui::vec2(150.0, 30.0));
        if selected {
            button = button.fill(theme::SURFACE_ALT);
        }
        if ui.add(button).clicked() {
            app.page = page;
        }
        if page == crate::app::Page::Checklist {
            let remaining = app.checklist.remaining();
            if remaining > 0 {
                ui.label(
                    RichText::new(format!("  {remaining} 件未完了"))
                        .color(theme::WARN)
                        .small(),
                );
            }
        }
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("UI スケール {:.2}x", app.ui_scale))
                .color(theme::MUTED)
                .small(),
        );
        ui.add(egui::Slider::new(&mut app.ui_scale, 0.75..=2.0).show_value(false));
    });
}

pub fn footer(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{:.0} FPS  受信 {} フレーム  送信 {}  破棄 {}",
                app.fps(),
                app.conn.received_frames,
                app.conn.sent_frames,
                app.conn.dropped_frames
            ))
            .color(theme::MUTED)
            .small(),
        );
        ui.separator();
        ui.label(
            RichText::new(format!(
                "トラッカー {}/{} 接続, {} 割当",
                app.connected_tracker_count(),
                app.all_trackers().len(),
                app.assigned_tracker_count()
            ))
            .color(theme::MUTED)
            .small(),
        );
        if let Some(ip) = &app.local_ip {
            ui.separator();
            ui.label(
                RichText::new(format!("サーバー {ip}"))
                    .color(theme::MUTED)
                    .small(),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(note) = &app.conn.last_event_note {
                ui.label(RichText::new(note).color(theme::MUTED).small());
            }
        });
    });
}

/// Two-column responsive layout helper: returns `true` when the second column
/// fits next to the first.
pub fn two_columns(ui: &egui::Ui) -> bool {
    ui.available_width() >= CARD_MIN_WIDTH * 2.0 + 24.0
}

pub fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong().size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("");
            });
        });
        ui.add_space(4.0);
        add(ui)
    })
}

pub fn status_color(status: TrackerStatus) -> egui::Color32 {
    match status {
        TrackerStatus::Ok => theme::OK,
        TrackerStatus::Busy => theme::ACCENT,
        TrackerStatus::Occluded | TrackerStatus::TimedOut => theme::WARN,
        TrackerStatus::Disconnected | TrackerStatus::Error | TrackerStatus::Unknown(_) => {
            theme::ERROR
        }
        TrackerStatus::None => theme::MUTED,
    }
}

pub fn placeholder(ui: &mut egui::Ui, reason: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(reason).color(theme::MUTED));
    });
}
