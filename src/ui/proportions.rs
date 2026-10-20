use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::*;
use crate::theme;
use crate::ui;

const BONES: std::ops::RangeInclusive<u8> = 1..=21;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("プロポーション");
        ui.label(
            RichText::new(
                "骨格の各ボーン長を調整します。変更はスライダーを離した時点で送信されます。",
            )
            .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                bones_card(&mut columns[0], app);
                feed_bones_card(&mut columns[0], app);
                calibration_card(&mut columns[1], app);
                autobone_card(&mut columns[1], app);
            });
        } else {
            bones_card(ui, app);
            ui.add_space(8.0);
            feed_bones_card(ui, app);
            ui.add_space(8.0);
            calibration_card(ui, app);
            ui.add_space(8.0);
            autobone_card(ui, app);
        }
    });
}

fn bones_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let skeleton = app.skeleton.clone();
    if skeleton.bones.is_empty() {
        ui::section(ui, "骨格", |ui| {
            ui.label(RichText::new("骨格設定を取得中…").color(theme::MUTED));
        });
        return;
    }

    let mut frames: Vec<(u8, f32)> = Vec::new();
    let mut reset_all = false;
    ui::section(ui, "骨格", |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("身長 {:.2} m", skeleton.user_height)).color(theme::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if skeleton.reset {
                    ui.label(
                        RichText::new("サーバーが骨格の再計算を要求しています")
                            .small()
                            .color(theme::WARN),
                    );
                }
                if ui
                    .button(RichText::new("全て初期化").color(theme::WARN))
                    .clicked()
                {
                    reset_all = true;
                }
                if ui.button("再取得").clicked() {
                    app.request_settings();
                }
            });
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("bones-scroll")
            .max_height(430.0)
            .show(ui, |ui| {
                egui::Grid::new("bones-grid")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for bone in BONES {
                            let Some(value) = skeleton.bones.get(&bone) else {
                                continue;
                            };
                            ui.label(skeleton_bone_label(bone));
                            let mut draft = *value;
                            let response = ui.add(
                                egui::DragValue::new(&mut draft)
                                    .speed(0.005)
                                    .range(-1.0..=2.0)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            );
                            if (response.drag_stopped() || response.lost_focus())
                                && (draft - value).abs() > 1e-4
                            {
                                frames.push((bone, draft));
                            }
                            ui.end_row();
                        }
                    });
            });
    });

    if reset_all {
        let frame = crate::proto::skeleton_reset_all();
        app.send_frame(frame, "骨格の初期化");
        app.request_settings();
    }
    for (bone, value) in frames {
        let frame = crate::proto::change_skeleton_config(bone, value);
        app.send_frame(frame, "骨格設定");
    }
}

