//! Application state and the top-level frame.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::LocalConfig;
use crate::model::*;
use crate::net::{ConnState, Connection};
use crate::server::{ServerProcess, format_mac, port_is_free};
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Dashboard,
    Trackers,
    Reset,
    Proportions,
    Checklist,
    Settings,
    Console,
    Server,
}

impl Page {
    pub const ALL: [(Page, &'static str); 8] = [
        (Page::Dashboard, "ダッシュボード"),
        (Page::Trackers, "トラッカー"),
        (Page::Reset, "リセット"),
        (Page::Proportions, "プロポーション"),
        (Page::Checklist, "チェックリスト"),
        (Page::Settings, "設定"),
        (Page::Console, "シリアル / FW"),
        (Page::Server, "サーバー"),
    ];

    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(page, _)| *page == self)
            .map(|(_, label)| *label)
            .unwrap_or("?")
    }

    /// Material Symbols name, matching `Page::ALL` order.
    pub fn icon(self) -> &'static str {
        match self {
            Page::Dashboard => "dashboard",
            Page::Trackers => "sensors",
            Page::Reset => "restart_alt",
            Page::Proportions => "straighten",
            Page::Checklist => "checklist",
            Page::Settings => "settings",
            Page::Console => "terminal",
            Page::Server => "dns",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub severity: Severity,
    pub created: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Default, Debug, Clone)]
pub struct SerialState {
    pub devices: Vec<(String, String, u8)>,
    pub log: Vec<String>,
    pub open: bool,
    pub command: String,
    pub ssid: String,
    pub password: String,
    pub provisioning_status: Option<u8>,
    pub device: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct FirmwareState {
    pub progress: HashMap<String, (i8, u8)>,
    pub url: String,
    pub digest: String,
    pub selected_devices: Vec<u8>,
    pub last_note: Option<String>,
}

pub struct SlimeVrApp {
    pub conn: Connection,
    pub config: LocalConfig,
    pub server: ServerProcess,
    pub page: Page,

    pub devices: Vec<Device>,
    pub synthetic: Vec<Tracker>,
    pub bones: Vec<Bone>,
    pub guards: Guards,
    pub settings: ServerSettings,
    pub skeleton: SkeletonConfig,
    pub checklist: Checklist,
    pub paused: bool,
    pub resets: Vec<ResetProgress>,
    pub local_ip: Option<String>,
    pub vrc: Option<VrcConfigState>,
    pub autobone: Option<AutoBoneStatus>,
    pub user_height: Option<UserHeightStatus>,
    pub installed_info: Option<(bool, bool)>,
    pub serial: SerialState,
    pub firmware: FirmwareState,

    pub drafts: ui::settings::SettingsDrafts,

