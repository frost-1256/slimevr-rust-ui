use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::*;
use crate::theme;
use crate::ui;

const FEET: &[u8] = &[10, 11];
const FINGERS: &[u8] = &[
    25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    49, 50, 51, 52, 53, 54,
];

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("リセット");
        ui.label(
            RichText::new(
                "Yaw は向きのみ、フルは全トラッカー、マウンティングは装着方向を再計算します。",
            )
            .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                controls(&mut columns[0], app);
                reset_settings(&mut columns[1], app);
            });
        } else {
            controls(ui, app);
            ui.add_space(8.0);
            reset_settings(ui, app);
        }
    });
}

fn controls(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    theme::card(ui, |ui| {
        ui.label(RichText::new("実行").strong());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let mut include_feet = app.reset_include_feet;
            if theme::switch(ui, &mut include_feet, "足を含める").changed() {
                app.reset_include_feet = include_feet;
            }
            let mut include_fingers = app.reset_include_fingers;
            if theme::switch(ui, &mut include_fingers, "指を含める").changed() {
                app.reset_include_fingers = include_fingers;
            }
        });

        ui.add_space(6.0);
        let body_parts = selected_parts(app);
        ui.horizontal_wrapped(|ui| {
            let yaw = ui.add_enabled(
                app.guards.can_do_yaw_reset,
                theme::outlined_button("Yaw リセット"),
            );
            if yaw.clicked() {
                app.start_reset(ResetType::Yaw, &body_parts);
            }
            if ui.add(theme::primary_button("フルリセット")).clicked() {
                app.start_reset(ResetType::Full, &body_parts);
            }
            let mounting = ui.add_enabled(
                app.guards.can_do_mounting,
                theme::outlined_button("マウンティング リセット"),
            );
            if mounting.clicked() {
                app.start_reset(ResetType::Mounting, &[]);
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("ガード").color(theme::MUTED));
            guard(ui, "Yaw", app.guards.can_do_yaw_reset);
            guard(ui, "マウンティング", app.guards.can_do_mounting);
            guard(ui, "身長計測", app.guards.can_do_user_height_calibration);
        });

        ui.add_space(6.0);
        match app.active_reset().cloned() {
            Some(reset) => {
                let total = reset.duration_ms.max(1) as f32;
                let fraction = (reset.progress_ms as f32 / total).clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(280.0)
                        .text(format!(
                            "{}: {:.0}%",
                            reset.reset_type.map(ResetType::label).unwrap_or("リセット"),
                            fraction * 100.0
                        )),
                );
            }
            None => {
                ui.label(RichText::new("実行中のリセットはありません").color(theme::MUTED));
            }
        }

        ui.add_space(8.0);
        ui.separator();
        let pause_label = if app.paused {
            "トラッキングを再開"
        } else {
            "トラッキングを一時停止"
        };
        if ui.add(theme::outlined_button(pause_label)).clicked() {
            app.set_paused(!app.paused);
        }
        ui.label(
            RichText::new("一時停止中もサーバーは接続を維持します。")
                .small()
                .color(theme::MUTED),
        );
    });
}

fn selected_parts(app: &SlimeVrApp) -> Vec<u8> {
    let mut parts = Vec::new();
    if app.reset_include_feet {
        parts.extend_from_slice(FEET);
    }
    if app.reset_include_fingers {
        parts.extend_from_slice(FINGERS);
    }
    parts
}

fn guard(ui: &mut egui::Ui, label: &str, allowed: bool) {
    let (text, color) = if allowed {
        ("可", theme::OK)
    } else {
        ("不可", theme::MUTED)
    };
    ui.label(
        RichText::new(format!("{label}: {text}"))
            .color(color)
            .small(),
    );
}

fn reset_settings(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let Some(truth) = app.settings.resets.clone() else {
        ui::section(ui, "リセット設定", |ui| {
            ui.label(RichText::new("設定を取得中…").color(theme::MUTED));
        });
        return;
    };

    let mut frame = None;
    let mut changed = false;
    {
        let draft = &mut app.drafts.resets;
        draft.sync(Some(&truth));
        ui::section(ui, "リセット設定", |ui| {
            changed |= theme::switch(
                ui,
                &mut draft.value.save_mounting_reset,
                "マウンティング リセットを保存",
            )
            .changed();
            changed |=
                theme::switch(ui, &mut draft.value.reset_mounting_feet, "足もリセット").changed();
            changed |= theme::switch(
                ui,
                &mut draft.value.reset_hmd_pitch,
                "HMD のピッチもリセット",
            )
            .changed();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Yaw リセットの平滑化 (秒)");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut draft.value.yaw_reset_smooth_time)
                            .speed(0.01)
                            .range(0.0..=2.0),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("腕のマウンティング リセット");
                let current = arms_mounting_mode_label(draft.value.arms_mounting_reset_mode);
                egui::ComboBox::from_id_salt("arms-mounting-mode")
                    .selected_text(current)
                    .width(230.0)
                    .show_ui(ui, |ui| {
                        for (mode, label) in [
                            (0u8, "上腕を後ろ・前腕を前"),
                            (1, "腕を前に"),
                            (2, "Tポーズから上げる"),
                            (3, "Tポーズから下ろす"),
                        ] {
                            if ui
                                .selectable_label(
                                    draft.value.arms_mounting_reset_mode == mode,
                                    label,
                                )
                                .clicked()
                            {
                                draft.value.arms_mounting_reset_mode = mode;
                                draft.dirty = true;
                            }
                        }
                    });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(draft.dirty, theme::primary_button("適用"))
                    .clicked()
                {
                    frame = Some(crate::proto::set_resets(&draft.value));
                    draft.dirty = false;
                }
                if ui
                    .add_enabled(draft.dirty, theme::outlined_button("元に戻す"))
                    .clicked()
                {
                    draft.value = truth.clone();
                    draft.dirty = false;
                }
            });
        });
    }

    if changed {
        app.drafts.resets.dirty = true;
    }
    if let Some(frame) = frame {
        app.send_frame(frame, "リセット設定");
    }
}
