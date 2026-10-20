//! Folding decoded server messages into UI state.

use crate::app::SlimeVrApp;
use crate::model::ServerEvent;
use crate::toast::Severity;

impl SlimeVrApp {
    pub(crate) fn apply_events(&mut self, events: Vec<ServerEvent>) {
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
}