    pub selected: Option<TrackerKey>,
    pub rename_buffer: String,
    pub rename_key: Option<TrackerKey>,
    pub assign_filter: String,
    pub forget_confirm: Option<(u8, Instant)>,
    pub reset_include_feet: bool,
    pub reset_include_fingers: bool,
    pub toasts: Vec<Toast>,
    pub event_log: Vec<String>,
    /// Frame times of the last second, used for the footer stats line.
    frame_times: Vec<Instant>,
    pub smoke_test: bool,
    pub smoke_deadline: Option<Instant>,
    smoke_page_timer: Instant,
    pub show_help: bool,
    pub downloads: crate::download::Downloads,
    pub ui_scale: f32,
    pub last_settings_request: Instant,
}

impl SlimeVrApp {
    pub fn new(cc: &eframe::CreationContext<'_>, smoke_test: bool) -> Self {
        let mut config = LocalConfig::load();
        if smoke_test {
            // Keep automated runs hermetic: never touch the real config file.
            config.server_url = crate::net::DEFAULT_URL.to_string();
        }

        if let Some(path) = crate::config::server_config_path(config.server_dir.as_deref()) {
            config.muted_warnings = crate::config::read_muted_warnings(&path);
        }

        let mut server = ServerProcess::default();
        server.detect(config.server_dir.as_deref(), config.java_path.as_deref());

        let ui_scale = config.ui_scale.clamp(0.75, 2.0);
        crate::theme::apply(&cc.egui_ctx);

        let mut app = Self {
            conn: Connection::new(config.server_url.clone()),
            config,
            server,
            page: Page::Dashboard,
            devices: Vec::new(),
            synthetic: Vec::new(),
            bones: Vec::new(),
            guards: Guards::default(),
            settings: ServerSettings::default(),
            skeleton: SkeletonConfig {
                bones: Default::default(),
                user_height: 0.0,
                reset: false,
            },
            checklist: Checklist::default(),
            paused: false,
            resets: Vec::new(),
            local_ip: None,
            vrc: None,
            autobone: None,
            user_height: None,
            installed_info: None,
            serial: SerialState::default(),
            firmware: FirmwareState::default(),
            drafts: Default::default(),
            selected: None,
            rename_buffer: String::new(),
            rename_key: None,
            assign_filter: String::new(),
            forget_confirm: None,
            reset_include_feet: false,
            reset_include_fingers: false,
            toasts: Vec::new(),
            event_log: Vec::new(),
            frame_times: Vec::new(),
            smoke_test,
            smoke_deadline: None,
            smoke_page_timer: Instant::now(),
            show_help: false,
            downloads: crate::download::Downloads::default(),
            ui_scale,
            last_settings_request: Instant::now(),
        };
        app.conn.include_bones = app.config.include_bones;
        app
    }

    // -- helpers -----------------------------------------------------------

    pub fn toast(&mut self, severity: Severity, text: impl Into<String>) {
        let text = text.into();
        log::info!("{text}");
        self.event_log
            .push(format!("[{}] {}", chrono_like_stamp(), text));
        let excess = self.event_log.len().saturating_sub(500);
        if excess > 0 {
            self.event_log.drain(..excess);
        }
        if severity != Severity::Info {
            self.toasts.push(Toast {
                text,
                severity,
                created: Instant::now(),
            });
        }
    }

    /// Physical trackers reported by the data feed. Computed (synthetic)
    /// trackers are intentionally excluded: they exist even with no hardware,
    /// and the Electron GUI keeps them out of the tracker lists too.
    pub fn all_trackers(&self) -> Vec<&Tracker> {
        self.devices
            .iter()
            .flat_map(|device| device.trackers.iter())
            .collect()
    }

    /// Server-side computed trackers (proportions preview and such).
    pub fn computed_trackers(&self) -> &[Tracker] {
        &self.synthetic
    }

    pub fn device_of(&self, key: TrackerKey) -> Option<&Device> {
        let id = key.device_id?;
        self.devices.iter().find(|device| device.id == id)
    }

    pub fn connected_tracker_count(&self) -> usize {
        self.all_trackers()
            .iter()
            .filter(|tracker| tracker.status.is_connected())
            .count()
    }

    pub fn assigned_tracker_count(&self) -> usize {
        self.all_trackers()
            .iter()
            .filter(|tracker| tracker.body_part != 0)
            .count()
    }

    pub fn average_battery(&self) -> Option<f32> {
        let values: Vec<f32> = self
            .devices
            .iter()
            .filter_map(|device| device.battery_pct.map(f32::from))
            .collect();
        if values.is_empty() {
            None
        } else {
            Some(values.iter().sum::<f32>() / values.len() as f32)
        }
    }

    pub fn fps(&self) -> f32 {
        let now = Instant::now();
        let recent = self
            .frame_times
            .iter()
            .filter(|time| now.duration_since(**time) < Duration::from_secs(1))
            .count();
        recent as f32
    }

    pub fn active_reset(&self) -> Option<&ResetProgress> {
        self.resets.iter().find(|reset| reset.started)
    }

    // -- networking --------------------------------------------------------

    pub fn send_frame(&mut self, frame: Vec<u8>, label: &str) {
        if self.conn.state() == ConnState::Connected {
            self.conn.send(frame);
            log::debug!("sent {label}");
        } else {
            self.conn.send(frame);
            self.toast(
                Severity::Warning,
                format!("{label}: サーバー未接続のため送信できません"),
            );
        }
    }

