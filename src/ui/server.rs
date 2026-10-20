use eframe::egui::{self, RichText};

use crate::app::{SlimeVrApp, port_status_text};
use crate::config;
use crate::net::ConnState;
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("サーバー");
        ui.label(
            RichText::new("SlimeVR サーバー (Java) の起動と、GUI の接続先を管理します。")
                .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                connection_card(&mut columns[0], app);
                steamvr_card(&mut columns[0], app);
                process_card(&mut columns[0], app);
                client_log_card(&mut columns[1], app);
            });
        } else {
            connection_card(ui, app);
            ui.add_space(8.0);
            steamvr_card(ui, app);
            ui.add_space(8.0);
            process_card(ui, app);
            ui.add_space(8.0);
            client_log_card(ui, app);
        }
    });
}

fn connection_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "接続", |ui| {
        ui.horizontal(|ui| {
            let (label, color) = match app.conn.state() {
                ConnState::Connected => ("接続済み", theme::OK),
                ConnState::Connecting => ("接続中", theme::WARN),
                ConnState::Disconnected => ("未接続", theme::ERROR),
            };
            ui.label(RichText::new("●").color(color));
            ui.label(RichText::new(label).color(color));
            ui.label(
                RichText::new(format!(
                    "受信 {} / 送信 {} / 破棄 {}",
                    app.conn.received_frames, app.conn.sent_frames, app.conn.dropped_frames
                ))
                .small()
                .color(theme::MUTED),
            );
        });
        if let Some(error) = &app.conn.last_error {
            ui.label(RichText::new(error).small().color(theme::ERROR));
        }

        ui.horizontal(|ui| {
            ui.label("WebSocket URL");
            ui.add(
                egui::TextEdit::singleline(&mut app.config.server_url)
                    .desired_width(240.0)
                    .hint_text("ws://127.0.0.1:21110"),
            );
            if ui.button("接続先を適用").clicked() {
                let url = app.config.server_url.trim().to_string();
                app.conn.set_url(url);
                app.save_config();
            }
        });
        ui.horizontal(|ui| {
            ui.label("サーバーポート 21110:");
            let free = port_status_text();
            ui.label(RichText::new(&free).color(if free == "空き" {
                theme::MUTED
            } else {
                theme::OK
            }));
            if ui.button("確認").clicked() {
                ui.ctx().request_repaint();
            }
        });
        if let Some(ip) = &app.local_ip {
            ui.label(
                RichText::new(format!("サーバーが報告した IP: {ip}"))
                    .small()
                    .color(theme::MUTED),
            );
        }
        ui.label(
            RichText::new(format!("最終受信から {:.1} 秒", app.conn.silence_seconds()))
                .small()
                .color(theme::MUTED),
        );

        ui.separator();
        let mut include_bones = app.conn.include_bones;
        if ui
            .checkbox(&mut include_bones, "データフィードで骨格 (bones) も購読")
            .on_hover_text("トラッカーの算出骨格長を取得します。再購読が必要です。")
            .changed()
        {
            app.conn.include_bones = include_bones;
            app.config.include_bones = include_bones;
            app.conn.restart_data_feed();
            app.save_config();
        }
        if ui.button("データフィードを再送").clicked() {
            app.conn.restart_data_feed();
        }
    });
}

/// SteamVR 連携 (ドライバ有効化とインストール状態)。
fn steamvr_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    ui::section(ui, "SteamVR 連携", |ui| {
        ui.horizontal(|ui| {
            if ui
                .button("SteamVR ドライバを有効化")
                .on_hover_text("ドライバ登録と SteamVR 設定の書き込みを行います")
                .clicked()
            {
                frames.push((
                    "SteamVR ドライバ有効化",
                    crate::proto::enable_steamvr_driver(),
                ));
            }
            if ui.button("インストール情報を取得").clicked() {
                frames.push((
                    "インストール情報取得",
                    crate::proto::installed_info_request(),
                ));
            }
        });
        match app.installed_info {
            Some((udev, wayland)) => {
                ui.label(format!(
                    "udev ルール: {}",
                    if udev {
                        "インストール済み"
                    } else {
                        "未インストール"
                    }
                ));
                ui.label(format!(
                    "セッション: {}",
                    if wayland { "Wayland" } else { "X11" }
                ));
            }
            None => {
                ui.label(
                    RichText::new("インストール情報は未取得です")
                        .small()
                        .color(theme::MUTED),
                );
            }
        }
    });

    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

fn process_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<Vec<u8>> = Vec::new();
    ui::section(ui, "サーバープロセス", |ui| {
        let running = app.server.is_running();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if running { "実行中" } else { "停止" }).color(if running {
                    theme::OK
                } else {
                    theme::MUTED
                }),
            );
            ui.label(
                RichText::new(app.server.status_text())
                    .small()
                    .color(theme::MUTED),
            );
        });
        ui.label(
            RichText::new(format!("jar: {}", app.server.jar_display()))
                .monospace()
                .small()
                .color(theme::MUTED),
        );
        ui.label(
            RichText::new(format!("java: {}", app.server.java_display()))
                .monospace()
                .small()
                .color(theme::MUTED),
        );

        ui.horizontal(|ui| {
            ui.label("サーバーディレクトリ");
            let mut dir = app
                .config
                .server_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            let response = ui.add(
                egui::TextEdit::singleline(&mut dir)
                    .desired_width(260.0)
                    .hint_text("jar を含むディレクトリ"),
            );
            if response.changed() {
                let trimmed = dir.trim();
                app.config.server_dir = if trimmed.is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(trimmed))
                };
            }
        });
        ui.horizontal(|ui| {
            if ui.button("再検出").clicked() {
                let dir = app.config.server_dir.clone();
                let java = app.config.java_path.clone();
                app.server.detect(dir.as_deref(), java.as_deref());
                app.save_config();
            }
            if ui.button("サーバーログを消去").clicked() {
                app.server.clear_logs();
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if running {
                if ui
                    .button(RichText::new("サーバーを停止").color(theme::WARN))
                    .clicked()
                {
                    app.server.stop();
                }
            } else if ui.button("サーバーを起動").clicked() {
                match app.server.start() {
                    Ok(()) => app.toast(crate::app::Severity::Info, "サーバーを起動しました"),
                    Err(error) => app.toast(crate::app::Severity::Error, error),
                }
            }
            if ui.button("設定ファイルを開く").clicked()
                && let Some(path) = config::server_config_path(app.config.server_dir.as_deref())
                && let Some(parent) = path.parent()
            {
                let _ = std::fs::create_dir_all(parent);
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        });

        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("server-stdout")
            .max_height(240.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let text = if app.server.logs.is_empty() {
                    "サーバーログはありません".to_string()
                } else {
                    app.server.logs.join("\n")
                };
                ui.add(
                    egui::Label::new(RichText::new(text).monospace().small())
                        .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
    });

    for frame in frames.drain(..) {
        app.send_frame(frame, "サーバー");
    }
}

fn client_log_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "GUI イベントログ", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("client-log")
            .max_height(560.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let text = if app.event_log.is_empty() {
                    "まだイベントはありません".to_string()
                } else {
                    app.event_log
                        .iter()
                        .rev()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                ui.add(
                    egui::Label::new(RichText::new(text).monospace().small())
                        .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
    });
}
