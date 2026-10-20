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

use std::time::{Duration, Instant};

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Headless check of the bundled server: no window, just prove that the
    // jar next to the executable starts under the bundled JRE.
    if std::env::args().any(|arg| arg == "--start-server") {
        std::process::exit(start_server_check());
    }

    let smoke_test = std::env::args().any(|arg| arg == "--smoke-test");

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
                app.arm_smoke_test(Duration::from_millis(2600));
            }
            Ok(Box::new(app))
        }),
    )
}

/// Start `slimevr.jar` from the executable's directory, wait for the
/// WebSocket port, then shut it down again. Used by CI to verify the bundle.
fn start_server_check() -> i32 {
    use std::net::{Ipv4Addr, SocketAddr, TcpStream};
    use std::time::Duration;

    let mut server = server::ServerProcess::default();
    server.detect(None, None);
    if let Err(reason) = server.start() {
        eprintln!("server-start-failed: {reason}");
        eprintln!(
            "jar={} java={}",
            server.jar_display(),
            server.java_display()
        );
        return 1;
    }

    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 21110));
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        server.drain_logs();
        if TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok() {
            println!("server-start-ok ws://127.0.0.1:21110");
            println!("jar={}", server.jar_display());
            println!("java={}", server.java_display());
            server.stop();
            return 0;
        }
        if !server.is_running() {
            eprintln!("server-start-failed: プロセスが終了しました");
            for line in &server.logs {
                eprintln!("  {line}");
            }
            return 1;
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    eprintln!("server-start-failed: ポート 21110 が開きませんでした");
    for line in &server.logs {
        eprintln!("  {line}");
    }
    server.stop();
    1
}