    pub fn request_settings(&mut self) {
        self.last_settings_request = Instant::now();
        self.send_frame(crate::proto::settings_request(), "設定取得");
    }

    /// Queue a server start; only missing files fail synchronously, everything
    /// else (port, Java version, spawn) is reported by `poll_server`.
    pub fn start_server(&mut self) {
        if let Err(error) = self.server.start() {
            self.toast(Severity::Error, error);
        }
    }

    pub fn stop_server(&mut self) {
        self.server.stop();
    }

    // -- events ------------------------------------------------------------

    /// Fold supervisor events into toasts; the supervisor itself never blocks.
    fn poll_server(&mut self) {
        use crate::server::State;
        let before = self.server.state();
        let before_error = self.server.last_error.clone();
        self.server.poll();
        let after = self.server.state();
        if matches!(after, State::Running(_)) && !matches!(before, State::Running(_)) {
            self.toast(Severity::Info, "サーバーを起動しました");
        }
        if after == State::Stopped && matches!(before, State::Running(_) | State::Stopping) {
            self.toast(Severity::Info, "サーバーを停止しました");
        }
        if self.server.last_error != before_error
            && let Some(error) = self.server.last_error.clone()
        {
            self.toast(Severity::Error, format!("サーバー: {error}"));
        }
    }

    /// Fold finished downloads into the server panel: re-detect the paths so a
    /// freshly downloaded jar/JRE is used without restarting the app.
    fn poll_downloads(&mut self) {
        for (kind, phase) in self.downloads.poll() {
            match phase {
                crate::download::Phase::Done(path) => {
                    let dir = self.config.server_dir.clone();
                    let java = self.config.java_path.clone();
                    self.server.detect(dir.as_deref(), java.as_deref());
                    let note = if self.server.is_running() {
                        "（次回サーバー起動時に反映されます）"
                    } else {
                        ""
                    };
                    self.toast(
                        Severity::Info,
                        format!("{} を取得しました: {}{note}", kind.label(), path.display()),
                    );
                }
                crate::download::Phase::Failed(error) => {
                    self.toast(Severity::Error, format!("{}: {error}", kind.label()));
                }
                _ => {}
            }
        }
    }

