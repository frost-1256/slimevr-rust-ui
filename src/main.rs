#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod download;
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
    install_panic_hook();

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
    let mut failed = false;
    while Instant::now() < deadline {
        server.poll();
        if server.is_running()
            && TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
        {
            println!("server-start-ok ws://127.0.0.1:21110");
            println!("jar={}", server.jar_display());
            println!("java={}", server.java_display());
            server.shutdown();
            return 0;
        }
        if server.state() == server::State::Stopped && !server.is_running() {
            failed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    if failed {
        eprintln!("server-start-failed: プロセスが終了しました");
    } else {
        eprintln!("server-start-failed: ポート 21110 が開きませんでした");
    }
    if let Some(error) = &server.last_error {
        eprintln!("  {error}");
    }
    for line in &server.logs {
        eprintln!("  {line}");
    }
    server.shutdown();
    1
}

/// Keep a backtrace of any UI panic in the data directory; a Windows GUI
/// build has no console to print it to.
fn install_panic_hook() {
    use std::io::Write;

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let report = format!(
            "--- {} ---\npanic at {location}\n{info}\n{}\n",
            chrono_like_stamp(),
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{report}");
        let path = download::install_root().join("crash.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = file.write_all(report.as_bytes());
        }
        previous(info);
    }));
}

fn chrono_like_stamp() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => format!("{}.{:03}", elapsed.as_secs(), elapsed.subsec_millis()),
        Err(_) => "0.000".to_string(),
    }
}
