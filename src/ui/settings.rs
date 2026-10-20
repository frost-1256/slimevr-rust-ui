use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::*;
use crate::theme;
use crate::ui;

/// A settings group copied from the server. Edits stay local until the user
/// presses 適用, so a broadcast from another GUI never fights with typing.
#[derive(Clone, Debug)]
pub struct GroupDraft<T: Clone> {
    pub value: T,
    pub dirty: bool,
    pub seen: bool,
}

impl<T: Clone + Default> Default for GroupDraft<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            dirty: false,
            seen: false,
        }
    }
}

impl<T: Clone> GroupDraft<T> {
    /// Adopt the server value unless the user has unsaved edits.
    pub fn sync(&mut self, truth: Option<&T>) {
        if let Some(truth) = truth {
            if !self.dirty {
                self.value = truth.clone();
            }
            self.seen = true;
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct SettingsDrafts {
    pub steam_vr: GroupDraft<SteamVrSettings>,
    pub filtering: GroupDraft<FilteringSettings>,
    pub drift: GroupDraft<DriftSettings>,
    pub osc_router: GroupDraft<OscSettings>,
    pub vrc_osc: GroupDraft<VrcOscSettings>,
    pub vmc_osc: GroupDraft<VmcOscSettings>,
    pub model: GroupDraft<ModelSettings>,
    pub resets: GroupDraft<ResetsSettings>,
    pub stay_aligned: GroupDraft<StayAlignedSettings>,
    pub tap: GroupDraft<TapDetectionSettings>,
    pub hid: GroupDraft<bool>,
    pub velocity: GroupDraft<bool>,
    pub vrm: GroupDraft<String>,
    pub vrm_dirty: bool,
}

impl SettingsDrafts {
    pub fn sync(&mut self, truth: &ServerSettings) {
        self.steam_vr.sync(truth.steam_vr.as_ref());
        self.filtering.sync(truth.filtering.as_ref());
        self.drift.sync(truth.drift.as_ref());
        self.osc_router.sync(truth.osc_router.as_ref());
        self.vrc_osc.sync(truth.vrc_osc.as_ref());
        self.vmc_osc.sync(truth.vmc_osc.as_ref());
        self.model.sync(truth.model.as_ref());
        self.resets.sync(truth.resets.as_ref());
        self.stay_aligned.sync(truth.stay_aligned.as_ref());
        self.tap.sync(truth.tap_detection.as_ref());
        self.hid.sync(truth.hid.as_ref());
        self.velocity.sync(truth.velocity.as_ref());
        if !self.vrm_dirty {
            self.vrm.sync(truth.vrm_json.as_ref());
        }
    }
}

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("設定");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("サーバーから再取得").clicked() {
                    app.request_settings();
                }
                if ui
                    .button(RichText::new("設定を初期化").color(theme::WARN))
                    .on_hover_text("サーバー側の全設定を既定値に戻します")
                    .clicked()
                {
                    let frame = crate::proto::settings_reset();
                    app.send_frame(frame, "設定初期化");
                }
            });
        });
        ui.label(
            RichText::new(
                "変更はカードの「適用」で送信されます。サーバー側の受信値が正となります。",
            )
            .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                steam_vr_card(&mut columns[0], app);
                filtering_card(&mut columns[0], app);
                drift_card(&mut columns[0], app);
                model_card(&mut columns[1], app);
                resets_card(&mut columns[1], app);
                stay_aligned_card(&mut columns[1], app);
            });
            ui.add_space(8.0);
            ui.columns(2, |columns| {
                osc_card(&mut columns[0], app);
                vrc_osc_card(&mut columns[0], app);
                vmc_osc_card(&mut columns[0], app);
                tap_card(&mut columns[1], app);
                misc_card(&mut columns[1], app);
                vrc_config_card(&mut columns[1], app);
            });
        } else {
            steam_vr_card(ui, app);
            filtering_card(ui, app);
            drift_card(ui, app);
            model_card(ui, app);
            resets_card(ui, app);
            stay_aligned_card(ui, app);
            osc_card(ui, app);
            vrc_osc_card(ui, app);
            vmc_osc_card(ui, app);
            tap_card(ui, app);
            misc_card(ui, app);
            vrc_config_card(ui, app);
        }
    });
}