    fn apply_events(&mut self, events: Vec<ServerEvent>) {
        for event in events {
            match event {
                ServerEvent::Feed(feed) => {
                    self.devices = feed.devices;
                    self.synthetic = feed.synthetic;
                    self.bones = feed.bones;
                    if let Some(guards) = feed.guards {
                        self.guards = guards;
                    }
                    if let Some(selected) = self.selected
                        && !self
                            .all_trackers()
                            .iter()
                            .any(|tracker| tracker.key == selected)
                    {
                        self.selected = None;
                    }
                }
                ServerEvent::Settings(settings) => {
                    self.settings.merge(settings);
                    self.drafts.sync(&self.settings);
                }
                ServerEvent::Reset(reset) => {
                    if reset.started {
                        if let Some(existing) = self
                            .resets
                            .iter_mut()
                            .find(|entry| entry.reset_type == reset.reset_type)
                        {
                            *existing = reset;
                        } else {
                            self.resets.push(reset);
                        }
                    } else {
                        self.resets
                            .retain(|entry| entry.reset_type != reset.reset_type);
                        if let Some(reset_type) = reset.reset_type {
                            self.toast(
                                Severity::Success,
                                format!("{} が完了しました", reset_type.label()),
                            );
                        }
                    }
                }
                ServerEvent::PauseState(paused) => {
                    let changed = self.paused != paused;
                    self.paused = paused;
                    if changed {
                        self.toast(
                            Severity::Info,
                            if paused {
                                "トラッキングを一時停止しました"
                            } else {
                                "トラッキングを再開しました"
                            },
                        );
                    }
                }
                ServerEvent::Skeleton(skeleton) => {
                    self.skeleton = skeleton;
                }
                ServerEvent::Checklist(checklist) => {
                    self.checklist = checklist;
                }
                ServerEvent::ServerInfos { local_ip } => {
                    self.local_ip = Some(local_ip);
                }
                ServerEvent::MagToggle { key, enabled } => {
                    self.toast(
                        Severity::Info,
                        format!(
                            "{} の磁気補正: {}",
                            key.label(),
                            if enabled { "有効" } else { "無効" }
                        ),
                    );
                }
                ServerEvent::VrcConfig(vrc) => {
                    self.vrc = Some(vrc);
                }
                ServerEvent::AutoBone(status) => {
                    self.autobone = Some(status);
                }
                ServerEvent::UserHeight(status) => {
                    let finished = status.status == 2;
                    self.user_height = Some(status);
                    if finished {
                        self.toast(Severity::Success, "身長キャリブレーション完了");
                    }
                }
                ServerEvent::SerialUpdate {
                    log,
                    closed,
                    device,
                } => {
                    self.serial.log.extend(log);
                    if closed {
                        self.serial.open = false;
                        self.serial.device = None;
                        self.toast(Severity::Info, "シリアル接続を閉じました");
                    }
                    if device.is_some() {
                        self.serial.device = device;
                    }
                    let excess = self.serial.log.len().saturating_sub(2000);
                    if excess > 0 {
                        self.serial.log.drain(..excess);
                    }
                }
                ServerEvent::WifiProvisioning(status) => {
                    self.serial.provisioning_status = Some(status);
                }
                ServerEvent::InstalledInfo {
                    udev_installed,
                    wayland,
                } => {
                    self.installed_info = Some((udev_installed, wayland));
                }
                ServerEvent::FirmwareUpdate {
                    progress,
                    status,
                    device,
                } => {
                    let key = device.unwrap_or_else(|| "不明".to_string());
                    self.firmware.progress.insert(key, (progress, status));
                }
                ServerEvent::NewSerialDevice { port, name, kind } => {
                    self.serial.devices.retain(|entry| entry.0 != port);
                    self.serial.devices.push((port, name, kind));
                }
                ServerEvent::UnknownRpc(kind) => {
                    log::trace!("unhandled RPC message type {kind}");
                }
                ServerEvent::Malformed(error) => {
                    log::warn!("malformed frame: {error}");
                    self.toast(Severity::Warning, format!("不正なフレーム: {error}"));
                }
            }
        }
    }

    // -- actions used by the pages ----------------------------------------

    pub fn assign_tracker(&mut self, key: TrackerKey, body_part: u8, name: Option<String>) {
        let drift = self
            .all_trackers()
            .iter()
            .find(|tracker| tracker.key == key)
            .filter(|tracker| tracker.is_imu)
            .map(|tracker| tracker.allow_drift_compensation)
            .unwrap_or(false);
        let frame = crate::proto::assign_tracker(
            key,
            body_part,
            name.as_deref().filter(|name| !name.is_empty()),
            drift,
        );
        let label = if body_part == 0 {
            "割り当て解除".to_string()
        } else {
            format!("割り当て ({})", body_part_label(body_part))
        };
        self.send_frame(frame, &label);
    }

    pub fn forget_device(&mut self, device_id: u8) {
        let Some(device) = self.devices.iter().find(|device| device.id == device_id) else {
            return;
        };
        let Some(mac) = device.mac_address.map(format_mac) else {
            self.toast(
                Severity::Warning,
                "MAC アドレスが取得できないため削除できません",
            );
            return;
        };
        let frame = crate::proto::forget_device(&mac);
        self.send_frame(frame, &format!("デバイス削除 ({mac})"));
    }

    pub fn start_reset(&mut self, reset_type: ResetType, body_parts: &[u8]) {
        let allowed = match reset_type {
            ResetType::Yaw => self.guards.can_do_yaw_reset,
            ResetType::Full => true,
            ResetType::Mounting => self.guards.can_do_mounting,
        };
        if !allowed {
            self.toast(
                Severity::Warning,
                format!("{} は現在実行できません", reset_type.label()),
            );
            return;
        }
        let frame = crate::proto::reset(reset_type, body_parts);
        self.send_frame(frame, reset_type.label());
    }

