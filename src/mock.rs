#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Trackers,
    Calibration,
    Settings,
}

impl Tab {
    pub const ALL: [(Tab, &'static str); 4] = [
        (Tab::Dashboard, "Dashboard"),
        (Tab::Trackers, "Trackers"),
        (Tab::Calibration, "Calibration"),
        (Tab::Settings, "Settings"),
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackerStatus {
    Online,
    Charging,
    Disconnected,
}

impl TrackerStatus {
    pub fn label(self) -> &'static str {
        match self {
            TrackerStatus::Online => "Online",
            TrackerStatus::Charging => "Charging",
            TrackerStatus::Disconnected => "Disconnected",
        }
    }
}

pub struct Tracker {
    pub id: u8,
    pub name: &'static str,
    pub role: &'static str,
    pub battery: u8,
    pub rssi: i8,
    pub tps: u16,
    pub status: TrackerStatus,
}

impl Tracker {
    pub fn online(&self) -> bool {
        self.status != TrackerStatus::Disconnected
    }
}

pub struct MockServer {
    pub running: bool,
    pub port: u16,
    pub trackers: Vec<Tracker>,
    pub uptime: f64,
}

impl MockServer {
    pub fn new() -> Self {
        Self {
            running: true,
            port: 6969,
            trackers: vec![
                Tracker {
                    id: 0,
                    name: "Chest",
                    role: "Chest",
                    battery: 91,
                    rssi: -48,
                    tps: 100,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 1,
                    name: "Hip",
                    role: "Hip",
                    battery: 84,
                    rssi: -55,
                    tps: 100,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 2,
                    name: "L-Thigh",
                    role: "LeftUpperLeg",
                    battery: 67,
                    rssi: -71,
                    tps: 98,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 3,
                    name: "L-Shin",
                    role: "LeftLowerLeg",
                    battery: 22,
                    rssi: -82,
                    tps: 94,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 4,
                    name: "L-Foot",
                    role: "LeftFoot",
                    battery: 100,
                    rssi: -60,
                    tps: 100,
                    status: TrackerStatus::Charging,
                },
                Tracker {
                    id: 5,
                    name: "R-Thigh",
                    role: "RightUpperLeg",
                    battery: 76,
                    rssi: -63,
                    tps: 99,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 6,
                    name: "R-Shin",
                    role: "RightLowerLeg",
                    battery: 58,
                    rssi: -66,
                    tps: 97,
                    status: TrackerStatus::Online,
                },
                Tracker {
                    id: 7,
                    name: "R-Foot",
                    role: "RightFoot",
                    battery: 12,
                    rssi: -89,
                    tps: 0,
                    status: TrackerStatus::Disconnected,
                },
            ],
            uptime: 12.0 * 60.0 + 42.0,
        }
    }

    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.running {
            self.uptime += f64::from(dt);
        }
    }

    pub fn online_count(&self) -> usize {
        self.trackers
            .iter()
            .filter(|tracker| tracker.online())
            .count()
    }

    pub fn average_battery(&self) -> u8 {
        let (sum, count) = self
            .trackers
            .iter()
            .filter(|tracker| tracker.online())
            .fold((0u32, 0u32), |(sum, count), tracker| {
                (sum + u32::from(tracker.battery), count + 1)
            });

        (sum.checked_div(count).unwrap_or(0)) as u8
    }

    pub fn reset_tracker(&mut self, id: u8) {
        if let Some(tracker) = self.trackers.iter_mut().find(|tracker| tracker.id == id) {
            tracker.battery = 100;
            tracker.rssi = -40;
            tracker.tps = 100;
            tracker.status = TrackerStatus::Online;
        }
    }

    pub fn uptime_label(&self) -> String {
        let total = self.uptime as u64;
        format!(
            "{:02}:{:02}:{:02}",
            total / 3600,
            (total / 60) % 60,
            total % 60
        )
    }
}

pub struct Calibration {
    pub active: bool,
    pub progress: f32,
}

impl Calibration {
    pub fn new() -> Self {
        Self {
            active: false,
            progress: 0.0,
        }
    }

    pub fn start(&mut self) {
        if self.progress >= 1.0 {
            self.progress = 0.0;
        }
        self.active = true;
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.progress = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        self.progress = (self.progress + dt * 0.35).min(1.0);
        if self.progress >= 1.0 {
            self.active = false;
        }
    }

    pub fn stage(&self) -> usize {
        ((self.progress * 3.0).floor() as usize).min(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_stop_toggle_server_state() {
        let mut server = MockServer::new();
        server.stop();
        assert!(!server.running);

        server.start();
        assert!(server.running);
    }

    #[test]
    fn counts_only_connected_trackers() {
        let server = MockServer::new();
        assert_eq!(server.online_count(), 7);
        assert_eq!(server.average_battery(), 71);
    }

    #[test]
    fn resetting_a_tracker_restores_it() {
        let mut server = MockServer::new();
        server.reset_tracker(7);

        let tracker = &server.trackers[7];
        assert_eq!(tracker.battery, 100);
        assert_eq!(tracker.status, TrackerStatus::Online);
        assert_eq!(server.online_count(), 8);
    }

    #[test]
    fn calibration_progress_stops_at_one() {
        let mut calibration = Calibration::new();
        calibration.start();
        for _ in 0..100 {
            calibration.tick(0.1);
        }

        assert_eq!(calibration.progress, 1.0);
        assert!(!calibration.active);
        assert_eq!(calibration.stage(), 3);
    }
}
