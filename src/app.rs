use crate::mock::{Calibration, MockServer, Tab, Tracker};
use crate::theme;
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use std::time::{Duration, Instant};

pub struct SlimeVrApp {
    tab: Tab,
    server: MockServer,
    calibration: Calibration,
    settings: Settings,
    smoke_deadline: Option<Instant>,
}

struct Settings {
    port: u16,
    osc: bool,
    vmc: bool,
    steamvr: bool,
    auto_bone: bool,
    smoothing: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 6969,
            osc: true,
            vmc: false,
            steamvr: true,
            auto_bone: true,
            smoothing: 0.65,
        }
    }
}

impl SlimeVrApp {
    pub fn new(cc: &eframe::CreationContext<'_>, smoke_test: bool) -> Self {
        theme::apply(&cc.egui_ctx);

        Self {
            tab: Tab::Dashboard,
            server: MockServer::new(),
            calibration: Calibration::new(),
            settings: Settings::default(),
            smoke_deadline: smoke_test.then(|| Instant::now() + Duration::from_secs(3)),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_bar")
            .exact_size(58.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(egui::Margin::symmetric(18, 9)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("SLIMEVR")
                            .size(21.0)
                            .strong()
                            .color(theme::ACCENT),
                    );
                    ui.label(
                        RichText::new("native ui · mock")
                            .size(12.0)
                            .color(theme::MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (label, color) = if self.server.running {
                            ("● Running", theme::OK)
                        } else {
                            ("● Stopped", theme::MUTED)
                        };
                        egui::Frame::new()
                            .fill(color.gamma_multiply(0.12))
                            .corner_radius(12)
                            .inner_margin(egui::Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(RichText::new(label).small().color(color));
                            });

                        let action = if self.server.running {
                            "Stop server"
                        } else {
                            "Start server"
                        };
                        if ui.button(action).clicked() {
                            if self.server.running {
                                self.server.stop();
                            } else {
                                self.server.start();
                            }
                        }

                        ui.label(
                            RichText::new(format!("127.0.0.1:{}", self.server.port))
                                .monospace()
                                .color(theme::MUTED),
                        );
                    });
                });
            });
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .resizable(false)
            .exact_size(212.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(egui::Margin::symmetric(12, 12)),
            )
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("NAVIGATION").small().color(theme::MUTED));
                ui.add_space(6.0);

                for (tab, label) in Tab::ALL {
                    let selected = self.tab == tab;
                    let text = RichText::new(label).size(15.0).color(if selected {
                        theme::ACCENT
                    } else {
                        theme::TEXT
                    });
                    let button = egui::Button::new(text)
                        .fill(if selected {
                            theme::SURFACE_ALT
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if selected {
                            Stroke::new(1.0, theme::ACCENT_DIM)
                        } else {
                            Stroke::NONE
                        })
                        .min_size(Vec2::new(ui.available_width(), 38.0));

                    if ui.add(button).clicked() {
                        self.tab = tab;
                    }
                    ui.add_space(4.0);
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("rust + egui").small().color(theme::MUTED));
                    ui.label(
                        RichText::new("protocol: not connected")
                            .small()
                            .color(theme::MUTED),
                    );
                });
            });
    }

    fn bottom_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar")
            .exact_size(28.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(egui::Margin::symmetric(14, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("v0.1.0 · mock data · Windows x64")
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("trackers: 2.4 GHz")
                                .small()
                                .color(theme::MUTED),
                        );
                    });
                });
            });
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Dashboard");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("mock telemetry").small().color(theme::MUTED));
            });
        });
        ui.add_space(2.0);
        ui.label(
            RichText::new(
                "This screen uses fixture data. The server protocol is not wired up yet.",
            )
            .color(theme::MUTED),
        );
        ui.add_space(14.0);

        let online = self.server.online_count();
        let total = self.server.trackers.len();
        let average_battery = self.server.average_battery();
        let uptime = self.server.uptime_label();
        let (server_state, server_color) = if self.server.running {
            ("Running", theme::OK)
        } else {
            ("Stopped", theme::MUTED)
        };

        ui.columns(4, |columns| {
            stat_card(
                &mut columns[0],
                "Trackers",
                &format!("{online}/{total}"),
                theme::ACCENT,
            );
            stat_card(
                &mut columns[1],
                "Avg. battery",
                &format!("{average_battery}%"),
                theme::battery_color(average_battery),
            );
            stat_card(&mut columns[2], "Server", server_state, server_color);
            stat_card(&mut columns[3], "Uptime", &uptime, theme::PURPLE);
        });

        ui.add_space(18.0);
        ui.label(
            RichText::new("TRACKER OVERVIEW")
                .small()
                .color(theme::MUTED),
        );
        ui.add_space(4.0);

        egui::Grid::new("dashboard_trackers")
            .num_columns(3)
            .spacing([12.0, 12.0])
            .show(ui, |ui| {
                for (index, tracker) in self.server.trackers.iter().enumerate() {
                    tracker_card(ui, tracker);
                    if (index + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn trackers_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Trackers");
        ui.label(
            RichText::new("Fixture trackers. Reset changes only the in-memory mock state.")
                .color(theme::MUTED),
        );
        ui.add_space(12.0);

        let mut reset_id = None;
        theme::card(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("tracker_table")
                        .num_columns(7)
                        .striped(true)
                        .spacing([18.0, 10.0])
                        .show(ui, |ui| {
                            for header in
                                ["ID", "Name", "Role", "Battery", "RSSI", "TPS", "Actions"]
                            {
                                ui.label(
                                    RichText::new(header).small().strong().color(theme::MUTED),
                                );
                            }
                            ui.end_row();

                            for tracker in &mut self.server.trackers {
                                ui.label(format!("#{}", tracker.id));
                                ui.label(RichText::new(tracker.name).strong());
                                ui.label(RichText::new(tracker.role).color(theme::MUTED));

                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::ProgressBar::new(f32::from(tracker.battery) / 100.0)
                                            .desired_width(90.0)
                                            .fill(theme::battery_color(tracker.battery))
                                            .text(
                                                RichText::new(format!("{}%", tracker.battery))
                                                    .small(),
                                            ),
                                    );
                                });

                                ui.label(format!("{} dBm", tracker.rssi));
                                ui.label(format!("{}", tracker.tps));

                                ui.horizontal(|ui| {
                                    let color = theme::status_color(tracker.status.label());
                                    ui.label(RichText::new("●").color(color));
                                    ui.label(
                                        RichText::new(tracker.status.label()).small().color(color),
                                    );
                                    if ui.button("Reset").clicked() {
                                        reset_id = Some(tracker.id);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
        });

        if let Some(id) = reset_id {
            self.server.reset_tracker(id);
        }
    }

    fn calibration_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Calibration");
        ui.label(
            RichText::new("Mock flow for the mounting / stand-still / walk sequence.")
                .color(theme::MUTED),
        );
        ui.add_space(12.0);

        theme::card(ui, |ui| {
            ui.set_min_width(680.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Step {}/4", self.calibration.stage() + 1))
                        .size(18.0)
                        .strong()
                        .color(theme::ACCENT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let state = if self.calibration.active {
                        "in progress"
                    } else if self.calibration.progress >= 1.0 {
                        "complete"
                    } else {
                        "idle"
                    };
                    ui.label(RichText::new(state).small().color(theme::MUTED));
                });
            });

            ui.add_space(10.0);
            let stages = [
                "1. Mounting",
                "2. Stand still",
                "3. Walk forward",
                "4. Verify skeleton",
            ];
            for (index, stage) in stages.iter().enumerate() {
                let active = index == self.calibration.stage();
                let done = index < self.calibration.stage() || self.calibration.progress >= 1.0;
                let color = if done {
                    theme::OK
                } else if active {
                    theme::ACCENT
                } else {
                    theme::MUTED
                };
                ui.label(RichText::new(*stage).color(color));
            }

            ui.add_space(14.0);
            ui.add(
                egui::ProgressBar::new(self.calibration.progress)
                    .desired_width(640.0)
                    .show_percentage()
                    .fill(theme::ACCENT),
            );

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if self.calibration.active {
                    if ui.button("Pause").clicked() {
                        self.calibration.active = false;
                    }
                } else if ui.button("Start calibration").clicked() {
                    self.calibration.start();
                }

                if ui.button("Reset").clicked() {
                    self.calibration.reset();
                }

                ui.label(
                    RichText::new("The real calibration will be driven by the server.")
                        .small()
                        .color(theme::MUTED),
                );
            });
        });
    }

    fn settings_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.label(
            RichText::new("Local mock settings. Nothing here talks to the server yet.")
                .color(theme::MUTED),
        );
        ui.add_space(12.0);

        theme::card(ui, |ui| {
            ui.set_min_width(620.0);
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([28.0, 14.0])
                .show(ui, |ui| {
                    ui.label("Server port");
                    ui.add(egui::DragValue::new(&mut self.settings.port).range(1024..=65535));
                    ui.end_row();

                    ui.label("OSC trackers");
                    ui.checkbox(&mut self.settings.osc, "Enable OSC output");
                    ui.end_row();

                    ui.label("VMC");
                    ui.checkbox(&mut self.settings.vmc, "Enable VMC sender");
                    ui.end_row();

                    ui.label("SteamVR");
                    ui.checkbox(&mut self.settings.steamvr, "Start OpenVR driver");
                    ui.end_row();

                    ui.label("Auto-bone");
                    ui.checkbox(&mut self.settings.auto_bone, "Calculate bone lengths");
                    ui.end_row();

                    ui.label("Smoothing");
                    ui.add(
                        egui::Slider::new(&mut self.settings.smoothing, 0.0..=1.0).show_value(true),
                    );
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Reset defaults").clicked() {
                    self.settings = Settings::default();
                }
                ui.label(
                    RichText::new("settings are not persisted")
                        .small()
                        .color(theme::MUTED),
                );
            });
        });
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        match self.tab {
            Tab::Dashboard => self.dashboard(ui),
            Tab::Trackers => self.trackers_view(ui),
            Tab::Calibration => self.calibration_view(ui),
            Tab::Settings => self.settings_view(ui),
        }
    }
}