    pub fn set_paused(&mut self, paused: bool) {
        let frame = crate::proto::set_pause_tracking(paused);
        self.send_frame(frame, "トラッキング一時停止");
    }

    pub fn save_config(&mut self) {
        self.config.server_url = self.conn.url().to_string();
        self.config.ui_scale = self.ui_scale;
        self.config.save();
    }

    /// Close the window after `after` so CI can run the binary as a smoke test.
    pub fn arm_smoke_test(&mut self, after: Duration) {
        self.smoke_deadline = Some(Instant::now() + after);
    }
}

fn chrono_like_stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs();
    let millis = now.subsec_millis();
    format!("{}.{:03}", seconds % 100_000, millis)
}

impl eframe::App for SlimeVrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctx = ctx.clone();
        let now = Instant::now();
        self.frame_times
            .retain(|time| now.duration_since(*time) < Duration::from_secs(2));
        self.frame_times.push(now);

        let previous_scale = self.ui_scale;
        // Zoom multiplies the native DPI instead of replacing it; setting
        // pixels_per_point every frame ignored the monitor scale and made
        // text blurry/resize mid-frame on Windows.
        if (ctx.zoom_factor() - self.ui_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.ui_scale);
        }

        self.poll_server();
        if self.server.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
        let events = self.conn.poll(&ctx);
        self.apply_events(events);
        self.toasts
            .retain(|toast| toast.created.elapsed() < Duration::from_secs(6));
        self.poll_downloads();

        // Re-request settings once in a while so a GUI started before the server
        // still converges without the user touching anything.
        if self.conn.state() == ConnState::Connected
            && self.last_settings_request.elapsed() > Duration::from_secs(30)
        {
            self.request_settings();
        }

        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(&ctx, |ui| ui::header(self, ui));
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(232.0)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::BG)
                    .inner_margin(egui::Margin::symmetric(8, 8)),
            )
            .show(&ctx, |ui| ui::nav(self, ui));
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(&ctx, |ui| ui::footer(self, ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::BG)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(&ctx, |ui| match self.page {
                Page::Dashboard => ui::dashboard::show(self, ui),
                Page::Trackers => ui::trackers::show(self, ui),
                Page::Reset => ui::reset::show(self, ui),
                Page::Proportions => ui::proportions::show(self, ui),
                Page::Checklist => ui::checklist::show(self, ui),
                Page::Settings => ui::settings::show(self, ui),
                Page::Console => ui::console::show(self, ui),
                Page::Server => ui::server::show(self, ui),
            });

        if self.ui_scale != previous_scale {
            self.config.ui_scale = self.ui_scale;
            self.save_config();
        }

        if self.downloads.any_running() {
            ctx.request_repaint();
        }

        if self.smoke_test {
            // Walk every page so a headless CI run lays out all widgets and
            // surfaces missing glyphs or panics that only appear off-dashboard.
            if self.smoke_page_timer.elapsed() >= Duration::from_millis(280) {
                self.smoke_page_timer = Instant::now();
                let index = Page::ALL
                    .iter()
                    .position(|(page, _)| *page == self.page)
                    .unwrap_or(0);
                self.page = Page::ALL[(index + 1) % Page::ALL.len()].0;
                log::info!("smoke page: {}", self.page.label());
            }
            // Give the socket a moment, then exit successfully.
            ctx.request_repaint_after(Duration::from_millis(50));
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                self.smoke_deadline = None;
                println!("smoke-ok");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

/// Convenience for pages that need to know whether the server link is usable.
pub fn connected(app: &SlimeVrApp) -> bool {
    app.conn.state() == ConnState::Connected
}

pub fn port_status_text() -> String {
    if port_is_free(21110) {
        "空き".to_string()
    } else {
        "使用中".to_string()
    }
}
