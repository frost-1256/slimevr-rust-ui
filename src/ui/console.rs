use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::serial_device_kind_label;
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("シリアル / ファームウェア");
        ui.label(
            RichText::new(
                "USB 接続したトラッカーの診断、WiFi 設定、OTA ファームウェア更新を行います。",
            )
            .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                serial_card(&mut columns[0], app);
                wifi_card(&mut columns[0], app);
                firmware_card(&mut columns[1], app);
                log_card(&mut columns[1], app);
            });
        } else {
            serial_card(ui, app);
            ui.add_space(8.0);
            wifi_card(ui, app);
            ui.add_space(8.0);
            firmware_card(ui, app);
            ui.add_space(8.0);
            log_card(ui, app);
        }
    });
}

fn serial_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut confirm_factory = false;

    ui::section(ui, "シリアルデバイス", |ui| {
        ui.horizontal(|ui| {
            if ui.add(theme::text_button("デバイスを再取得")).clicked() {
                frames.push((
                    "シリアルデバイス取得",
                    crate::proto::serial_devices_request(),
                ));
            }
            if app.serial.open && ui.add(theme::outlined_button("切断")).clicked() {
                frames.push(("シリアル切断", crate::proto::close_serial()));
            }
            ui.label(
                RichText::new(if app.serial.open {
                    "接続中"
                } else {
                    "未接続"
                })
                .color(if app.serial.open {
                    theme::OK
                } else {
                    theme::MUTED
                })
                .small(),
            );
        });

        if app.serial.devices.is_empty() {
            ui.label(
                RichText::new("デバイスが見つかりません")
                    .small()
                    .color(theme::MUTED),
            );
        }
        egui::Grid::new("serial-devices")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for (port, name, kind) in app.serial.devices.clone() {
                    ui.label(RichText::new(&port).monospace());
                    ui.label(format!("{name} ({})", serial_device_kind_label(kind)));
                    if ui.add(theme::primary_button("接続")).clicked() {
                        frames.push((
                            "シリアル接続",
                            crate::proto::open_serial(false, Some(&port)),
                        ));
                    }
                    ui.end_row();
                }
            });

        if app.serial.open {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.add(theme::outlined_button("再起動")).clicked() {
                    frames.push(("トラッカー再起動", crate::proto::serial_tracker_reboot()));
                }
                if ui.add(theme::outlined_button("情報取得")).clicked() {
                    frames.push(("トラッカー情報", crate::proto::serial_tracker_get_info()));
                }
                if ui.add(theme::outlined_button("WiFi スキャン")).clicked() {
                    frames.push((
                        "WiFi スキャン",
                        crate::proto::serial_tracker_get_wifi_scan(),
                    ));
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add(theme::outlined_button("工場出荷状態に戻す").text_color(theme::ERROR))
                    .clicked()
                {
                    confirm_factory = true;
                }
            });
            if confirm_factory {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("全設定とキャリブレーションが消去されます。")
                            .color(theme::WARN)
                            .small(),
                    );
                    if ui
                        .add(theme::primary_button("実行する").text_color(theme::ERROR))
                        .clicked()
                    {
                        frames.push((
                            "工場出荷リセット",
                            crate::proto::serial_tracker_factory_reset(),
                        ));
                    }
                });
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("コマンド");
                ui.add(
                    egui::TextEdit::singleline(&mut app.serial.command)
                        .desired_width(180.0)
                        .hint_text("例: SET WIFI"),
                );
                if ui.add(theme::primary_button("送信")).clicked()
                    && !app.serial.command.trim().is_empty()
                {
                    let command = app.serial.command.trim().to_string();
                    frames.push((
                        "シリアルコマンド",
                        crate::proto::serial_tracker_custom_command(&command),
                    ));
                }
            });
        }
    });

    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