impl eframe::App for SlimeVrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dt = ui.ctx().input(|input| input.stable_dt).min(0.1);
        self.server.tick(dt);
        self.calibration.tick(dt);

        self.top_bar(ui);
        self.nav(ui);
        self.bottom_bar(ui);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ui, |ui| {
                self.content(ui);
            });

        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                self.smoke_deadline = None;
                println!("smoke-ok");
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    theme::card(ui, |ui| {
        ui.set_min_width(150.0);
        ui.label(RichText::new(label).small().color(theme::MUTED));
        ui.label(RichText::new(value).size(24.0).strong().color(color));
    });
}

fn tracker_card(ui: &mut egui::Ui, tracker: &Tracker) {
    theme::card(ui, |ui| {
        ui.set_min_width(250.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tracker.name)
                    .size(16.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let color = theme::status_color(tracker.status.label());
                ui.label(RichText::new("●").color(color));
            });
        });
        ui.label(RichText::new(tracker.role).small().color(theme::MUTED));
        ui.add_space(6.0);

        ui.add(
            egui::ProgressBar::new(f32::from(tracker.battery) / 100.0)
                .desired_width(210.0)
                .fill(theme::battery_color(tracker.battery))
                .text(RichText::new(format!("{}%", tracker.battery)).small()),
        );

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} dBm", tracker.rssi))
                    .small()
                    .color(theme::MUTED),
            );
            ui.label(
                RichText::new(format!("{} TPS", tracker.tps))
                    .small()
                    .color(theme::MUTED),
            );
        });
    });
}
