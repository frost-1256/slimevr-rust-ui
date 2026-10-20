use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::*;
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("トラッカー");
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.assign_filter)
                .hint_text("名前 / 役割で絞り込み")
                .desired_width(220.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme::text_button("設定再取得")).clicked() {
                app.request_settings();
            }
            ui.label(
                RichText::new(format!(
                    "{} 接続 / {} 台",
                    app.connected_tracker_count(),
                    app.all_trackers().len()
                ))
                .color(theme::MUTED),
            );
        });
    });
    ui.add_space(6.0);

    if !app.computed_trackers().is_empty() {
        ui.label(
            RichText::new(format!(
                "サーバーが計算している仮想トラッカー: {} 件（実機ではありません）",
                app.computed_trackers().len()
            ))
            .color(theme::MUTED)
            .small(),
        );
        ui.add_space(4.0);
    }

    if app.all_trackers().is_empty() {
        ui::placeholder(
            ui,
            "トラッカーが検出されていないか、割り当てられていません。",
        );
        return;
    }

    if ui::two_columns(ui) {
        ui.columns(2, |columns| {
            list(app, &mut columns[0]);
            details(app, &mut columns[1]);
        });
    } else {
        list(app, ui);
        ui.add_space(8.0);
        details(app, ui);
    }
}

fn list(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    let filter = app.assign_filter.to_lowercase();
    #[allow(clippy::type_complexity)]
    let trackers: Vec<(
        TrackerKey,
        String,
        u8,
        TrackerStatus,
        Option<u8>,
        bool,
        bool,
        bool,
    )> = app
        .all_trackers()
        .iter()
        .map(|tracker| {
            let battery = app
                .device_of(tracker.key)
                .and_then(|device| device.battery_pct);
            (
                tracker.key,
                tracker.name(),
                tracker.body_part,
                tracker.status,
                battery,
                tracker.is_computed,
                tracker.is_hmd,
                tracker.is_imu,
            )
        })
        .collect();

    theme::card(ui, |ui| {
        ui.label(RichText::new("一覧").strong());
        egui::ScrollArea::vertical()
            .id_salt("tracker-list")
            .max_height(520.0)
            .show(ui, |ui| {
                for (key, name, body_part, status, battery, computed, hmd, imu) in trackers {
                    let haystack =
                        format!("{} {}", name, body_part_label(body_part)).to_lowercase();
                    if !filter.is_empty() && !haystack.contains(&filter) {
                        continue;
                    }
                    let selected = app.selected == Some(key);
                    let response = egui::Frame::new()
                        .fill(if selected {
                            theme::SURFACE_ALT
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("●").color(ui::status_color(status)));
                                ui.label(RichText::new(&name).strong());
                                if hmd {
                                    ui.label(RichText::new("HMD").small().color(theme::PURPLE));
                                }
                                if computed {
                                    ui.label(RichText::new("演算").small().color(theme::PURPLE));
                                }
                                if !imu {
                                    ui.label(RichText::new("非IMU").small().color(theme::MUTED));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(match battery {
                                                Some(percent) => format!("{percent}%"),
                                                None => "—".to_string(),
                                            })
                                            .small()
                                            .color(
                                                battery
                                                    .map(theme::battery_color)
                                                    .unwrap_or(theme::MUTED),
                                            ),
                                        );
                                        ui.label(
                                            RichText::new(body_part_label(body_part))
                                                .small()
                                                .color(theme::MUTED),
                                        );
                                    },
                                );
                            });
                        })
                        .response;
                    if response.interact(egui::Sense::click()).clicked() {
                        app.selected = Some(key);
                    }
                    ui.add_space(2.0);
                }
            });
    });
}