fn apply_button<T: Clone>(
    ui: &mut egui::Ui,
    draft: &mut GroupDraft<T>,
    encode: impl Fn(&T) -> Vec<u8>,
) -> Option<Vec<u8>> {
    let mut frame = None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(draft.dirty && draft.seen, egui::Button::new("適用"))
            .clicked()
        {
            frame = Some(encode(&draft.value));
            draft.dirty = false;
        }
        if ui
            .add_enabled(draft.dirty, egui::Button::new("元に戻す"))
            .clicked()
        {
            draft.dirty = false;
            draft.seen = false;
        }
    });
    frame
}

fn wait_note(ui: &mut egui::Ui, seen: bool) {
    if !seen {
        ui.label(
            RichText::new("サーバーから設定を取得中…")
                .small()
                .color(theme::MUTED),
        );
    }
}

fn steam_vr_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.steam_vr;
        draft.sync(app.settings.steam_vr.as_ref());
        ui::section(ui, "SteamVR", |ui| {
            wait_note(ui, draft.seen);
            let value = &mut draft.value;
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut value.waist, "腰").changed();
                draft.dirty |= ui.checkbox(&mut value.chest, "胸").changed();
            });
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut value.left_knee, "左膝").changed();
                draft.dirty |= ui.checkbox(&mut value.right_knee, "右膝").changed();
            });
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut value.left_foot, "左足").changed();
                draft.dirty |= ui.checkbox(&mut value.right_foot, "右足").changed();
            });
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut value.left_elbow, "左肘").changed();
                draft.dirty |= ui.checkbox(&mut value.right_elbow, "右肘").changed();
            });
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut value.left_hand, "左手").changed();
                draft.dirty |= ui.checkbox(&mut value.right_hand, "右手").changed();
            });
            draft.dirty |= ui
                .checkbox(
                    &mut value.automatic_tracker_toggle,
                    "SteamVR 起動時に自動でトラッカーを有効化",
                )
                .changed();
            apply_button(ui, draft, crate::proto::set_steam_vr)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "SteamVR 設定");
    }
}