fn wifi_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    ui::section(ui, "WiFi プロビジョニング", |ui| {
        ui.horizontal(|ui| {
            ui.label("SSID");
            ui.add(egui::TextEdit::singleline(&mut app.serial.ssid).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label("パスワード");
            ui.add(
                egui::TextEdit::singleline(&mut app.serial.password)
                    .password(true)
                    .desired_width(160.0),
            );
        });
        ui.horizontal(|ui| {
            let ready = app.serial.open && !app.serial.ssid.is_empty();
            if ui
                .add_enabled(ready, theme::primary_button("WiFi 設定を開始"))
                .clicked()
            {
                let ssid = app.serial.ssid.clone();
                let password = app.serial.password.clone();
                frames.push((
                    "WiFi プロビジョニング開始",
                    crate::proto::start_wifi_provisioning(&ssid, &password),
                ));
            }
            if ui.add(theme::outlined_button("停止")).clicked() {
                frames.push((
                    "WiFi プロビジョニング停止",
                    crate::proto::stop_wifi_provisioning(),
                ));
            }
        });
        match app.serial.provisioning_status {
            Some(status) => {
                ui.label(format!("状態: {}", wifi_status_label(status)));
            }
            None => {
                ui.label(RichText::new("待機中").small().color(theme::MUTED));
            }
        }
        if !app.serial.open {
            ui.label(
                RichText::new("先にシリアルデバイスへ接続してください")
                    .small()
                    .color(theme::MUTED),
            );
        }
    });

    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

fn firmware_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    ui::section(ui, "ファームウェア (OTA)", |ui| {
        ui.horizontal(|ui| {
            ui.label("URL");
            ui.add(
                egui::TextEdit::singleline(&mut app.firmware.url)
                    .desired_width(300.0)
                    .hint_text("https://…/firmware.bin"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("SHA-256");
            ui.add(
                egui::TextEdit::singleline(&mut app.firmware.digest)
                    .desired_width(300.0)
                    .hint_text("任意 (sha256-…)"),
            );
        });
        ui.label(
            RichText::new(
                "GitHub からの自動取得は未対応です。リリースの .bin URL を指定してください。",
            )
            .small()
            .color(theme::MUTED),
        );

        ui.add_space(4.0);
        let connected: Vec<(u8, String)> = app
            .devices
            .iter()
            .map(|device| {
                let name = if device.custom_name.is_empty() {
                    format!("デバイス #{}", device.id)
                } else {
                    device.custom_name.clone()
                };
                (device.id, name)
            })
            .collect();
        if connected.is_empty() {
            ui.label(
                RichText::new("更新可能なデバイスがありません")
                    .small()
                    .color(theme::MUTED),
            );
        }
        for (device_id, name) in connected {
            let selected = app.firmware.selected_devices.contains(&device_id);
            let mut checked = selected;
            ui.horizontal(|ui| {
                if theme::checkbox(ui, &mut checked, &name).changed() {
                    if checked {
                        app.firmware.selected_devices.push(device_id);
                    } else {
                        app.firmware.selected_devices.retain(|id| *id != device_id);
                    }
                }
                if let Some((progress, status)) =
                    app.firmware.progress.get(&format!("#{device_id}")).copied()
                {
                    let fraction = (f32::from(progress) / 100.0).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(120.0)
                            .text(firmware_status_label(status)),
                    );
                }
            });
        }

        ui.horizontal(|ui| {
            let ready =
                !app.firmware.selected_devices.is_empty() && !app.firmware.url.trim().is_empty();
            if ui
                .add_enabled(ready, theme::primary_button("更新を開始"))
                .on_hover_text("選択したデバイスに OTA 更新をキューします")
                .clicked()
            {
                let url = app.firmware.url.trim().to_string();
                let digest = app.firmware.digest.trim().to_string();
                for device_id in app.firmware.selected_devices.clone() {
                    frames.push((
                        "ファームウェア更新",
                        crate::proto::firmware_update_ota(device_id, &url, &digest),
                    ));
                }
            }
            if ui
                .add(theme::outlined_button("キューを停止"))
                .on_hover_text("実行中のファームウェア更新を中断します")
                .clicked()
            {
                frames.push((
                    "ファームウェア更新停止",
                    crate::proto::firmware_stop_queues(),
                ));
            }
        });
        if let Some(note) = &app.firmware.last_note {
            ui.label(RichText::new(note).small().color(theme::MUTED));
        }
    });

    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

fn log_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "シリアルログ", |ui| {
        let text = if app.serial.log.is_empty() {
            "ログはありません".to_string()
        } else {
            app.serial.log.join("\n")
        };
        egui::ScrollArea::vertical()
            .id_salt("serial-log")
            .max_height(260.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(text).monospace().small())
                        .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
    });
}

fn wifi_status_label(status: u8) -> &'static str {
    match status {
        0 => "待機",
        1 => "シリアル接続中",
        2 => "プロビジョニング中",
        3 => "完了",
        4 => "失敗",
        _ => "不明",
    }
}

fn firmware_status_label(status: u8) -> &'static str {
    match status {
        0 => "待機",
        1 => "開始",
        2 => "転送中",
        3 => "再起動中",
        4 => "成功",
        5 => "失敗",
        _ => "不明",
    }
}