fn details(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    let Some(key) = app.selected else {
        ui::placeholder(ui, "左の一覧からトラッカーを選択してください");
        return;
    };
    let Some(tracker) = app
        .all_trackers()
        .into_iter()
        .find(|tracker| tracker.key == key)
        .cloned()
    else {
        ui::placeholder(ui, "選択中のトラッカーが見つかりません");
        return;
    };

    if app.rename_key != Some(key) {
        app.rename_key = Some(key);
        app.rename_buffer = tracker.custom_name.clone();
    }

    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tracker.name()).size(17.0).strong());
            ui.label(RichText::new(tracker.status.label()).color(ui::status_color(tracker.status)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("ID {}", key.label()))
                        .monospace()
                        .small()
                        .color(theme::MUTED),
                );
            });
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("役割");
            let current = body_part_label(tracker.body_part);
            egui::ComboBox::from_id_salt(("role", key))
                .selected_text(current)
                .width(180.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(tracker.body_part == 0, "割り当てなし")
                        .clicked()
                    {
                        app.assign_tracker(key, 0, None);
                    }
                    for (body_part, label) in ASSIGNABLE_BODY_PARTS {
                        if ui
                            .selectable_label(tracker.body_part == *body_part, *label)
                            .clicked()
                        {
                            app.assign_tracker(key, *body_part, None);
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("表示名");
            ui.add(
                egui::TextEdit::singleline(&mut app.rename_buffer)
                    .hint_text("カスタム名")
                    .desired_width(170.0),
            );
            if ui
                .add(theme::primary_button("保存"))
                .on_hover_text("AssignTrackerRequest として送信します")
                .clicked()
            {
                let name = app.rename_buffer.clone();
                let body_part = tracker.body_part;
                app.assign_tracker(key, body_part, Some(name));
            }
            if ui.add(theme::text_button("クリア")).clicked() {
                app.rename_buffer.clear();
                let body_part = tracker.body_part;
                app.assign_tracker(key, body_part, None);
            }
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    app.guards.can_do_mounting,
                    theme::outlined_button("マウンティング リセット"),
                )
                .clicked()
            {
                app.start_reset(ResetType::Mounting, &[tracker.body_part]);
            }
            let mag_enabled = tracker.magnetometer == 2;
            let mag_label = if mag_enabled {
                "磁気補正を無効化"
            } else {
                "磁気補正を有効化"
            };
            if ui
                .add_enabled(tracker.magnetometer != 0, theme::outlined_button(mag_label))
                .on_hover_text("MagnetometerStatus が「非対応」の場合は操作できません")
                .clicked()
            {
                let frame = crate::proto::change_mag_toggle(key, !mag_enabled);
                app.send_frame(frame, "磁気補正の切替");
            }
            if ui.add(theme::text_button("磁気状態を取得")).clicked() {
                let frame = crate::proto::mag_toggle_request(key);
                app.send_frame(frame, "磁気状態取得");
            }
        });

        ui.add_space(8.0);
        egui::Grid::new(("tracker-info", key))
            .num_columns(2)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("役割").color(theme::MUTED));
                ui.label(body_part_label(tracker.body_part));
                ui.end_row();

                ui.label(RichText::new("状態").color(theme::MUTED));
                ui.label(tracker.status.label());
                ui.end_row();

                ui.label(RichText::new("TPS").color(theme::MUTED));
                ui.label(
                    tracker
                        .tps
                        .map(|tps| tps.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                );
                ui.end_row();

                ui.label(RichText::new("温度").color(theme::MUTED));
                ui.label(
                    tracker
                        .temp
                        .map(|temp| format!("{temp:.1} °C"))
                        .unwrap_or_else(|| "—".to_string()),
                );
                ui.end_row();

                ui.label(RichText::new("磁気").color(theme::MUTED));
                ui.label(magnetometer_label(tracker.magnetometer));
                ui.end_row();

                if let Some(identifier) = &tracker.hardware_identifier {
                    ui.label(RichText::new("ハードウェア ID").color(theme::MUTED));
                    ui.label(RichText::new(identifier).monospace().small());
                    ui.end_row();
                }

                ui.label(RichText::new("種別").color(theme::MUTED));
                ui.label(format!(
                    "{}{}{}",
                    if tracker.is_imu { "IMU" } else { "非 IMU" },
                    if tracker.is_computed { ", 演算" } else { "" },
                    if tracker.is_hmd { ", HMD" } else { "" },
                ));
                ui.end_row();

                ui.label(RichText::new("ドリフト補正").color(theme::MUTED));
                ui.label(if tracker.allow_drift_compensation {
                    "許可"
                } else {
                    "不許可"
                });
                ui.end_row();

                ui.label(RichText::new("ヨー").color(theme::MUTED));
                ui.label(
                    tracker
                        .rotation
                        .map(|rotation| format!("{:.1}°", rotation.yaw_degrees()))
                        .unwrap_or_else(|| "—".to_string()),
                );
                ui.end_row();

                ui.label(RichText::new("装着クォータニオン").color(theme::MUTED));
                let mount = tracker.mounting_orientation;
                ui.label(
                    RichText::new(format!(
                        "({:.2}, {:.2}, {:.2}, {:.2})",
                        mount.x, mount.y, mount.z, mount.w
                    ))
                    .monospace()
                    .small(),
                );
                ui.end_row();
            });

        if let Some(device) = app.device_of(key) {
            ui.add_space(6.0);
            ui.separator();
            ui.label(RichText::new("デバイス").strong());
            egui::Grid::new(("device-info", key))
                .num_columns(2)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    let mut row = |label: &str, value: String| {
                        ui.label(RichText::new(label).color(theme::MUTED));
                        ui.label(value);
                        ui.end_row();
                    };
                    if !device.custom_name.is_empty() {
                        row("デバイス名", device.custom_name.clone());
                    }
                    row(
                        "ファームウェア",
                        device
                            .firmware_version
                            .clone()
                            .unwrap_or_else(|| "—".to_string()),
                    );
                    if let Some(date) = &device.firmware_date {
                        row("FW ビルド日", date.clone());
                    }
                    if let Some(manufacturer) = &device.manufacturer {
                        row("メーカー", manufacturer.clone());
                    }
                    if let Some(identifier) = &device.hardware_identifier {
                        row("ハードウェア ID", identifier.clone());
                    }
                    if let Some(battery) = device.battery_pct {
                        row("バッテリー", format!("{battery} %"));
                    }
                    if let Some(voltage) = device.battery_voltage {
                        row("電圧", format!("{voltage:.2} V"));
                    }
                    if let Some(rssi) = device.rssi {
                        row("RSSI", format!("{rssi} dBm"));
                    }
                    if let Some(ping) = device.ping {
                        row("Ping", format!("{ping} ms"));
                    }
                    if let Some(loss) = device.packet_loss {
                        row("パケット損失", format!("{loss:.2} %"));
                    }
                    if let Some(temp) = device.mcu_temp {
                        row("MCU 温度", format!("{temp:.1} °C"));
                    }
                    if let Some(ip) = &device.ip_address {
                        row("IP", ip.clone());
                    }
                    if let Some(mac) = device.mac_address {
                        row("MAC", crate::server::format_mac(mac));
                    }
                    if let Some(protocol) = device.network_protocol_version {
                        row("プロトコル", protocol.to_string());
                    }
                });

            ui.add_space(6.0);
            let device_id = device.id;
            let label = format!("デバイス #{} を忘れる", device_id);
            let confirmed = app
                .forget_confirm
                .as_ref()
                .map(|(id, _)| *id == device_id)
                .unwrap_or(false);
            ui.horizontal(|ui| {
                if confirmed {
                    if ui
                        .add(theme::outlined_button("本当に削除").text_color(theme::ERROR))
                        .clicked()
                    {
                        app.forget_device(device_id);
                        app.forget_confirm = None;
                    }
                    if ui.add(theme::text_button("キャンセル")).clicked() {
                        app.forget_confirm = None;
                    }
                } else if ui
                    .add(theme::text_button(label))
                    .on_hover_text("サーバーからデバイスのペアリング情報を削除します")
                    .clicked()
                {
                    app.forget_confirm = Some((device_id, std::time::Instant::now()));
                }
            });
        }
    });
}