fn filtering_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.filtering;
        draft.sync(app.settings.filtering.as_ref());
        ui::section(ui, "フィルタリング", |ui| {
            wait_note(ui, draft.seen);
            ui.horizontal(|ui| {
                ui.label("種類");
                egui::ComboBox::from_id_salt("filtering-type")
                    .selected_text(filtering_type_label(draft.value.filter_type))
                    .show_ui(ui, |ui| {
                        for kind in 0..=2u8 {
                            if ui
                                .selectable_label(
                                    draft.value.filter_type == kind,
                                    filtering_type_label(kind),
                                )
                                .clicked()
                            {
                                draft.value.filter_type = kind;
                                draft.dirty = true;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("強さ");
                draft.dirty |= ui
                    .add(egui::Slider::new(&mut draft.value.amount, 0.0..=1.0).fixed_decimals(2))
                    .changed();
            });
            apply_button(ui, draft, crate::proto::set_filtering)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "フィルタリング設定");
    }
}

fn drift_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.drift;
        draft.sync(app.settings.drift.as_ref());
        ui::section(ui, "ドリフト補正", |ui| {
            wait_note(ui, draft.seen);
            draft.dirty |= ui.checkbox(&mut draft.value.enabled, "有効").changed();
            draft.dirty |= ui
                .checkbox(&mut draft.value.prediction, "予測補正を使う")
                .changed();
            ui.horizontal(|ui| {
                ui.label("補正量");
                draft.dirty |= ui
                    .add(egui::Slider::new(&mut draft.value.amount, 0.0..=1.0).fixed_decimals(2))
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("最大リセット回数");
                draft.dirty |= ui
                    .add(egui::DragValue::new(&mut draft.value.max_resets).range(0..=1000))
                    .changed();
            });
            apply_button(ui, draft, crate::proto::set_drift)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "ドリフト補正設定");
    }
}

fn model_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.model;
        draft.sync(app.settings.model.as_ref());
        ui::section(ui, "トラッカー計算 (Model)", |ui| {
            wait_note(ui, draft.seen);
            let toggles = &mut draft.value.toggles;
            let items: [(&mut bool, &str); 12] = [
                (&mut toggles.extended_spine, "拡張脊椎"),
                (&mut toggles.extended_pelvis, "拡張骨盤"),
                (&mut toggles.extended_knee, "拡張膝"),
                (&mut toggles.force_arms_from_hmd, "腕を HMD から強制"),
                (&mut toggles.floor_clip, "床クリップ"),
                (&mut toggles.skating_correction, "スケーティング補正"),
                (&mut toggles.toe_snap, "つま先スナップ"),
                (&mut toggles.foot_plant, "足の接地"),
                (&mut toggles.self_localization, "自己位置推定"),
                (&mut toggles.use_position, "位置を使用"),
                (&mut toggles.enforce_constraints, "拘束を強制"),
                (&mut toggles.correct_constraints, "拘束を補正"),
            ];
            for (value, label) in items {
                draft.dirty |= ui.checkbox(value, label).changed();
            }

            ui.add_space(4.0);
            ui.label(RichText::new("比率").color(theme::MUTED).small());
            let ratios: [(&mut Option<f32>, &str); 7] = [
                (
                    &mut draft.value.ratios.impute_waist_from_chest_hip,
                    "腰を胸+骨盤から補完",
                ),
                (
                    &mut draft.value.ratios.impute_waist_from_chest_legs,
                    "腰を胸+脚から補完",
                ),
                (
                    &mut draft.value.ratios.impute_hip_from_chest_legs,
                    "骨盤を胸+脚から補完",
                ),
                (
                    &mut draft.value.ratios.impute_hip_from_waist_legs,
                    "骨盤を腰+脚から補完",
                ),
                (&mut draft.value.ratios.interp_hip_legs, "骨盤-脚の補間"),
                (
                    &mut draft.value.ratios.interp_knee_tracker_ankle,
                    "膝トラッカー-足首の補間",
                ),
                (&mut draft.value.ratios.interp_knee_ankle, "膝-足首の補間"),
            ];
            for (value, label) in ratios {
                ui.horizontal(|ui| {
                    let mut enabled = value.is_some();
                    if ui.checkbox(&mut enabled, "").changed() {
                        *value = if enabled { Some(0.5) } else { None };
                        draft.dirty = true;
                    }
                    ui.label(label);
                    if let Some(ratio) = value {
                        draft.dirty |= ui
                            .add(egui::Slider::new(ratio, 0.0..=1.0).fixed_decimals(2))
                            .changed();
                    } else {
                        ui.label(RichText::new("自動").small().color(theme::MUTED));
                    }
                });
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut enabled = draft.value.correction_strength.is_some();
                if ui.checkbox(&mut enabled, "脚補正の強さ").changed() {
                    draft.value.correction_strength = enabled.then_some(0.5);
                    draft.dirty = true;
                }
                if let Some(strength) = &mut draft.value.correction_strength {
                    draft.dirty |= ui
                        .add(egui::Slider::new(strength, 0.0..=1.0).fixed_decimals(2))
                        .changed();
                }
            });
            ui.horizontal(|ui| {
                ui.label("HMD 身長 (m)");
                let mut hmd = draft.value.hmd_height.unwrap_or(0.0);
                if ui
                    .add(egui::DragValue::new(&mut hmd).speed(0.01).range(0.0..=3.0))
                    .changed()
                {
                    draft.value.hmd_height = Some(hmd);
                    draft.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("床高 (m)");
                let mut floor = draft.value.floor_height.unwrap_or(0.0);
                if ui
                    .add(
                        egui::DragValue::new(&mut floor)
                            .speed(0.01)
                            .range(0.0..=3.0),
                    )
                    .changed()
                {
                    draft.value.floor_height = Some(floor);
                    draft.dirty = true;
                }
            });

            apply_button(ui, draft, crate::proto::set_model)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "モデル設定");
    }
}

fn resets_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.resets;
        draft.sync(app.settings.resets.as_ref());
        ui::section(ui, "リセット", |ui| {
            wait_note(ui, draft.seen);
            draft.dirty |= ui
                .checkbox(
                    &mut draft.value.save_mounting_reset,
                    "マウンティング リセットを保存",
                )
                .changed();
            draft.dirty |= ui
                .checkbox(&mut draft.value.reset_mounting_feet, "足もリセット")
                .changed();
            draft.dirty |= ui
                .checkbox(&mut draft.value.reset_hmd_pitch, "HMD のピッチもリセット")
                .changed();
            ui.horizontal(|ui| {
                ui.label("Yaw 平滑化 (秒)");
                draft.dirty |= ui
                    .add(
                        egui::DragValue::new(&mut draft.value.yaw_reset_smooth_time)
                            .speed(0.01)
                            .range(0.0..=2.0),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("腕のリセット");
                egui::ComboBox::from_id_salt("settings-arms-mode")
                    .selected_text(arms_mounting_mode_label(
                        draft.value.arms_mounting_reset_mode,
                    ))
                    .show_ui(ui, |ui| {
                        for mode in 0..=3u8 {
                            if ui
                                .selectable_label(
                                    draft.value.arms_mounting_reset_mode == mode,
                                    arms_mounting_mode_label(mode),
                                )
                                .clicked()
                            {
                                draft.value.arms_mounting_reset_mode = mode;
                                draft.dirty = true;
                            }
                        }
                    });
            });
            apply_button(ui, draft, crate::proto::set_resets)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "リセット設定");
    }
}

fn stay_aligned_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.stay_aligned;
        draft.sync(app.settings.stay_aligned.as_ref());
        ui::section(ui, "Stay Aligned (姿勢維持補正)", |ui| {
            wait_note(ui, draft.seen);
            draft.dirty |= ui.checkbox(&mut draft.value.enabled, "有効").changed();
            draft.dirty |= ui
                .checkbox(&mut draft.value.hide_yaw_correction, "Yaw 補正を隠す")
                .changed();

            let poses: [(&mut bool, &mut f32, &mut f32, &mut f32, &str); 3] = [
                (
                    &mut draft.value.standing_enabled,
                    &mut draft.value.standing_upper_leg_angle,
                    &mut draft.value.standing_lower_leg_angle,
                    &mut draft.value.standing_foot_angle,
                    "立ち",
                ),
                (
                    &mut draft.value.sitting_enabled,
                    &mut draft.value.sitting_upper_leg_angle,
                    &mut draft.value.sitting_lower_leg_angle,
                    &mut draft.value.sitting_foot_angle,
                    "座位",
                ),
                (
                    &mut draft.value.flat_enabled,
                    &mut draft.value.flat_upper_leg_angle,
                    &mut draft.value.flat_lower_leg_angle,
                    &mut draft.value.flat_foot_angle,
                    "横たわり",
                ),
            ];
            for (enabled, upper, lower, foot, label) in poses {
                ui.horizontal(|ui| {
                    draft.dirty |= ui.checkbox(enabled, label).changed();
                    ui.label("腿上");
                    draft.dirty |= ui
                        .add(egui::DragValue::new(upper).speed(1.0).range(0.0..=180.0))
                        .changed();
                    ui.label("腿下");
                    draft.dirty |= ui
                        .add(egui::DragValue::new(lower).speed(1.0).range(0.0..=180.0))
                        .changed();
                    ui.label("足");
                    draft.dirty |= ui
                        .add(egui::DragValue::new(foot).speed(1.0).range(0.0..=180.0))
                        .changed();
                });
            }
            draft.dirty |= ui
                .checkbox(&mut draft.value.setup_complete, "セットアップ完了")
                .changed();
            let mut enabled_now = None;
            ui.horizontal(|ui| {
                if ui
                    .button("Stay Aligned を今すぐ有効化")
                    .on_hover_text("設定の適用とは別に、補正を即座に有効にします")
                    .clicked()
                {
                    enabled_now = Some(true);
                }
                if ui.button("無効化").clicked() {
                    enabled_now = Some(false);
                }
            });
            if let Some(enable) = enabled_now {
                draft.value.enabled = enable;
                draft.dirty = false;
                return Some(crate::proto::enable_stay_aligned(enable));
            }
            apply_button(ui, draft, crate::proto::set_stay_aligned)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "Stay Aligned 設定");
    }
}

