use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::{ResetType, TrackerStatus, body_part_label};
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        // The most common first action: bring the server up. Keep it at the
        // very top instead of hiding it behind the server page.
        ui::section(ui, "サーバー", |ui| {
            ui::server_summary(ui, app);
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "jar: {}   java: {}",
                    app.server.jar_display(),
                    app.server.java_display()
                ))
                .monospace()
                .small()
                .color(theme::MUTED),
            );
        });
        ui.add_space(12.0);

        if !crate::app::connected(app) {
            theme::card(ui, |ui| {
                ui.label(
                    RichText::new("サーバーに接続していません")
                        .color(theme::WARN)
                        .strong(),
                );
                ui.label(format!("接続先: {}", app.conn.url()));
                ui.label(
                    "SlimeVR サーバーを起動するか、「サーバー」ページで jar を指定してください。",
                );
                ui.horizontal(|ui| {
                    if ui.add(theme::primary_button("再接続")).clicked() {
                        app.conn.reconnect();
                    }
                    if ui.add(theme::text_button("設定再取得")).clicked() {
                        app.request_settings();
                    }
                });
            });
            ui.add_space(8.0);
        }

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                summary(&mut columns[0], app);
                resets(&mut columns[1], app);
            });
        } else {
            summary(ui, app);
            ui.add_space(8.0);
            resets(ui, app);
        }

        ui.add_space(8.0);
        tracker_overview(app, ui);
        ui.add_space(8.0);
        events(app, ui);
    });
}

fn summary(ui: &mut egui::Ui, app: &SlimeVrApp) {
    let total = app.all_trackers().len();
    let connected = app.connected_tracker_count();
    let assigned = app.assigned_tracker_count();
    let battery = app.average_battery();

    ui::section(ui, "概要", |ui| {
        egui::Grid::new("dashboard-summary")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("トラッカー").color(theme::MUTED));
                ui.label(format!("{connected} / {total} 接続中"));
                ui.end_row();

                ui.label(RichText::new("割り当て済み").color(theme::MUTED));
                ui.label(format!("{assigned} 件"));
                ui.end_row();

                ui.label(RichText::new("平均バッテリー").color(theme::MUTED));
                match battery {
                    Some(percent) => ui.label(
                        RichText::new(format!("{percent:.0} %"))
                            .color(theme::battery_color(percent as u8)),
                    ),
                    None => ui.label(RichText::new("取得待ち").color(theme::MUTED)),
                };
                ui.end_row();

                ui.label(RichText::new("デバイス").color(theme::MUTED));
                ui.label(format!("{} 台", app.devices.len()));
                ui.end_row();

                ui.label(RichText::new("トラッキング").color(theme::MUTED));
                if app.paused {
                    ui.label(RichText::new("一時停止中").color(theme::WARN));
                } else {
                    ui.label(RichText::new("動作中").color(theme::OK));
                }
                ui.end_row();

                ui.label(RichText::new("チェックリスト").color(theme::MUTED));
                let remaining = app.checklist.remaining();
                if remaining == 0 {
                    ui.label(RichText::new("完了").color(theme::OK));
                } else {
                    ui.label(RichText::new(format!("{remaining} 件未完了")).color(theme::WARN));
                }
                ui.end_row();

                ui.label(RichText::new("骨格").color(theme::MUTED));
                ui.label(format!(
                    "{} ボーン, 身長 {:.2} m",
                    app.skeleton.bones.len(),
                    app.skeleton.user_height
                ));
                ui.end_row();
            });
    });
}

fn resets(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "リセット", |ui| {
        let active = app.active_reset().cloned();
        ui.horizontal_wrapped(|ui| {
            let yaw = ui.add_enabled(
                app.guards.can_do_yaw_reset,
                theme::outlined_button("Yaw リセット"),
            );
            if yaw.clicked() {
                app.start_reset(ResetType::Yaw, &[]);
            }
            yaw.on_hover_text("向きだけをリセットします");

            if ui.add(theme::primary_button("フルリセット")).clicked() {
                app.start_reset(ResetType::Full, &[]);
            }

            let mounting = ui.add_enabled(
                app.guards.can_do_mounting,
                theme::outlined_button("マウンティング リセット"),
            );
            if mounting.clicked() {
                app.start_reset(ResetType::Mounting, &[]);
            }
            mounting.on_hover_text("装着方向を初期状態に戻します (HMD と両足が必要)");
        });

        ui.add_space(4.0);
        match active {
            Some(reset) => {
                let total = reset.duration_ms.max(1) as f32;
                let fraction = (reset.progress_ms as f32 / total).clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .text(format!(
                            "{}: {:.0}%",
                            reset.reset_type.map(ResetType::label).unwrap_or("リセット"),
                            fraction * 100.0
                        ))
                        .desired_width(260.0),
                );
            }
            None => {
                ui.label(RichText::new("実行中のリセットはありません").color(theme::MUTED));
            }
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut feet = app
                .settings
                .resets
                .as_ref()
                .map(|resets| resets.reset_mounting_feet)
                .unwrap_or(false);
            if theme::switch(ui, &mut feet, "マウンティング リセット時に足もリセット").changed()
                && let Some(resets) = app.settings.resets.clone()
            {
                let mut update = resets;
                update.reset_mounting_feet = feet;
                let frame = crate::proto::set_resets(&update);
                app.send_frame(frame, "リセット設定");
            }
        });
    });
}

fn tracker_overview(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui::section(ui, "トラッカー", |ui| {
        let trackers: Vec<(
            crate::model::TrackerKey,
            String,
            u8,
            TrackerStatus,
            Option<u16>,
        )> = app
            .all_trackers()
            .iter()
            .map(|tracker| {
                (
                    tracker.key,
                    tracker.name(),
                    tracker.body_part,
                    tracker.status,
                    tracker.tps,
                )
            })
            .collect();

        if trackers.is_empty() {
            ui.label(RichText::new("トラッカーが検出されていません").color(theme::MUTED));
            return;
        }

        egui::Grid::new("dashboard-trackers")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("名前").color(theme::MUTED));
                ui.label(RichText::new("役割").color(theme::MUTED));
                ui.label(RichText::new("状態").color(theme::MUTED));
                ui.label(RichText::new("TPS").color(theme::MUTED));
                ui.end_row();

                for (key, name, body_part, status, tps) in trackers {
                    if ui.selectable_label(false, name).clicked() {
                        app.selected = Some(key);
                        app.page = crate::app::Page::Trackers;
                    }
                    ui.label(if body_part == 0 {
                        RichText::new("未割当").color(theme::MUTED)
                    } else {
                        RichText::new(body_part_label(body_part))
                    });
                    ui.label(RichText::new(status.label()).color(ui::status_color(status)));
                    ui.label(match tps {
                        Some(tps) => tps.to_string(),
                        None => "-".to_string(),
                    });
                    ui.end_row();
                }
            });
    });
}

fn events(app: &SlimeVrApp, ui: &mut egui::Ui) {
    ui::section(ui, "イベントログ", |ui| {
        if app.event_log.is_empty() {
            ui.label(RichText::new("まだイベントはありません").color(theme::MUTED));
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &app.event_log {
                    ui.label(RichText::new(line).small().monospace());
                }
            });
    });
}
