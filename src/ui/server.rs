use eframe::egui::{self, RichText};

use crate::app::SlimeVrApp;
use crate::config;
use crate::net::ConnState;
use crate::theme;
use crate::ui;

pub fn show(app: &mut SlimeVrApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("サーバー");
        ui.label(
            RichText::new("SlimeVR サーバー (Java) の起動と、GUI の接続先を管理します。")
                .color(theme::MUTED),
        );
        ui.add_space(8.0);

        if ui::two_columns(ui) {
            ui.columns(2, |columns| {
                connection_card(&mut columns[0], app);
                download_card(&mut columns[0], app);
                steamvr_card(&mut columns[0], app);
                process_card(&mut columns[0], app);
                client_log_card(&mut columns[1], app);
            });
        } else {
            connection_card(ui, app);
            ui.add_space(8.0);
            download_card(ui, app);
            ui.add_space(8.0);
            steamvr_card(ui, app);
            ui.add_space(8.0);
            process_card(ui, app);
            ui.add_space(8.0);
            client_log_card(ui, app);
        }
    });
}

/// Fetch `slimevr.jar` and a JRE without leaving the app. Both land in the
/// user data directory, where `ServerProcess::detect` picks them up.
fn download_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "ダウンロード", |ui| {
        ui.label(
            RichText::new(
                "同梱されていない場合や最新版を使いたい場合に、サーバー本体と Java を取得できます。",
            )
            .small()
            .color(theme::MUTED),
        );
        ui.add_space(6.0);

        let jar_installed = crate::download::installed_jar().is_file();
        let java_installed = crate::download::installed_java().is_dir();

        for (kind, installed) in [
            (crate::download::Kind::Server, jar_installed),
            (crate::download::Kind::Java, java_installed),
        ] {
            let phase = app.downloads.job_mut(kind).phase.clone();
            let busy = phase.is_busy();
            let label = if busy {
                format!("{} を取得中…", kind.label())
            } else if installed {
                format!("{} を再取得", kind.label())
            } else {
                format!("{} をダウンロード", kind.label())
            };
            let clicked = ui
                .horizontal(|ui| {
                    let response = ui.add_enabled(!busy, theme::outlined_button(label));
                    if installed {
                        ui.label(RichText::new("取得済み").small().color(theme::OK));
                    }
                    response.clicked()
                })
                .inner;
            if clicked {
                app.downloads.start(kind);
            }

            match &phase {
                crate::download::Phase::Downloading { received, total } => {
                    let text = match total {
                        Some(total) => format!(
                            "{:.1} / {:.1} MB",
                            *received as f32 / 1_048_576.0,
                            *total as f32 / 1_048_576.0
                        ),
                        None => format!("{:.1} MB", *received as f32 / 1_048_576.0),
                    };
                    let bar = match phase.progress_fraction() {
                        Some(fraction) => egui::ProgressBar::new(fraction),
                        None => egui::ProgressBar::new(0.0).animate(true),
                    };
                    ui.add(bar.text(text).desired_width(260.0));
                }
                crate::download::Phase::Unpacking => {
                    ui.label(RichText::new("展開中…").small().color(theme::MUTED));
                }
                crate::download::Phase::Failed(error) => {
                    ui.label(RichText::new(error).small().color(theme::ERROR));
                }
                _ => {}
            }
            ui.add_space(4.0);
        }

        ui.label(
            RichText::new(format!(
                "保存先: {}",
                crate::download::install_root().display()
            ))
            .monospace()
            .small()
            .color(theme::MUTED),
        );
    });
}