fn osc_fields(ui: &mut egui::Ui, osc: &mut OscSettings, dirty: &mut bool) {
    *dirty |= ui.checkbox(&mut osc.enabled, "OSC を有効化").changed();
    ui.horizontal(|ui| {
        ui.label("受信ポート");
        *dirty |= ui
            .add(egui::DragValue::new(&mut osc.port_in).range(1..=65535))
            .changed();
        ui.label("送信ポート");
        *dirty |= ui
            .add(egui::DragValue::new(&mut osc.port_out).range(1..=65535))
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("アドレス");
        *dirty |= ui
            .add(egui::TextEdit::singleline(&mut osc.address).desired_width(180.0))
            .changed();
    });
}

fn osc_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.osc_router;
        draft.sync(app.settings.osc_router.as_ref());
        ui::section(ui, "OSC Router", |ui| {
            wait_note(ui, draft.seen);
            osc_fields(ui, &mut draft.value, &mut draft.dirty);
            apply_button(ui, draft, crate::proto::set_osc_router)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "OSC Router 設定");
    }
}

fn vrc_osc_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.vrc_osc;
        draft.sync(app.settings.vrc_osc.as_ref());
        ui::section(ui, "VRChat OSC", |ui| {
            wait_note(ui, draft.seen);
            osc_fields(ui, &mut draft.value.osc, &mut draft.dirty);
            ui.horizontal_wrapped(|ui| {
                draft.dirty |= ui.checkbox(&mut draft.value.head, "頭").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.chest, "胸").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.waist, "腰").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.knees, "膝").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.feet, "足").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.elbows, "肘").changed();
                draft.dirty |= ui.checkbox(&mut draft.value.hands, "手").changed();
            });
            draft.dirty |= ui
                .checkbox(&mut draft.value.oscquery_enabled, "OSCQuery を有効化")
                .changed();
            apply_button(ui, draft, crate::proto::set_vrc_osc)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "VRChat OSC 設定");
    }
}

