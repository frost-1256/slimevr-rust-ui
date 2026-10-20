#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod model;
mod net;
mod proto;
mod server;
mod theme;
mod ui;

use std::time::Duration;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let smoke_test = std::env::args().any(|arg| arg == "--smoke-test");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([980.0, 620.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "SlimeVR Native UI",
        options,
        Box::new(move |cc| {
            let mut app = app::SlimeVrApp::new(cc, smoke_test);
            if smoke_test {
                app.arm_smoke_test(Duration::from_secs(3));
            }
            Ok(Box::new(app))
        }),
    )
}
