#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod mock;
mod theme;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let smoke_test = std::env::args().any(|arg| arg == "--smoke-test");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 620.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "SlimeVR Native UI (mock)",
        options,
        Box::new(move |cc| Ok(Box::new(app::SlimeVrApp::new(cc, smoke_test)))),
    )
}