fn connection_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "接続", |ui| {
        ui.horizontal(|ui| {
            let (label, color) = match app.conn.state() {
                ConnState::Connected => ("接続済み", theme::OK),
                ConnState::Connecting => ("接続中", theme::WARN),
                ConnState::Disconnected => ("未接続", theme::ERROR),
            };
            ui.label(RichText::new("●").color(color));
            ui.label(RichText::new(label).color(color));
            ui.label(
                RichText::new(format!(
                    "受信 {} / 送信 {} / 破棄 {}",
                    app.conn.received_frames, app.conn.sent_frames, app.conn.dropped_frames
                ))
                .small()
                .color(theme::MUTED),
            );
        });
        if let Some(error) = &app.conn.last_error {
            ui.label(RichText::new(error).small().color(theme::ERROR));
        }

        ui.horizontal(|ui| {
            ui.label("WebSocket URL");
            ui.add(
                egui::TextEdit::singleline(&mut app.config.server_url)
                    .desired_width(240.0)
                    .hint_text("ws://127.0.0.1:21110"),
            );
            if ui.add(theme::primary_button("接続先を適用")).clicked() {
                let url = app.config.server_url.trim().to_string();
                app.conn.set_url(url);
                app.save_config();
            }
        });
        ui.horizontal(|ui| {
            ui.label("サーバーポート 21110:");
            let in_use = app.port.in_use();
            let text = if in_use { "使用中" } else { "空き" };
            ui.label(RichText::new(text).color(if in_use { theme::OK } else { theme::MUTED }));
            if ui.add(theme::text_button("確認")).clicked() {
                app.port.check();
            }
        });
        if let Some(ip) = &app.local_ip {
            ui.label(
                RichText::new(format!("サーバーが報告した IP: {ip}"))
                    .small()
                    .color(theme::MUTED),
            );
        }
        ui.label(
            RichText::new(format!("最終受信から {:.1} 秒", app.conn.silence_seconds()))
                .small()
                .color(theme::MUTED),
        );

        ui.separator();
        let mut include_bones = app.conn.include_bones;
        if theme::switch(
            ui,
            &mut include_bones,
            "データフィードで骨格 (bones) も購読",
        )
        .on_hover_text("トラッカーの算出骨格長を取得します。再購読が必要です。")
        .changed()
        {
            app.conn.include_bones = include_bones;
            app.config.include_bones = include_bones;
            app.conn.restart_data_feed();
            app.save_config();
        }
        if ui.add(theme::text_button("データフィードを再送")).clicked() {
            app.conn.restart_data_feed();
        }
    });
}

/// SteamVR 連携 (ドライバ有効化とインストール状態)。
fn steamvr_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    if app.steamvr.is_none() {
        app.refresh_steamvr();
    }

    let mut frames: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut recheck = false;
    ui::section(ui, "SteamVR 連携", |ui| {
        ui.horizontal(|ui| {
            if ui
                .add(theme::outlined_button("SteamVR ドライバを有効化"))
                .on_hover_text("サーバーにドライバ登録を要求します (SteamVR の vrpathreg を使用)")
                .clicked()
            {
                frames.push((
                    "SteamVR ドライバ有効化",
                    crate::proto::enable_steamvr_driver(),
                ));
                recheck = true;
            }
            if ui
                .add(theme::text_button("状態を再確認"))
                .on_hover_text("OpenVR のドライバ登録情報 (openvrpaths.vrpath) を読み直します")
                .clicked()
            {
                recheck = true;
            }
            if ui
                .add(theme::text_button("Linux 情報"))
                .on_hover_text("udev ルールと Wayland をサーバーに問い合わせます")
                .clicked()
            {
                frames.push((
                    "インストール情報取得",
                    crate::proto::installed_info_request(),
                ));
            }
        });

        ui.add_space(6.0);
        driver_status(ui, app.steamvr.as_ref());

        if let Some((udev, wayland)) = app.installed_info {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "udev: {} / セッション: {}",
                    if udev { "済み" } else { "未設定" },
                    if wayland { "Wayland" } else { "X11" }
                ))
                .small()
                .color(theme::MUTED),
            );
        }
    });

    if recheck {
        app.refresh_steamvr();
        app.schedule_steamvr_recheck();
    }
    for (label, frame) in frames {
        app.send_frame(frame, label);
    }
}

