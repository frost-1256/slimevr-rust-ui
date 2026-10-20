pub mod checklist;
pub mod console;
pub mod dashboard;
pub mod log;
pub mod proportions;
pub mod reset;
pub mod server;
pub mod settings;
pub mod trackers;

use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::TrackerStatus;
use crate::net::ConnState;
use crate::theme;
use crate::toast::Severity;

pub const CARD_MIN_WIDTH: f32 = 380.0;

/// Top bar: identity, connection state, and the actions that are useful from
/// anywhere in the UI.
pub fn header(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("SlimeVR")
                .color(theme::ACCENT)
                .strong()
                .size(20.0),
        );
        ui.label(RichText::new("Native UI").color(theme::MUTED).size(13.0));
        ui.add_space(8.0);

        let (text, color) = match app.conn.state() {
            ConnState::Connected => ("接続済み", theme::OK),
            ConnState::Connecting => ("接続中…", theme::WARN),
            ConnState::Disconnected => ("未接続", theme::ERROR),
        };
        status_badge(ui, text, color);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(theme::text_button("ヘルプ"))
                .on_hover_text("キー操作と既知の制限")
                .clicked()
            {
                app.show_help = !app.show_help;
            }
            if ui.add(theme::outlined_button("再接続")).clicked() {
                app.conn.reconnect();
            }
            let (label, next) = if app.paused {
                ("トラッキング再開", false)
            } else {
                ("トラッキング一時停止", true)
            };
            let button = if app.paused {
                theme::primary_button(label)
            } else {
                theme::outlined_button(label)
            };
            if ui
                .add(button)
                .on_hover_text("サーバー全体のトラッキングを一時停止します")
                .clicked()
            {
                app.set_paused(next);
            }
        });
    });

    // Second line keeps the (often long) target URL and the last socket error
    // out of the action row, where they used to overlap the buttons.
    // Always render the URL row: hiding it while connected made the whole
    // page jump by ~25px every time the socket came up or dropped.
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        ui.label(
            RichText::new(app.conn.url())
                .color(theme::MUTED)
                .small()
                .monospace(),
        );
        if let Some(error) = &app.conn.last_error {
            ui.add(egui::Label::new(RichText::new(error).color(theme::ERROR).small()).truncate());
        }
    });

    if !app.notices.toasts.is_empty() {
        ui.add_space(2.0);
        let mut dismiss = None;
        ui.horizontal_wrapped(|ui| {
            for (index, toast) in app.notices.toasts.iter().enumerate() {
                let color = match toast.severity {
                    Severity::Info => theme::TEXT,
                    Severity::Success => theme::OK,
                    Severity::Warning => theme::WARN,
                    Severity::Error => theme::ERROR,
                };
                let response = egui::Frame::new()
                    .fill(theme::TOAST_INFO)
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&toast.text).color(color).small());
                    })
                    .response;
                if response
                    .interact(egui::Sense::click())
                    .on_hover_text("クリックで閉じる")
                    .clicked()
                {
                    dismiss = Some(index);
                }
            }
        });
        if let Some(index) = dismiss {
            app.notices.dismiss(index);
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

/// Small pill used for connection/severity state in the header.
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = galley.size() + egui::vec2(20.0, 8.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, size.y / 2.0, color.gamma_multiply(0.16));
    ui.painter()
        .galley(rect.min + egui::vec2(10.0, 4.0), galley, color);
}

/// Start/stop button shared by the dashboard and the server page. Requests are
/// queued to the supervisor thread, so this never blocks the frame.
pub fn server_controls(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    use crate::server::State;

    let state = app.server.state();
    let busy = matches!(state, State::Starting | State::Stopping);
    let running = matches!(state, State::Running(_));

    if running {
        let button = ui.add_enabled(
            !busy,
            theme::outlined_button("サーバーを停止").text_color(theme::WARN),
        );
        if button.clicked() {
            app.stop_server();
        }
    } else {
        let button = ui.add_enabled(!busy, theme::primary_button("サーバーを起動"));
        if button.clicked() {
            app.start_server();
        }
    }

    if busy {
        ui.spinner();
    }
}

/// Compact server summary used on the dashboard: state, PID, controls and a
/// link to the full server page.
pub fn server_summary(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui.horizontal(|ui| {
        server_status_line(ui, app);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme::text_button("サーバー設定へ")).clicked() {
                app.page = crate::page::Page::Server;
            }
        });
    });

    ui.horizontal(|ui| {
        server_controls(ui, app);
        if !app.server.is_running()
            && (app.server.jar.is_none() || app.server.java.is_none())
            && ui.add(theme::text_button("jar / Java を入手")).clicked()
        {
            app.page = crate::page::Page::Server;
        }
    });
}

/// Badge plus PID/error text, shared by both server surfaces.
pub fn server_status_line(ui: &mut egui::Ui, app: &SlimeVrApp) {
    use crate::server::State;

    let (label, color) = match app.server.state() {
        State::Running(_) => ("実行中", theme::OK),
        State::Starting => ("起動中…", theme::WARN),
        State::Stopping => ("停止処理中…", theme::WARN),
        State::Stopped => ("停止中", theme::MUTED),
    };
    status_badge(ui, label, color);
    let status = app.server.status_text();
    if !status.is_empty() {
        ui.label(RichText::new(status).small().color(theme::MUTED));
    }
}

pub fn nav(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new("ナビゲーション")
                .color(theme::MUTED)
                .size(12.0),
        );
    });
    ui.add_space(2.0);

    for (page, label) in crate::page::Page::ALL {
        let selected = app.page == page;
        let badge = (page == crate::page::Page::Checklist)
            .then(|| app.checklist.remaining())
            .filter(|remaining| *remaining > 0);
        if nav_item(ui, page.icon(), label, selected, badge) {
            app.page = page;
        }
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("UI スケール {:.2}x", app.ui_scale))
                .color(theme::MUTED)
                .small(),
        );
        ui.add(egui::Slider::new(&mut app.ui_scale, 0.75..=2.0).show_value(false));
    });
}

/// One Material 3 navigation-drawer row: pill highlight, leading icon, label.
fn nav_item(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    selected: bool,
    badge: Option<usize>,
) -> bool {
    let height = 46.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if !ui.is_rect_visible(rect) {
        return false;
    }

    let bg = if selected {
        theme::ACCENT_DIM
    } else if response.hovered() {
        theme::SURFACE_ALT
    } else {
        egui::Color32::TRANSPARENT
    };
    if bg != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 23.0, bg);
    }
    let fg = if selected { theme::TEXT } else { theme::MUTED };

    let painter = ui.painter();
    if let Some(glyph) = egui_material3::material_symbol::get_material_symbol(icon) {
        painter.text(
            rect.left_center() + egui::vec2(22.0, 0.0),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(22.0),
            if selected { theme::ACCENT } else { fg },
        );
    }
    painter.text(
        rect.left_center() + egui::vec2(44.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(15.0),
        if selected { theme::ACCENT } else { theme::TEXT },
    );
    if let Some(remaining) = badge {
        painter.text(
            rect.right_center() - egui::vec2(16.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            remaining.to_string(),
            egui::FontId::proportional(13.0),
            theme::WARN,
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
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
        ui.label(RichText::new(title).strong().size(16.0));
        ui.add_space(8.0);
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
