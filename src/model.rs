//! Owned snapshots of the data received from the SlimeVR server.
//!
//! The FlatBuffers bindings are zero-copy accessors, so every incoming frame is
//! immediately copied into these plain structs. Nothing in the UI holds a borrow
//! into a network buffer.

use std::collections::BTreeMap;

/// Wire `TrackerStatus` is 1-based (`server id + 1`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackerStatus {
    None,
    Disconnected,
    Ok,
    Busy,
    Error,
    Occluded,
    TimedOut,
    Unknown(u8),
}

impl TrackerStatus {
    pub fn from_wire(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Disconnected,
            2 => Self::Ok,
            3 => Self::Busy,
            4 => Self::Error,
            5 => Self::Occluded,
            6 => Self::TimedOut,
            other => Self::Unknown(other),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "未設定",
            Self::Disconnected => "切断",
            Self::Ok => "正常",
            Self::Busy => "処理中",
            Self::Error => "エラー",
            Self::Occluded => "遮蔽",
            Self::TimedOut => "タイムアウト",
            Self::Unknown(_) => "不明",
        }
    }

    pub fn is_connected(self) -> bool {
        !matches!(self, Self::None | Self::Disconnected)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TrackerKey {
    pub device_id: Option<u8>,
    pub tracker_num: u8,
}

impl TrackerKey {
    pub fn label(&self) -> String {
        match self.device_id {
            Some(device) => format!("{device}:{}", self.tracker_num),
            None => format!("computed:{}", self.tracker_num),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    /// Yaw in degrees around +Y, matching the server's `EulerOrder.YZX` yaw.
    pub fn yaw_degrees(&self) -> f32 {
        let siny = 2.0 * (self.w * self.y + self.x * self.z);
        let cosy = 1.0 - 2.0 * (self.x * self.x + self.y * self.y);
        siny.atan2(cosy).to_degrees()
    }
}

#[derive(Clone, Debug)]
pub struct Tracker {
    pub key: TrackerKey,
    pub display_name: String,
    pub custom_name: String,
    pub body_part: u8,
    pub status: TrackerStatus,
    pub is_imu: bool,
    pub is_computed: bool,
    pub is_hmd: bool,
    pub editable: bool,
    pub allow_drift_compensation: bool,
    pub magnetometer: u8,
    pub mounting_orientation: Quat,
    pub rotation: Option<Quat>,
    pub tps: Option<u16>,
    pub temp: Option<f32>,
    pub hardware_identifier: Option<String>,
}

impl Tracker {
    pub fn name(&self) -> String {
        if !self.custom_name.is_empty() {
            return self.custom_name.clone();
        }
        if self.body_part != 0 {
            return body_part_label(self.body_part).to_string();
        }
        if !self.display_name.is_empty() {
            return self.display_name.clone();
        }
        "NONE".to_string()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Device {
    pub id: u8,
    pub custom_name: String,
    pub firmware_version: Option<String>,
    pub firmware_date: Option<String>,
    pub manufacturer: Option<String>,
    pub hardware_identifier: Option<String>,
    pub ip_address: Option<String>,
    pub battery_pct: Option<u8>,
    pub battery_voltage: Option<f32>,
    pub battery_runtime_us: Option<i64>,
    pub rssi: Option<i16>,
    pub ping: Option<u16>,
    pub mcu_temp: Option<f32>,
    pub packet_loss: Option<f32>,
    pub packets_lost: Option<i32>,
    pub packets_received: Option<i32>,
    pub error_status: Option<u8>,
    pub mac_address: Option<u64>,
    pub network_protocol_version: Option<u16>,
    pub trackers: Vec<Tracker>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Guards {
    pub can_do_mounting: bool,
    pub can_do_yaw_reset: bool,
    pub can_do_user_height_calibration: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Bone {
    pub body_part: u8,
    pub bone_length: f32,
    pub head_position: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct FeedUpdate {
    pub devices: Vec<Device>,
    pub synthetic: Vec<Tracker>,
    pub bones: Vec<Bone>,
    pub guards: Option<Guards>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResetType {
    Yaw,
    Full,
    Mounting,
}

impl ResetType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Yaw => "Yaw リセット",
            Self::Full => "フルリセット",
            Self::Mounting => "マウンティング リセット",
        }
    }

    pub fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Yaw),
            1 => Some(Self::Full),
            2 => Some(Self::Mounting),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResetProgress {
    pub reset_type: Option<ResetType>,
    pub started: bool,
    pub progress_ms: u32,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SteamVrSettings {
    pub waist: bool,
    pub chest: bool,
    pub left_foot: bool,
    pub right_foot: bool,
    pub left_knee: bool,
    pub right_knee: bool,
    pub left_elbow: bool,
    pub right_elbow: bool,
    pub left_hand: bool,
    pub right_hand: bool,
    pub automatic_tracker_toggle: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FilteringSettings {
    pub filter_type: u8,
    pub amount: f32,
}

#[derive(Clone, Debug, Default)]
pub struct DriftSettings {
    pub enabled: bool,
    pub prediction: bool,
    pub amount: f32,
    pub max_resets: u16,
}

#[derive(Clone, Debug, Default)]
pub struct OscSettings {
    pub enabled: bool,
    pub port_in: u16,
    pub port_out: u16,
    pub address: String,
}

#[derive(Clone, Debug, Default)]
pub struct VrcOscSettings {
    pub osc: OscSettings,
    pub head: bool,
    pub chest: bool,
    pub waist: bool,
    pub knees: bool,
    pub feet: bool,
    pub elbows: bool,
    pub hands: bool,
    pub oscquery_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct VmcOscSettings {
    pub osc: OscSettings,
    pub anchor_hip: bool,
    pub mirror_tracking: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModelToggles {
    pub extended_spine: bool,
    pub extended_pelvis: bool,
    pub extended_knee: bool,
    pub force_arms_from_hmd: bool,
    pub floor_clip: bool,
    pub skating_correction: bool,
    pub toe_snap: bool,
    pub foot_plant: bool,
    pub self_localization: bool,
    pub use_position: bool,
    pub enforce_constraints: bool,
    pub correct_constraints: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModelRatios {
    pub impute_waist_from_chest_hip: Option<f32>,
    pub impute_waist_from_chest_legs: Option<f32>,
    pub impute_hip_from_chest_legs: Option<f32>,
    pub impute_hip_from_waist_legs: Option<f32>,
    pub interp_hip_legs: Option<f32>,
    pub interp_knee_tracker_ankle: Option<f32>,
    pub interp_knee_ankle: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct ModelSettings {
    pub toggles: ModelToggles,
    pub ratios: ModelRatios,
    pub correction_strength: Option<f32>,
    pub hmd_height: Option<f32>,
    pub floor_height: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct ResetsSettings {
    pub reset_mounting_feet: bool,
    pub arms_mounting_reset_mode: u8,
    pub yaw_reset_smooth_time: f32,
    pub save_mounting_reset: bool,
    pub reset_hmd_pitch: bool,
}

#[derive(Clone, Debug, Default)]
pub struct StayAlignedSettings {
    pub enabled: bool,
    pub hide_yaw_correction: bool,
    pub standing_enabled: bool,
    pub standing_upper_leg_angle: f32,
    pub standing_lower_leg_angle: f32,
    pub standing_foot_angle: f32,
    pub sitting_enabled: bool,
    pub sitting_upper_leg_angle: f32,
    pub sitting_lower_leg_angle: f32,
    pub sitting_foot_angle: f32,
    pub flat_enabled: bool,
    pub flat_upper_leg_angle: f32,
    pub flat_lower_leg_angle: f32,
    pub flat_foot_angle: f32,
    pub setup_complete: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TapDetectionSettings {
    pub full_reset_enabled: Option<bool>,
    pub full_reset_delay: Option<f32>,
    pub full_reset_taps: Option<u8>,
    pub yaw_reset_enabled: Option<bool>,
    pub yaw_reset_delay: Option<f32>,
    pub yaw_reset_taps: Option<u8>,
    pub mounting_reset_enabled: Option<bool>,
    pub mounting_reset_delay: Option<f32>,
    pub mounting_reset_taps: Option<u8>,
    pub number_trackers_over_threshold: Option<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct ServerSettings {
    pub steam_vr: Option<SteamVrSettings>,
    pub filtering: Option<FilteringSettings>,
    pub drift: Option<DriftSettings>,
    pub osc_router: Option<OscSettings>,
    pub vrc_osc: Option<VrcOscSettings>,
    pub vmc_osc: Option<VmcOscSettings>,
    pub model: Option<ModelSettings>,
    pub resets: Option<ResetsSettings>,
    pub stay_aligned: Option<StayAlignedSettings>,
    pub tap_detection: Option<TapDetectionSettings>,
    pub hid: Option<bool>,
    pub velocity: Option<bool>,
    pub vrm_json: Option<String>,
}

impl ServerSettings {
    /// Merge a partial update (the server broadcasts a SteamVR-only
    /// `SettingsResponse` when those change).
    pub fn merge(&mut self, other: ServerSettings) {
        macro_rules! merge_field {
            ($($field:ident),* $(,)?) => {
                $(if other.$field.is_some() { self.$field = other.$field; })*
            };
        }
        merge_field!(
            steam_vr,
            filtering,
            drift,
            osc_router,
            vrc_osc,
            vmc_osc,
            model,
            resets,
            stay_aligned,
            tap_detection,
            hid,
            velocity,
            vrm_json,
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct SkeletonConfig {
    pub bones: BTreeMap<u8, f32>,
    pub user_height: f32,
    pub reset: bool,
}

#[derive(Clone, Debug)]
pub struct ChecklistStep {
    pub id: u8,
    pub valid: bool,
    pub enabled: bool,
    pub visibility: u8,
    pub optional: bool,
    pub ignorable: bool,
    pub ignored: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Checklist {
    pub steps: Vec<ChecklistStep>,
}

impl Checklist {
    pub fn remaining(&self) -> usize {
        self.steps
            .iter()
            .filter(|step| step.enabled && !step.valid && !step.ignored)
            .count()
    }

    pub fn visible(&self) -> impl Iterator<Item = &ChecklistStep> {
        // The Electron GUI only lists steps the server marks as enabled.
        self.steps.iter().filter(|step| step.enabled)
    }
}

#[derive(Clone, Debug)]
pub enum AutoBoneStatus {
    Progress {
        process_type: u8,
        current: u32,
        total: u32,
        completed: bool,
        success: bool,
        eta: f32,
    },
    Epoch {
        current_epoch: u32,
        total_epochs: u32,
        error: f32,
    },
}

#[derive(Clone, Debug)]
pub struct UserHeightStatus {
    pub hmd_height: f32,
    pub status: u8,
}

/// Decoded server -> client messages.
#[derive(Clone, Debug)]
/// Events are short-lived and decoded once per frame; boxing variants would only
/// add indirection to every consumer.
#[allow(clippy::large_enum_variant)]
pub enum ServerEvent {
    Feed(FeedUpdate),
    Settings(ServerSettings),
    Reset(ResetProgress),
    PauseState(bool),
    Skeleton(SkeletonConfig),
    Checklist(Checklist),
    ServerInfos {
        local_ip: String,
    },
    MagToggle {
        key: TrackerKey,
        enabled: bool,
    },
    VrcConfig(VrcConfigState),
    AutoBone(AutoBoneStatus),
    UserHeight(UserHeightStatus),
    InstalledInfo {
        udev_installed: bool,
        wayland: bool,
    },
    SerialUpdate {
        log: Vec<String>,
        closed: bool,
        device: Option<String>,
    },
    WifiProvisioning(u8),
    FirmwareUpdate {
        progress: i8,
        status: u8,
        device: Option<String>,
    },
    NewSerialDevice {
        port: String,
        name: String,
        kind: u8,
    },
    UnknownRpc(u8),
    Malformed(String),
}

#[derive(Clone, Debug, Default)]
pub struct VrcConfigState {
    pub is_supported: bool,
    /// Settings that do not satisfy the VRChat requirements.
    pub issues: Vec<String>,
    pub values: Vec<(String, String)>,
    pub recommended: Vec<(String, String)>,
    pub muted: Vec<String>,
}

pub fn user_height_status_label(status: u8) -> &'static str {
    match status {
        0 => "開始待ち",
        1 => "録音中",
        2 => "完了",
        3 => "キャンセル",
        _ => "不明",
    }
}

pub fn body_part_label(body_part: u8) -> &'static str {
    match body_part {
        0 => "なし",
        1 => "頭",
        2 => "首",
        3 => "胸",
        4 => "腰",
        5 => "骨盤",
        6 => "左上腿",
        7 => "右上腿",
        8 => "左下腿",
        9 => "右下腿",
        10 => "左足",
        11 => "右足",
        14 => "左下腕",
        15 => "右下腕",
        16 => "左上腕",
        17 => "右上腕",
        18 => "左手",
        19 => "右手",
        20 => "左肩",
        21 => "右肩",
        22 => "上胸",
        23 => "左骨盤",
        24 => "右骨盤",
        25 => "左親指CM",
        26 => "左親指根元",
        27 => "左親指先",
        28 => "左人差し指根元",
        29 => "左人差し指中",
        30 => "左人差し指先",
        31 => "左中指根元",
        32 => "左中指中",
        33 => "左中指先",
        34 => "左薬指根元",
        35 => "左薬指中",
        36 => "左薬指先",
        37 => "左小指根元",
        38 => "左小指中",
        39 => "左小指先",
        40 => "右親指CM",
        41 => "右親指根元",
        42 => "右親指先",
        43 => "右人差し指根元",
        44 => "右人差し指中",
        45 => "右人差し指先",
        46 => "右中指根元",
        47 => "右中指中",
        48 => "右中指先",
        49 => "右薬指根元",
        50 => "右薬指中",
        51 => "右薬指先",
        52 => "右小指根元",
        53 => "右小指中",
        54 => "右小指先",
        _ => "不明",
    }
}

/// Roles offered in the assignment dropdown, in the same order as the Electron GUI.
pub const ASSIGNABLE_BODY_PARTS: &[(u8, &str)] = &[
    (1, "頭"),
    (2, "首"),
    (3, "胸"),
    (22, "上胸"),
    (4, "腰"),
    (5, "骨盤"),
    (23, "左骨盤"),
    (24, "右骨盤"),
    (6, "左上腿"),
    (7, "右上腿"),
    (8, "左下腿"),
    (9, "右下腿"),
    (10, "左足"),
    (11, "右足"),
    (16, "左上腕"),
    (17, "右上腕"),
    (14, "左下腕"),
    (15, "右下腕"),
    (18, "左手"),
    (19, "右手"),
    (20, "左肩"),
    (21, "右肩"),
];

pub fn filtering_type_label(filter_type: u8) -> &'static str {
    match filter_type {
        0 => "なし",
        1 => "スムージング",
        2 => "予測",
        _ => "不明",
    }
}

pub fn arms_mounting_mode_label(mode: u8) -> &'static str {
    match mode {
        0 => "上腕を後ろ・前腕を前",
        1 => "腕を前に",
        2 => "Tポーズから上げる",
        3 => "Tポーズから下ろす",
        _ => "不明",
    }
}

pub fn skeleton_bone_label(bone: u8) -> &'static str {
    match bone {
        1 => "頭",
        2 => "首",
        3 => "胸",
        4 => "胸オフセット",
        5 => "腰",
        6 => "骨盤",
        7 => "骨盤オフセット",
        8 => "骨盤幅",
        9 => "上腿",
        10 => "下腿",
        11 => "足長",
        12 => "足シフト",
        13 => "骨格オフセット",
        14 => "肩距離",
        15 => "肩幅",
        16 => "上腕",
        17 => "下腕",
        18 => "手 Y",
        19 => "手 Z",
        20 => "肘オフセット",
        21 => "上胸",
        _ => "不明",
    }
}

pub fn vrc_tracker_model_label(model: u8) -> &'static str {
    match model {
        0 => "不明",
        1 => "汎用",
        2 => "SlimeVR",
        3 => "Vive",
        4 => "Tundra",
        5 => "Haritora",
        6 => "HaritoraX Wireless",
        7 => "Mocopi",
        8 => "MeowTracker",
        _ => "その他",
    }
}

pub fn vrc_spine_mode_label(mode: u8) -> &'static str {
    match mode {
        0 => "不明",
        1 => "上半身のみ",
        2 => "腰椎優先",
        3 => "胸椎優先",
        4 => "胸椎/腰椎分割",
        _ => "その他",
    }
}

pub fn vrc_avatar_measurement_label(kind: u8) -> &'static str {
    match kind {
        0 => "不明",
        1 => "身長と腕",
        2 => "身長のみ",
        3 => "腕のみ",
        4 => "なし",
        _ => "その他",
    }
}

pub fn checklist_step_label(id: u8) -> &'static str {
    match id {
        0 => "不明",
        1 => "トラッカーの安静時キャリブレーション",
        2 => "フルリセット",
        3 => "VRChat 設定",
        4 => "SteamVR が未接続",
        5 => "HMD が未割当",
        6 => "トラッカーエラー",
        7 => "ネットワークプロファイルがパブリック",
        8 => "マウンティング キャリブレーション",
        9 => "足のマウンティング キャリブレーション",
        10 => "Stay Aligned の設定",
        11 => "SteamVR の手トラッキング",
        12 => "Standable がインストール済み",
        _ => "その他",
    }
}

pub fn magnetometer_label(status: u8) -> &'static str {
    match status {
        0 => "非対応",
        1 => "無効",
        2 => "有効",
        _ => "不明",
    }
}

pub fn serial_device_kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "不明",
        1 => "シリアル",
        2 => "USB",
        3 => "Bluetooth",
        4 => "WiFi",
        _ => "その他",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_status_is_one_based() {
        assert_eq!(TrackerStatus::from_wire(0), TrackerStatus::None);
        assert_eq!(TrackerStatus::from_wire(1), TrackerStatus::Disconnected);
        assert_eq!(TrackerStatus::from_wire(2), TrackerStatus::Ok);
        assert_eq!(TrackerStatus::from_wire(6), TrackerStatus::TimedOut);
        assert_eq!(TrackerStatus::from_wire(9), TrackerStatus::Unknown(9));
    }

    #[test]
    fn body_part_labels_follow_the_wire_enum() {
        assert_eq!(body_part_label(0), "なし");
        assert_eq!(body_part_label(4), "腰");
        assert_eq!(body_part_label(5), "骨盤");
        assert_eq!(body_part_label(6), "左上腿");
        assert_eq!(body_part_label(9), "右下腿");
        assert_eq!(body_part_label(255), "不明");
    }

    #[test]
    fn skeleton_bone_labels_match_server_enum() {
        assert_eq!(skeleton_bone_label(1), "頭");
        assert_eq!(skeleton_bone_label(2), "首");
        assert_eq!(skeleton_bone_label(5), "腰");
        assert_eq!(skeleton_bone_label(9), "上腿");
        assert_eq!(skeleton_bone_label(21), "上胸");
    }

    #[test]
    fn checklist_labels_match_server_enum() {
        assert_eq!(
            checklist_step_label(1),
            "トラッカーの安静時キャリブレーション"
        );
        assert_eq!(checklist_step_label(12), "Standable がインストール済み");
    }

    #[test]
    fn every_assignable_body_part_has_a_label() {
        for (part, label) in ASSIGNABLE_BODY_PARTS {
            assert_ne!(*part, 0, "the unassigned entry is not sendable");
            assert!(!label.is_empty());
            assert_eq!(body_part_label(*part), *label);
        }
    }

    #[test]
    fn merge_keeps_fields_the_update_omits() {
        let mut current = ServerSettings {
            filtering: Some(FilteringSettings {
                filter_type: 2,
                amount: 0.5,
            }),
            hid: Some(true),
            ..Default::default()
        };
        // The server broadcasts a SteamVR-only response when the driver
        // reconnects; it must not wipe unrelated settings.
        current.merge(ServerSettings {
            steam_vr: Some(SteamVrSettings {
                waist: true,
                ..Default::default()
            }),
            ..Default::default()
        });

        assert_eq!(current.filtering.as_ref().map(|f| f.filter_type), Some(2));
        assert_eq!(current.hid, Some(true));
        assert_eq!(current.steam_vr.as_ref().map(|s| s.waist), Some(true));
    }

    #[test]
    fn merge_replaces_present_sections_entirely() {
        let mut current = ServerSettings {
            filtering: Some(FilteringSettings {
                filter_type: 1,
                amount: 0.25,
            }),
            ..Default::default()
        };
        current.merge(ServerSettings {
            filtering: Some(FilteringSettings {
                filter_type: 3,
                amount: 0.75,
            }),
            ..Default::default()
        });
        let filtering = current.filtering.expect("filtering");
        assert_eq!(filtering.filter_type, 3);
        assert!((filtering.amount - 0.75).abs() < 1e-6);
    }

    #[test]
    fn checklist_remaining_ignores_disabled_and_ignored_steps() {
        let checklist = Checklist {
            steps: vec![
                ChecklistStep {
                    id: 1,
                    valid: false,
                    enabled: true,
                    visibility: 0,
                    optional: false,
                    ignorable: true,
                    ignored: false,
                },
                ChecklistStep {
                    id: 2,
                    valid: false,
                    enabled: true,
                    visibility: 0,
                    optional: false,
                    ignorable: true,
                    ignored: true,
                },
                ChecklistStep {
                    id: 3,
                    valid: false,
                    enabled: false,
                    visibility: 0,
                    optional: false,
                    ignorable: true,
                    ignored: false,
                },
                ChecklistStep {
                    id: 4,
                    valid: true,
                    enabled: true,
                    visibility: 0,
                    optional: false,
                    ignorable: true,
                    ignored: false,
                },
            ],
        };
        assert_eq!(checklist.remaining(), 1);
        let visible: Vec<u8> = checklist.visible().map(|step| step.id).collect();
        assert_eq!(visible, vec![1, 2, 4], "disabled step 3 is hidden");
    }

    #[test]
    fn quat_yaw_matches_known_rotations() {
        let identity = Quat {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        assert!(identity.yaw_degrees().abs() < 1e-4);

        // 90 degrees around Y: (0, sin(45°), 0, cos(45°)).
        let half = std::f32::consts::FRAC_PI_4;
        let quarter_turn = Quat {
            x: 0.0,
            y: half.sin(),
            z: 0.0,
            w: half.cos(),
        };
        assert!((quarter_turn.yaw_degrees() - 90.0).abs() < 1e-3);
    }
}