/// One status line plus the registered driver path.
fn driver_status(ui: &mut egui::Ui, status: Option<&crate::steamvr::DriverStatus>) {
    use crate::steamvr::DriverStatus;

    let (text, color) = match status {
        None => ("確認中…".to_string(), theme::MUTED),
        Some(DriverStatus::SteamVrNotFound) => (
            "SteamVR 未検出 (openvrpaths.vrpath がありません)".to_string(),
            theme::WARN,
        ),
        Some(DriverStatus::NotRegistered) => {
            ("SlimeVR ドライバは未登録です".to_string(), theme::ERROR)
        }
        Some(DriverStatus::Registered {
            binary: Some(_), ..
        }) => ("SlimeVR ドライバは登録済みです".to_string(), theme::OK),
        Some(DriverStatus::Registered { binary: None, .. }) => (
            "登録済みですが driver_slimevr が見つかりません".to_string(),
            theme::WARN,
        ),
        Some(DriverStatus::Error(error)) => (format!("確認エラー: {error}"), theme::ERROR),
    };
    ui.label(RichText::new(text).color(color));

    if let Some(DriverStatus::Registered { dir, binary }) = status {
        ui.label(
            RichText::new(dir.display().to_string())
                .monospace()
                .small()
                .color(theme::MUTED),
        );
        if let Some(binary) = binary {
            ui.label(
                RichText::new(binary.display().to_string())
                    .monospace()
                    .small()
                    .color(theme::MUTED),
            );
        }
    }
}

fn process_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    let mut frames: Vec<Vec<u8>> = Vec::new();
    ui::section(ui, "サーバープロセス", |ui| {
        ui::server_status_line(ui, app);
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("jar: {}", app.server.jar_display()))
                .monospace()
                .small()
                .color(theme::MUTED),
        );
        ui.label(
            RichText::new(format!("java: {}", app.server.java_display()))
                .monospace()
                .small()
                .color(theme::MUTED),
        );
        ui.hyperlink_to(
            RichText::new("計測データ (ライブ記録 / ドリフト解析用)").small(),
            "http://127.0.0.1:21111/",
        );

        ui.horizontal(|ui| {
            ui.label("サーバーディレクトリ");
            let mut dir = app
                .config
                .server_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            let response = ui.add(
                egui::TextEdit::singleline(&mut dir)
                    .desired_width(260.0)
                    .hint_text("jar を含むディレクトリ"),
            );
            if response.changed() {
                let trimmed = dir.trim();
                app.config.server_dir = if trimmed.is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(trimmed))
                };
            }
        });
        ui.horizontal(|ui| {
            if ui.add(theme::outlined_button("再検出")).clicked() {
                let dir = app.config.server_dir.clone();
                let java = app.config.java_path.clone();
                app.server.detect(dir.as_deref(), java.as_deref());
                app.save_config();
            }
            if ui.add(theme::text_button("サーバーログを消去")).clicked() {
                app.server.clear_logs();
            }
        });

        ui.add_space(6.0);
        let mut autostart = app.config.autostart_server;
        let response = ui.checkbox(&mut autostart, "起動時にサーバーを自動起動");
        if response.changed() {
            app.config.autostart_server = autostart;
            app.save_config();
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui::server_controls(ui, app);
            if ui.add(theme::text_button("設定ファイルを開く")).clicked()
                && let Some(path) = config::server_config_path(app.config.server_dir.as_deref())
                && let Some(parent) = path.parent()
            {
                let _ = std::fs::create_dir_all(parent);
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        });

        ui.add_space(6.0);
        crate::ui::log::show(
            ui,
            "server-stdout",
            &app.server.logs,
            &mut app.server_log_filter,
            240.0,
            "サーバーログはありません",
        );
    });

    for frame in frames.drain(..) {
        app.send_frame(frame, "サーバー");
    }
}

fn client_log_card(ui: &mut egui::Ui, app: &mut SlimeVrApp) {
    ui::section(ui, "GUI イベントログ", |ui| {
        crate::ui::log::show(
            ui,
            "client-log",
            app.notices.log(),
            &mut app.gui_log_filter,
            560.0,
            "まだイベントはありません",
        );
    });
}