fn vmc_osc_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.vmc_osc;
        draft.sync(app.settings.vmc_osc.as_ref());
        ui::section(ui, "VMC OSC", |ui| {
            wait_note(ui, draft.seen);
            osc_fields(ui, &mut draft.value.osc, &mut draft.dirty);
            draft.dirty |= ui
                .checkbox(&mut draft.value.anchor_hip, "骨盤を基準にする")
                .changed();
            draft.dirty |= ui
                .checkbox(&mut draft.value.mirror_tracking, "トラッキングをミラー")
                .changed();
            apply_button(ui, draft, crate::proto::set_vmc_osc)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "VMC OSC 設定");
    }
}

fn tap_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let frame = {
        let draft = &mut app.drafts.tap;
        draft.sync(app.settings.tap_detection.as_ref());
        ui::section(ui, "タップ検出", |ui| {
            wait_note(ui, draft.seen);
            #[allow(clippy::type_complexity)]
            let rows: [(&mut Option<bool>, &mut Option<f32>, &mut Option<u8>, &str);
                3] = [
                (
                    &mut draft.value.full_reset_enabled,
                    &mut draft.value.full_reset_delay,
                    &mut draft.value.full_reset_taps,
                    "フルリセット",
                ),
                (
                    &mut draft.value.yaw_reset_enabled,
                    &mut draft.value.yaw_reset_delay,
                    &mut draft.value.yaw_reset_taps,
                    "Yaw リセット",
                ),
                (
                    &mut draft.value.mounting_reset_enabled,
                    &mut draft.value.mounting_reset_delay,
                    &mut draft.value.mounting_reset_taps,
                    "マウンティング リセット",
                ),
            ];
            for (enabled, delay, taps, label) in rows {
                ui.horizontal(|ui| {
                    let mut on = enabled.unwrap_or(false);
                    if ui.checkbox(&mut on, label).changed() {
                        *enabled = Some(on);
                        draft.dirty = true;
                    }
                    ui.label("回数");
                    let mut tap_count = taps.unwrap_or(2);
                    if ui
                        .add(egui::DragValue::new(&mut tap_count).range(1..=10))
                        .changed()
                    {
                        *taps = Some(tap_count);
                        draft.dirty = true;
                    }
                    ui.label("間隔 (秒)");
                    let mut delay_value = delay.unwrap_or(0.3);
                    if ui
                        .add(
                            egui::DragValue::new(&mut delay_value)
                                .speed(0.05)
                                .range(0.0..=5.0),
                        )
                        .changed()
                    {
                        *delay = Some(delay_value);
                        draft.dirty = true;
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label("同時検出に必要なトラッカー数");
                let mut threshold = draft.value.number_trackers_over_threshold.unwrap_or(1);
                if ui
                    .add(egui::DragValue::new(&mut threshold).range(0..=20))
                    .changed()
                {
                    draft.value.number_trackers_over_threshold = Some(threshold);
                    draft.dirty = true;
                }
            });
            apply_button(ui, draft, crate::proto::set_tap_detection)
        })
    };
    if let Some(frame) = frame {
        app.send_frame(frame, "タップ検出設定");
    }
}

fn misc_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    app.drafts.hid.sync(app.settings.hid.as_ref());
    app.drafts.velocity.sync(app.settings.velocity.as_ref());

    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    ui::section(ui, "その他", |ui| {
        {
            let hid = &mut app.drafts.hid;
            let mut hid_value = hid.value;
            if ui
                .checkbox(&mut hid_value, "トラッカーを HID 経由で送信")
                .changed()
            {
                hid.value = hid_value;
                hid.dirty = true;
            }
            if hid.dirty && ui.button("HID を適用").clicked() {
                hid.dirty = false;
                frames.push(("HID 設定", crate::proto::set_hid(hid.value)));
            }
        }

        {
            let velocity = &mut app.drafts.velocity;
            let mut velocity_value = velocity.value;
            if ui
                .checkbox(&mut velocity_value, "算出した速度を送信")
                .changed()
            {
                velocity.value = velocity_value;
                velocity.dirty = true;
            }
            if velocity.dirty && ui.button("速度を適用").clicked() {
                velocity.dirty = false;
                frames.push(("速度設定", crate::proto::set_velocity(velocity.value)));
            }
        }

        ui.add_space(4.0);
        ui.label(RichText::new("VRM 設定 JSON").color(theme::MUTED).small());
        let vrm_text = ui.add(
            egui::TextEdit::multiline(&mut app.drafts.vrm.value)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("{ ... }"),
        );
        if vrm_text.changed() {
            app.drafts.vrm_dirty = true;
        }
        if app.drafts.vrm_dirty && ui.button("VRM JSON を適用").clicked() {
            app.drafts.vrm_dirty = false;
            frames.push((
                "VRM 設定",
                crate::proto::set_vrm_json(&app.drafts.vrm.value),
            ));
        }
    });

    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

fn vrc_config_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let Some(mut vrc) = app.vrc.clone() else {
        ui::section(ui, "VRChat 設定チェック", |ui| {
            ui.label(RichText::new("未取得").color(theme::MUTED));
            if ui.button("取得").clicked() {
                let frame = crate::proto::vrc_config_state_request();
                app.send_frame(frame, "VRChat 設定取得");
            }
        });
        return;
    };

    let mut toggle: Option<String> = None;
    ui::section(ui, "VRChat 設定チェック", |ui| {
        if !vrc.is_supported {
            ui.label(RichText::new("VRChat の設定ファイルが見つかりません").color(theme::WARN));
            return;
        }
        if vrc.issues.is_empty() {
            ui.label(RichText::new("問題は見つかりませんでした").color(theme::OK));
        } else {
            for issue in &vrc.issues {
                ui.label(
                    RichText::new(format!("・{issue} が推奨値ではありません")).color(theme::WARN),
                );
            }
        }
        if !vrc.values.is_empty() {
            ui.add_space(4.0);
            egui::Grid::new("vrc-values")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("項目").color(theme::MUTED));
                    ui.label(RichText::new("現在").color(theme::MUTED));
                    ui.label(RichText::new("推奨").color(theme::MUTED));
                    ui.end_row();
                    for (index, (key, value)) in vrc.values.iter().enumerate() {
                        let recommended = vrc
                            .recommended
                            .get(index)
                            .map(|(_, value)| value.clone())
                            .unwrap_or_default();
                        ui.label(key);
                        ui.label(value);
                        ui.label(recommended);
                        ui.end_row();
                    }
                });
        }
        if !vrc.muted.is_empty() {
            ui.add_space(4.0);
            ui.label(
                RichText::new("ミュート中の警告")
                    .color(theme::MUTED)
                    .small(),
            );
            for key in &vrc.muted {
                ui.horizontal(|ui| {
                    if ui.button(format!("{key} を解除")).clicked() {
                        toggle = Some(key.clone());
                    }
                });
            }
        }
        if ui.button("再取得").clicked() {
            let frame = crate::proto::vrc_config_state_request();
            app.send_frame(frame, "VRChat 設定取得");
        }
    });

    if let Some(key) = toggle {
        let frame = crate::proto::vrc_toggle_mute(&key);
        app.send_frame(frame, "VRChat 警告のミュート切替");
        vrc.muted.retain(|muted| muted != &key);
        app.vrc = Some(vrc);
    }
}
