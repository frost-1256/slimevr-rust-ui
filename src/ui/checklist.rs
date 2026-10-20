use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::model::*;
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    let mut toggles: Vec<(u8, bool)> = Vec::new();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("セットアップ チェックリスト");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::text_button("再取得")).clicked() {
                    let frame = crate::proto::checklist_request();
                    app.send_frame(frame, "チェックリスト取得");
                }
            });
        });

        let checklist = app.checklist.clone();
        if checklist.steps.is_empty() {
            ui::placeholder(ui, "チェックリストを取得中です…");
            return;
        }

        let done = checklist.steps.iter().filter(|step| step.valid).count();
        let total = checklist.steps.len();
        // `desired_width(f32::INFINITY)` allocated an infinite rect and made
        // the tessellator panic as soon as a real checklist arrived.
        ui.add(
            egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                .desired_width(ui.available_width())
                .text(format!("{done} / {total} 項目完了")),
        );
        if checklist.remaining() == 0 {
            ui.label(RichText::new("すべての項目を満たしています").color(theme::OK));
        }
        ui.add_space(8.0);

        for step in checklist.visible() {
            theme::card(ui, |ui| {
                ui.horizontal(|ui| {
                    let (mark, color) = if step.valid {
                        ("✓", theme::OK)
                    } else if step.ignored {
                        ("—", theme::MUTED)
                    } else {
                        ("!", theme::WARN)
                    };
                    ui.label(RichText::new(mark).size(18.0).color(color));
                    ui.label(RichText::new(checklist_step_label(step.id)).strong().color(
                        if step.ignored {
                            theme::MUTED
                        } else {
                            theme::TEXT
                        },
                    ));
                    if step.optional {
                        ui.label(RichText::new("任意").small().color(theme::MUTED));
                    }
                    if step.visibility == 1 {
                        ui.label(RichText::new("問題時のみ").small().color(theme::MUTED));
                    }
                    if step.ignorable {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let label = if step.ignored {
                                "無視を解除"
                            } else {
                                "無視する"
                            };
                            if ui.add(theme::text_button(label)).clicked() {
                                toggles.push((step.id, !step.ignored));
                            }
                        });
                    }
                });
            });
            ui.add_space(4.0);
        }
    });

    for (step_id, ignore) in toggles {
        let frame = crate::proto::set_checklist_step_ignored(step_id, ignore);
        app.send_frame(
            frame,
            if ignore {
                "チェック項目を無視"
            } else {
                "無視を解除"
            },
        );
    }
}