fn feed_bones_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "フィード骨格 (実際の算出値)", |ui| {
        if app.bones.is_empty() {
            ui.label(
                RichText::new("骨格データを受信していません。サーバーページで bones 購読を有効にしてください。")
                    .small()
                    .color(theme::MUTED),
            );
            if ui.button("bones 購読を有効化").clicked() {
                app.conn.include_bones = true;
                app.config.include_bones = true;
                app.conn.restart_data_feed();
                app.save_config();
            }
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("feed-bones")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("feed-bones-grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for bone in &app.bones {
                            ui.label(body_part_label(bone.body_part));
                            ui.label(
                                RichText::new(format!(
                                    "{:.3} m  ({:.2}, {:.2}, {:.2})",
                                    bone.bone_length,
                                    bone.head_position[0],
                                    bone.head_position[1],
                                    bone.head_position[2],
                                ))
                                .monospace()
                                .small(),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}

fn calibration_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut start = false;
    let mut cancel = false;
    ui::section(ui, "身長キャリブレーション", |ui| {
        ui.label(
            RichText::new("HMD の高さから身長を推定し、骨格を自動調整します。")
                .small()
                .color(theme::MUTED),
        );
        match &app.user_height {
            Some(status) => {
                ui.label(format!("HMD 高さ: {:.2} m", status.hmd_height));
                ui.label(format!("状態: {}", user_height_status_label(status.status)));
            }
            None => {
                ui.label(RichText::new("未計測").color(theme::MUTED));
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let enabled = app.guards.can_do_user_height_calibration;
            if ui
                .add_enabled(enabled, egui::Button::new("計測を開始"))
                .clicked()
            {
                start = true;
            }
            if ui.button("キャンセル").clicked() {
                cancel = true;
            }
        });
    });

    if start {
        let frame = crate::proto::start_user_height_calibration();
        app.send_frame(frame, "身長キャリブレーション開始");
    }
    if cancel {
        let frame = crate::proto::cancel_user_height_calibration();
        app.send_frame(frame, "身長キャリブレーション中断");
    }
}

fn autobone_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut send: Option<(&'static str, Vec<u8>)> = None;
    let status = app.autobone.clone();
    let has_height = !app.skeleton.bones.is_empty();

    ui::section(ui, "AutoBone", |ui| {
        ui.label(
            RichText::new("トラッカーで姿勢を記録し、骨格を自動計算します。")
                .small()
                .color(theme::MUTED),
        );

        match &status {
            Some(AutoBoneStatus::Progress {
                process_type,
                current,
                total,
                completed,
                success,
                eta,
            }) => {
                let fraction = if *total == 0 || *total == u32::MAX {
                    0.0
                } else {
                    (*current as f32 / *total as f32).clamp(0.0, 1.0)
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(260.0)
                        .text(format!(
                            "{} {:.0}%",
                            autobone_process_label(*process_type),
                            fraction * 100.0
                        )),
                );
                if *total == u32::MAX {
                    ui.label(RichText::new("進捗は不明です").small().color(theme::MUTED));
                }
                if *completed {
                    let color = if *success { theme::OK } else { theme::ERROR };
                    ui.label(
                        RichText::new(if *success {
                            "完了しました"
                        } else {
                            "失敗しました"
                        })
                        .color(color),
                    );
                } else if *eta > 0.0 {
                    ui.label(
                        RichText::new(format!("残り約 {:.0} 秒", eta))
                            .small()
                            .color(theme::MUTED),
                    );
                }
            }
            Some(AutoBoneStatus::Epoch {
                current_epoch,
                total_epochs,
                error,
            }) => {
                let fraction = if *total_epochs == 0 {
                    0.0
                } else {
                    (*current_epoch as f32 / *total_epochs as f32).clamp(0.0, 1.0)
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(260.0)
                        .text(format!("エポック {current_epoch}/{total_epochs}")),
                );
                ui.label(
                    RichText::new(format!("誤差 {error:.4}"))
                        .small()
                        .color(theme::MUTED),
                );
            }
            None => {
                ui.label(RichText::new("待機中").color(theme::MUTED));
            }
        }

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("記録開始").clicked() {
                send = Some(("AutoBone 記録開始", crate::proto::auto_bone_process(1)));
            }
            if ui.button("記録停止").clicked() {
                send = Some((
                    "AutoBone 記録停止",
                    crate::proto::auto_bone_stop_recording(),
                ));
            }
            if ui.button("記録を破棄").clicked() {
                send = Some((
                    "AutoBone 記録破棄",
                    crate::proto::auto_bone_cancel_recording(),
                ));
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_height, egui::Button::new("処理を実行"))
                .clicked()
            {
                send = Some(("AutoBone 処理", crate::proto::auto_bone_process(3)));
            }
            if ui
                .add_enabled(has_height, egui::Button::new("結果を適用"))
                .clicked()
            {
                send = Some(("AutoBone 適用", crate::proto::auto_bone_apply()));
            }
        });
        ui.label(
            RichText::new("処理完了後に「結果を適用」を押すと骨格へ反映されます。")
                .small()
                .color(theme::MUTED),
        );
    });

    if let Some((label, frame)) = send {
        app.send_frame(frame, label);
    }
}

fn autobone_process_label(process_type: u8) -> &'static str {
    match process_type {
        1 => "記録",
        2 => "保存",
        3 => "処理",
        4 => "適用",
        _ => "待機",
    }
}
