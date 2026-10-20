//! WebSocket client for the SlimeVR server protocol.
//!
//! The socket runs on `ewebsock`'s own thread and wakes the egui context on every
//! event, so the UI never blocks. Reconnect policy mirrors the Electron GUI:
//! retry every 3 s, and treat 10 s of silence on an open socket as a dead link.

use std::time::{Duration, Instant};

use eframe::egui::Context;
use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};

use crate::model::ServerEvent;
use crate::proto;

pub const DEFAULT_URL: &str = "ws://127.0.0.1:21110";
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
const SILENCE_TIMEOUT: Duration = Duration::from_secs(10);
/// Data feed interval requested from the server, in milliseconds.
pub const FEED_INTERVAL_MS: u16 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct Connection {
    url: String,
    receiver: Option<WsReceiver>,
    sender: Option<WsSender>,
    state: ConnState,
    pub last_error: Option<String>,
    next_attempt: Instant,
    last_message: Option<Instant>,
    /// Include computed tracker bones in the data feed.
    pub include_bones: bool,
    /// Frames skipped because the socket is down.
    pub dropped_frames: u64,
    pub received_frames: u64,
    pub sent_frames: u64,
    pub last_event_note: Option<String>,
}

impl Connection {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            receiver: None,
            sender: None,
            state: ConnState::Disconnected,
            last_error: None,
            next_attempt: Instant::now(),
            last_message: None,
            include_bones: true,
            dropped_frames: 0,
            received_frames: 0,
            sent_frames: 0,
            last_event_note: None,
        }
    }

    pub fn state(&self) -> ConnState {
        self.state
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        let url = url.into();
        if url == self.url {
            return;
        }
        self.url = url;
        self.sender = None;
        self.receiver = None;
        self.state = ConnState::Disconnected;
        self.next_attempt = Instant::now();
    }

    /// Drop the current socket and reconnect on the next poll.
    pub fn reconnect(&mut self) {
        self.sender = None;
        self.receiver = None;
        self.state = ConnState::Disconnected;
        self.next_attempt = Instant::now();
        self.last_error = None;
    }

    fn ensure_connected(&mut self, ctx: &Context) {
        if self.state != ConnState::Disconnected || Instant::now() < self.next_attempt {
            return;
        }

        self.state = ConnState::Connecting;
        let wake = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        match ewebsock::connect_with_wakeup(self.url.clone(), Options::default(), wake) {
            Ok((sender, receiver)) => {
                self.sender = Some(sender);
                self.receiver = Some(receiver);
                tracing_note(&mut self.last_event_note, format!("接続試行: {}", self.url));
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                self.state = ConnState::Disconnected;
                self.next_attempt = Instant::now() + RECONNECT_DELAY;
            }
        }
    }

    /// Drain socket events and return the decoded server messages.
    pub fn poll(&mut self, ctx: &Context) -> Vec<ServerEvent> {
        self.ensure_connected(ctx);

        // Take the receiver out for the duration of the drain so the handlers can
        // borrow `self` mutably.
        let Some(receiver) = self.receiver.take() else {
            return Vec::new();
        };
        let mut events = Vec::new();
        let mut keep = true;
        while let Some(event) = receiver.try_recv() {
            match event {
                WsEvent::Opened => {
                    self.state = ConnState::Connected;
                    self.last_message = Some(Instant::now());
                    self.last_error = None;
                    tracing_note(&mut self.last_event_note, "接続完了".to_string());
                    self.send(proto::start_data_feed(FEED_INTERVAL_MS, self.include_bones));
                    self.send(proto::settings_request());
                    self.send(proto::skeleton_config_request());
                    self.send(proto::pause_state_request());
                    self.send(proto::server_infos_request());
                    self.send(proto::checklist_request());
                }
                WsEvent::Message(WsMessage::Binary(bytes)) => {
                    self.last_message = Some(Instant::now());
                    self.received_frames += 1;
                    events.extend(proto::decode(&bytes));
                }
                WsEvent::Message(_) => {
                    // The server emits a one-off JSON text frame; everything else
                    // on this socket is binary SolarXR.
                }
                WsEvent::Error(error) => {
                    self.last_error = Some(error);
                    self.state = ConnState::Disconnected;
                    self.sender = None;
                    self.next_attempt = Instant::now() + RECONNECT_DELAY;
                    keep = false;
                    break;
                }
                WsEvent::Closed => {
                    self.state = ConnState::Disconnected;
                    self.sender = None;
                    self.next_attempt = Instant::now() + RECONNECT_DELAY;
                    tracing_note(
                        &mut self.last_event_note,
                        "接続が閉じられました".to_string(),
                    );
                    keep = false;
                    break;
                }
            }
        }
        if keep {
            self.receiver = Some(receiver);
        }

        if self.state == ConnState::Connected
            && let Some(last) = self.last_message
            && last.elapsed() > SILENCE_TIMEOUT
        {
            tracing_note(
                &mut self.last_event_note,
                "10秒間データ無し: 再接続します".to_string(),
            );
            self.reconnect();
        }

        events
    }

    /// Seconds since the last frame from the server, or since connect.
    pub fn silence_seconds(&self) -> f32 {
        self.last_message
            .map(|last| last.elapsed().as_secs_f32())
            .unwrap_or(0.0)
    }

    /// Send a frame. While the socket is down the frame is queued for the next
    /// handshake instead of silently vanishing.
    pub fn send(&mut self, bytes: Vec<u8>) {
        match (self.state, self.sender.as_mut()) {
            (ConnState::Connected, Some(sender)) => {
                sender.send(WsMessage::Binary(bytes));
                self.sent_frames += 1;
            }
            _ => {
                // Not connected: drop the frame. Requests are user actions and
                // applying them minutes later, after a reconnect, would be worse
                // than losing them.
                self.dropped_frames += 1;
            }
        }
    }

    /// Re-send the data feed registration, e.g. after toggling bone output.
    pub fn restart_data_feed(&mut self) {
        self.send(proto::start_data_feed(FEED_INTERVAL_MS, self.include_bones));
    }
}

fn tracing_note(slot: &mut Option<String>, message: String) {
    log::debug!("{message}");
    *slot = Some(message);
}
