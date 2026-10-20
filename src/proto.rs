//! SolarXR FlatBuffers codec: every request the UI sends and every message it
//! understands coming back.
//!
//! One WebSocket binary frame is exactly one `MessageBundle` (no size prefix, no
//! file identifier). See `docs/protocol-notes.md` for the server-side quirks
//! this module works around.

use solarxr_protocol as sp;
use sp::flatbuffers::{FlatBufferBuilder, UnionWIPOffset, WIPOffset, root};

use crate::model::*;

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Decode a binary frame into zero or more events. A malformed frame yields a
/// single `Malformed` event instead of panicking.
pub fn decode(bytes: &[u8]) -> Vec<ServerEvent> {
    let bundle = match root::<sp::MessageBundle>(bytes) {
        Ok(bundle) => bundle,
        Err(error) => return vec![ServerEvent::Malformed(error.to_string())],
    };

    let mut events = Vec::new();

    if let Some(feeds) = bundle.data_feed_msgs() {
        for header in feeds {
            if let Some(update) = header.message_as_data_feed_update() {
                events.push(ServerEvent::Feed(decode_feed(&update)));
            }
        }
    }

    if let Some(rpcs) = bundle.rpc_msgs() {
        for header in rpcs {
            decode_rpc(&header, &mut events);
        }
    }

    events
}

fn decode_feed(update: &sp::data_feed::DataFeedUpdate<'_>) -> FeedUpdate {
    let mut feed = FeedUpdate::default();

    if let Some(devices) = update.devices() {
        for device in devices {
            feed.devices.push(decode_device(&device));
        }
    }

    if let Some(synthetic) = update.synthetic_trackers() {
        for tracker in synthetic {
            feed.synthetic.push(decode_tracker(&tracker, None));
        }
    }

    if let Some(guards) = update.server_guards() {
        feed.guards = Some(Guards {
            can_do_mounting: guards.canDoMounting(),
            can_do_yaw_reset: guards.canDoYawReset(),
            can_do_user_height_calibration: guards.canDoUserHeightCalibration(),
        });
    }

    if let Some(bones) = update.bones() {
        for bone in bones {
            let head = bone
                .head_position_g()
                .map(|p| [p.x(), p.y(), p.z()])
                .unwrap_or_default();
            feed.bones.push(Bone {
                body_part: bone.body_part().0,
                bone_length: bone.bone_length(),
                head_position: head,
            });
        }
    }

    feed
}

fn decode_device(device: &sp::data_feed::device_data::DeviceData<'_>) -> Device {
    let mut snapshot = Device {
        id: device.id().map(|id| id.id()).unwrap_or(0),
        custom_name: device.custom_name().unwrap_or_default().to_string(),
        ..Default::default()
    };

    if let Some(info) = device.hardware_info() {
        snapshot.firmware_version = info.firmware_version().map(str::to_string);
        snapshot.firmware_date = info.firmware_date().map(str::to_string);
        snapshot.manufacturer = info.manufacturer().map(str::to_string);
        snapshot.hardware_identifier = info.hardware_identifier().map(str::to_string);
        snapshot.mac_address = info.hardware_address().map(|address| address.addr());
        snapshot.network_protocol_version = info.network_protocol_version();
        snapshot.ip_address = info.ip_address().map(|ip| {
            let raw = ip.addr();
            format!(
                "{}.{}.{}.{}",
                (raw >> 24) & 0xff,
                (raw >> 16) & 0xff,
                (raw >> 8) & 0xff,
                raw & 0xff
            )
        });
    }

    if let Some(status) = device.hardware_status() {
        snapshot.battery_pct = status.battery_pct_estimate();
        snapshot.battery_voltage = status.battery_voltage();
        snapshot.battery_runtime_us = status.battery_runtime_estimate();
        snapshot.rssi = status.rssi();
        snapshot.ping = status.ping();
        snapshot.mcu_temp = status.mcu_temp();
        snapshot.packet_loss = status.packet_loss();
        snapshot.packets_lost = status.packets_lost();
        snapshot.packets_received = status.packets_received();
        snapshot.error_status = status.error_status().map(|code| code.0);
    }

    if let Some(trackers) = device.trackers() {
        for tracker in trackers {
            snapshot
                .trackers
                .push(decode_tracker(&tracker, Some(snapshot.id)));
        }
    }

    snapshot
}

fn decode_tracker(
    tracker: &sp::data_feed::tracker::TrackerData<'_>,
    device_id: Option<u8>,
) -> Tracker {
    let (key_device, tracker_num) = match tracker.tracker_id() {
        Some(id) => (
            id.device_id().map(|device| device.id()).or(device_id),
            id.tracker_num(),
        ),
        None => (device_id, 0),
    };

    let mut snapshot = Tracker {
        key: TrackerKey {
            device_id: key_device,
            tracker_num,
        },
        status: TrackerStatus::None,
        body_part: 0,
        display_name: String::new(),
        custom_name: String::new(),
        is_imu: false,
        is_computed: false,
        is_hmd: false,
        editable: false,
        allow_drift_compensation: false,
        magnetometer: 0,
        mounting_orientation: Quat::default(),
        rotation: None,
        tps: None,
        temp: None,
        hardware_identifier: None,
    };

    if let Some(info) = tracker.info() {
        snapshot.body_part = info.body_part().0;
        snapshot.is_imu = info.is_imu();
        snapshot.is_computed = info.is_computed();
        snapshot.is_hmd = info.is_hmd();
        snapshot.editable = info.editable();
        snapshot.allow_drift_compensation = info.allow_drift_compensation();
        snapshot.magnetometer = info.magnetometer().0;
        snapshot.display_name = info.display_name().unwrap_or_default().to_string();
        snapshot.custom_name = info.custom_name().unwrap_or_default().to_string();
        if let Some(rotation) = info.mounting_orientation() {
            snapshot.mounting_orientation = Quat {
                x: rotation.x(),
                y: rotation.y(),
                z: rotation.z(),
                w: rotation.w(),
            };
        }
    }

    snapshot.status = TrackerStatus::from_wire(tracker.status().0);
    snapshot.tps = tracker.tps();
    snapshot.temp = tracker.temp().map(|temp| temp.temp());

    if let Some(rotation) = tracker.rotation() {
        snapshot.rotation = Some(Quat {
            x: rotation.x(),
            y: rotation.y(),
            z: rotation.z(),
            w: rotation.w(),
        });
    }

    snapshot
}

fn decode_rpc(header: &sp::rpc::RpcMessageHeader<'_>, events: &mut Vec<ServerEvent>) {
    let kind = header.message_type();

    if kind == sp::rpc::RpcMessage::SettingsResponse {
        if let Some(response) = header.message_as_settings_response() {
            events.push(ServerEvent::Settings(decode_settings(&response)));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::ResetResponse {
        if let Some(response) = header.message_as_reset_response() {
            events.push(ServerEvent::Reset(ResetProgress {
                reset_type: ResetType::from_wire(response.reset_type().0),
                started: response.status().0 == 0,
                progress_ms: response.progress().max(0) as u32,
                duration_ms: response.duration().max(0) as u32,
            }));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::TrackingPauseStateResponse {
        if let Some(response) = header.message_as_tracking_pause_state_response() {
            events.push(ServerEvent::PauseState(response.trackingPaused()));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::SkeletonConfigResponse {
        if let Some(response) = header.message_as_skeleton_config_response() {
            let mut bones = std::collections::BTreeMap::new();
            if let Some(parts) = response.skeleton_parts() {
                for part in parts {
                    bones.insert(part.bone().0, part.value());
                }
            }
            events.push(ServerEvent::Skeleton(SkeletonConfig {
                bones,
                user_height: response.user_height(),
                reset: false,
            }));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::ServerInfosResponse {
        if let Some(response) = header.message_as_server_infos_response() {
            events.push(ServerEvent::ServerInfos {
                local_ip: response.localIp().unwrap_or_default().to_string(),
            });
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::TrackingChecklistResponse {
        if let Some(response) = header.message_as_tracking_checklist_response() {
            let ignored: Vec<u8> = response
                .ignored_steps()
                .map(|steps| steps.iter().map(|step| step.0).collect())
                .unwrap_or_default();
            let mut checklist = Checklist::default();
            if let Some(steps) = response.steps() {
                for step in steps {
                    let id = step.id().0;
                    checklist.steps.push(ChecklistStep {
                        id,
                        valid: step.valid(),
                        enabled: step.enabled(),
                        visibility: step.visibility().0,
                        optional: step.optional(),
                        ignorable: step.ignorable(),
                        ignored: ignored.contains(&id),
                    });
                }
            }
            events.push(ServerEvent::Checklist(checklist));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::MagToggleResponse {
        if let Some(response) = header.message_as_mag_toggle_response() {
            let key = response
                .tracker_id()
                .map(|id| TrackerKey {
                    device_id: id.device_id().map(|device| device.id()),
                    tracker_num: id.tracker_num(),
                })
                .unwrap_or(TrackerKey {
                    device_id: None,
                    tracker_num: 0,
                });
            events.push(ServerEvent::MagToggle {
                key,
                enabled: response.enable(),
            });
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::VRCConfigStateChangeResponse {
        if let Some(response) = header.message_as_vrcconfig_state_change_response() {
            events.push(ServerEvent::VrcConfig(decode_vrc_config(&response)));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::AutoBoneProcessStatusResponse {
        if let Some(response) = header.message_as_auto_bone_process_status_response() {
            events.push(ServerEvent::AutoBone(AutoBoneStatus::Progress {
                process_type: response.process_type().0,
                current: response.current(),
                total: response.total(),
                completed: response.completed(),
                success: response.success(),
                eta: response.eta(),
            }));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::AutoBoneEpochResponse {
        if let Some(response) = header.message_as_auto_bone_epoch_response() {
            events.push(ServerEvent::AutoBone(AutoBoneStatus::Epoch {
                current_epoch: response.current_epoch(),
                total_epochs: response.total_epochs(),
                error: response.epoch_error(),
            }));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::UserHeightRecordingStatusResponse {
        if let Some(response) = header.message_as_user_height_recording_status_response() {
            events.push(ServerEvent::UserHeight(UserHeightStatus {
                hmd_height: response.hmdHeight(),
                status: response.status().0,
            }));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::SerialUpdateResponse {
        if let Some(response) = header.message_as_serial_update_response() {
            let log = response
                .log()
                .map(|lines| lines.lines().map(str::to_string).collect())
                .unwrap_or_default();
            events.push(ServerEvent::SerialUpdate {
                log,
                closed: response.closed(),
                device: response
                    .device()
                    .map(|device| device.port().unwrap_or_default().to_string()),
            });
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::WifiProvisioningStatusResponse {
        if let Some(response) = header.message_as_wifi_provisioning_status_response() {
            events.push(ServerEvent::WifiProvisioning(response.status().0));
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::InstalledInfoResponse {
        if let Some(response) = header.message_as_installed_info_response() {
            events.push(ServerEvent::InstalledInfo {
                udev_installed: response.isUdevInstalled(),
                wayland: response.isWayland(),
            });
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::FirmwareUpdateStatusResponse {
        if let Some(response) = header.message_as_firmware_update_status_response() {
            let device = response
                .device_id_as_solarxr_protocol_datatypes_device_id_table()
                .and_then(|id| id.id().map(|device| format!("#{}", device.id())))
                .or_else(|| {
                    response
                        .device_id_as_serial_device_port()
                        .and_then(|port| port.port().map(str::to_string))
                });
            events.push(ServerEvent::FirmwareUpdate {
                progress: response.progress(),
                status: response.status().0,
                device,
            });
        }
        return;
    }
    if kind == sp::rpc::RpcMessage::NewSerialDeviceResponse {
        if let Some(response) = header.message_as_new_serial_device_response()
            && let Some(device) = response.device()
        {
            events.push(ServerEvent::NewSerialDevice {
                port: device.port().unwrap_or_default().to_string(),
                name: device.name().unwrap_or_default().to_string(),
                kind: device.type_().0,
            });
        }
        return;
    }

    events.push(ServerEvent::UnknownRpc(kind.0));
}

fn decode_settings(response: &sp::rpc::SettingsResponse<'_>) -> ServerSettings {
    let mut settings = ServerSettings::default();

    if let Some(steam) = response.steam_vr_trackers() {
        settings.steam_vr = Some(SteamVrSettings {
            waist: steam.waist(),
            chest: steam.chest(),
            left_foot: steam.left_foot(),
            right_foot: steam.right_foot(),
            left_knee: steam.left_knee(),
            right_knee: steam.right_knee(),
            left_elbow: steam.left_elbow(),
            right_elbow: steam.right_elbow(),
            left_hand: steam.left_hand(),
            right_hand: steam.right_hand(),
            automatic_tracker_toggle: steam.automaticTrackerToggle(),
        });
    }

    if let Some(filtering) = response.filtering() {
        settings.filtering = Some(FilteringSettings {
            filter_type: filtering.type_().0,
            amount: filtering.amount(),
        });
    }

    if let Some(drift) = response.drift_compensation() {
        settings.drift = Some(DriftSettings {
            enabled: drift.enabled(),
            prediction: drift.prediction(),
            amount: drift.amount(),
            max_resets: drift.max_resets(),
        });
    }

    if let Some(router) = response.osc_router() {
        settings.osc_router = router.osc_settings().map(decode_osc);
    }

    if let Some(vrc) = response.vrc_osc() {
        let trackers = vrc.trackers();
        settings.vrc_osc = Some(VrcOscSettings {
            osc: vrc.osc_settings().map(decode_osc).unwrap_or_default(),
            head: trackers.map(|t| t.head()).unwrap_or(false),
            chest: trackers.map(|t| t.chest()).unwrap_or(false),
            waist: trackers.map(|t| t.waist()).unwrap_or(false),
            knees: trackers.map(|t| t.knees()).unwrap_or(false),
            feet: trackers.map(|t| t.feet()).unwrap_or(false),
            elbows: trackers.map(|t| t.elbows()).unwrap_or(false),
            hands: trackers.map(|t| t.hands()).unwrap_or(false),
            oscquery_enabled: vrc.oscquery_enabled(),
        });
    }

    if let Some(vmc) = response.vmc_osc() {
        settings.vmc_osc = Some(VmcOscSettings {
            osc: vmc.osc_settings().map(decode_osc).unwrap_or_default(),
            anchor_hip: vmc.anchor_hip(),
            mirror_tracking: vmc.mirror_tracking(),
        });
    }

    if let Some(model) = response.model_settings() {
        let mut snapshot = ModelSettings::default();
        if let Some(toggles) = model.toggles() {
            snapshot.toggles = ModelToggles {
                extended_spine: toggles.extended_spine().unwrap_or(false),
                extended_pelvis: toggles.extended_pelvis().unwrap_or(false),
                extended_knee: toggles.extended_knee().unwrap_or(false),
                force_arms_from_hmd: toggles.force_arms_from_hmd().unwrap_or(false),
                floor_clip: toggles.floor_clip().unwrap_or(false),
                skating_correction: toggles.skating_correction().unwrap_or(false),
                toe_snap: toggles.toe_snap().unwrap_or(false),
                foot_plant: toggles.foot_plant().unwrap_or(false),
                self_localization: toggles.self_localization().unwrap_or(false),
                use_position: toggles.use_position().unwrap_or(false),
                enforce_constraints: toggles.enforce_constraints().unwrap_or(false),
                correct_constraints: toggles.correct_constraints().unwrap_or(false),
            };
        }
        if let Some(ratios) = model.ratios() {
            snapshot.ratios = ModelRatios {
                impute_waist_from_chest_hip: ratios.impute_waist_from_chest_hip(),
                impute_waist_from_chest_legs: ratios.impute_waist_from_chest_legs(),
                impute_hip_from_chest_legs: ratios.impute_hip_from_chest_legs(),
                impute_hip_from_waist_legs: ratios.impute_hip_from_waist_legs(),
                interp_hip_legs: ratios.interp_hip_legs(),
                interp_knee_tracker_ankle: ratios.interp_knee_tracker_ankle(),
                interp_knee_ankle: ratios.interp_knee_ankle(),
            };
        }
        snapshot.correction_strength = model.leg_tweaks().and_then(|leg| leg.correction_strength());
        if let Some(height) = model.skeleton_height() {
            snapshot.hmd_height = height.hmd_height();
            snapshot.floor_height = height.floor_height();
        }
        settings.model = Some(snapshot);
    }

    if let Some(resets) = response.resets_settings() {
        settings.resets = Some(ResetsSettings {
            reset_mounting_feet: resets.reset_mounting_feet(),
            arms_mounting_reset_mode: resets.arms_mounting_reset_mode().0,
            yaw_reset_smooth_time: resets.yaw_reset_smooth_time(),
            save_mounting_reset: resets.save_mounting_reset(),
            reset_hmd_pitch: resets.reset_hmd_pitch(),
        });
    }

    if let Some(stay) = response.stay_aligned() {
        settings.stay_aligned = Some(StayAlignedSettings {
            enabled: stay.enabled(),
            hide_yaw_correction: stay.hideYawCorrection(),
            standing_enabled: stay.standingEnabled(),
            standing_upper_leg_angle: stay.standingUpperLegAngle(),
            standing_lower_leg_angle: stay.standingLowerLegAngle(),
            standing_foot_angle: stay.standingFootAngle(),
            sitting_enabled: stay.sittingEnabled(),
            sitting_upper_leg_angle: stay.sittingUpperLegAngle(),
            sitting_lower_leg_angle: stay.sittingLowerLegAngle(),
            sitting_foot_angle: stay.sittingFootAngle(),
            flat_enabled: stay.flatEnabled(),
            flat_upper_leg_angle: stay.flatUpperLegAngle(),
            flat_lower_leg_angle: stay.flatLowerLegAngle(),
            flat_foot_angle: stay.flatFootAngle(),
            setup_complete: stay.setupComplete(),
        });
    }

    if let Some(tap) = response.tap_detection_settings() {
        settings.tap_detection = Some(TapDetectionSettings {
            full_reset_enabled: tap.full_reset_enabled(),
            full_reset_delay: tap.full_reset_delay(),
            full_reset_taps: tap.full_reset_taps(),
            yaw_reset_enabled: tap.yaw_reset_enabled(),
            yaw_reset_delay: tap.yaw_reset_delay(),
            yaw_reset_taps: tap.yaw_reset_taps(),
            mounting_reset_enabled: tap.mounting_reset_enabled(),
            mounting_reset_delay: tap.mounting_reset_delay(),
            mounting_reset_taps: tap.mounting_reset_taps(),
            number_trackers_over_threshold: tap.number_trackers_over_threshold(),
        });
    }

    settings.hid = response.hid_settings().map(|hid| hid.trackersOverHID());
    settings.velocity = response
        .velocity_settings()
        .map(|velocity| velocity.send_derived_velocity());
    settings.vrm_json = response
        .vrm()
        .and_then(|vrm| vrm.vrm_json())
        .map(str::to_string);

    settings
}

fn decode_vrc_config(response: &sp::rpc::VRCConfigStateChangeResponse<'_>) -> VrcConfigState {
    let mut state = VrcConfigState {
        is_supported: response.is_supported(),
        ..Default::default()
    };

    if let Some(validity) = response.validity() {
        let mut issues = Vec::new();
        let checks = [
            ("レガシーモード", validity.legacy_mode_ok()),
            ("肩トラッキング", validity.shoulder_tracking_ok()),
            ("ユーザー身長", validity.user_height_ok()),
            ("キャリブレーション範囲", validity.calibration_range_ok()),
            ("キャリブレーション表示", validity.calibration_visuals_ok()),
            ("トラッカーモデル", validity.tracker_model_ok()),
            ("脊椎モード", validity.spine_mode_ok()),
            ("アバター計測方式", validity.avatar_measurement_type_ok()),
            ("肩幅補正", validity.shoulder_width_compensation_ok()),
        ];
        for (label, ok) in checks {
            if !ok {
                issues.push(label.to_string());
            }
        }
        state.issues = issues;
    }

    if let Some(values) = response.state() {
        state.values = vec![
            ("レガシーモード".to_string(), yes_no(values.legacy_mode())),
            (
                "肩トラッキング無効".to_string(),
                yes_no(values.shoulder_tracking_disabled()),
            ),
            (
                "ユーザー身長".to_string(),
                format!("{:.2}", values.user_height()),
            ),
            (
                "キャリブレーション範囲".to_string(),
                format!("{:.2}", values.calibration_range()),
            ),
            (
                "キャリブレーション表示".to_string(),
                yes_no(values.calibration_visuals()),
            ),
            (
                "トラッカーモデル".to_string(),
                vrc_tracker_model_label(values.tracker_model().0).to_string(),
            ),
            (
                "脊椎モード".to_string(),
                vrc_spine_mode_label(values.spine_mode().0).to_string(),
            ),
            (
                "アバター計測方式".to_string(),
                vrc_avatar_measurement_label(values.avatar_measurement_type().0).to_string(),
            ),
            (
                "肩幅補正".to_string(),
                yes_no(values.shoulder_width_compensation()),
            ),
        ];
    }

    if let Some(recommended) = response.recommended() {
        state.recommended = vec![
            (
                "レガシーモード".to_string(),
                yes_no(recommended.legacy_mode()),
            ),
            (
                "肩トラッキング無効".to_string(),
                yes_no(recommended.shoulder_tracking_disabled()),
            ),
            (
                "ユーザー身長".to_string(),
                format!("{:.2}", recommended.user_height()),
            ),
            (
                "キャリブレーション範囲".to_string(),
                format!("{:.2}", recommended.calibration_range()),
            ),
            (
                "キャリブレーション表示".to_string(),
                yes_no(recommended.calibration_visuals()),
            ),
            (
                "トラッカーモデル".to_string(),
                vrc_tracker_model_label(recommended.tracker_model().0).to_string(),
            ),
            ("脊椎モード".to_string(), {
                let modes: Vec<&str> = recommended
                    .spine_mode()
                    .map(|modes| {
                        modes
                            .iter()
                            .map(|mode| vrc_spine_mode_label(mode.0))
                            .collect()
                    })
                    .unwrap_or_default();
                modes.join(", ")
            }),
            (
                "アバター計測方式".to_string(),
                vrc_avatar_measurement_label(recommended.avatar_measurement_type().0).to_string(),
            ),
            (
                "肩幅補正".to_string(),
                yes_no(recommended.shoulder_width_compensation()),
            ),
        ];
    }

    state.muted = response
        .muted()
        .map(|muted| muted.iter().map(str::to_string).collect())
        .unwrap_or_default();

    state
}

fn yes_no(value: bool) -> String {
    if value {
        "はい".to_string()
    } else {
        "✗".to_string()
    }
}

fn decode_osc(osc: sp::rpc::OSCSettings<'_>) -> OscSettings {
    OscSettings {
        enabled: osc.enabled(),
        port_in: osc.port_in(),
        port_out: osc.port_out(),
        address: osc.address().unwrap_or_default().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Build a one-RPC `MessageBundle`.
fn rpc_frame(
    encode: impl FnOnce(&mut FlatBufferBuilder<'_>) -> (sp::rpc::RpcMessage, WIPOffset<UnionWIPOffset>),
) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let (message_type, message) = encode(&mut fbb);
    let header = sp::rpc::RpcMessageHeader::create(
        &mut fbb,
        &sp::rpc::RpcMessageHeaderArgs {
            tx_id: None,
            message_type,
            message: Some(message),
        },
    );
    let rpc_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        &mut fbb,
        &sp::MessageBundleArgs {
            rpc_msgs: Some(rpc_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

fn tracker_id<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    key: TrackerKey,
) -> WIPOffset<sp::datatypes::TrackerId<'a>> {
    let device_id = key.device_id.map(|id| sp::datatypes::DeviceId([id]));
    sp::datatypes::TrackerId::create(
        fbb,
        &sp::datatypes::TrackerIdArgs {
            device_id: device_id.as_ref(),
            tracker_num: key.tracker_num,
        },
    )
}

/// Register the single persistent data feed. `minimum_time_since_last` is in ms.
///
/// Only one feed is ever registered: the server emits bundles with zero offsets
/// when several feeds are due at once, which a verifying FlatBuffers client
/// rejects.
pub fn start_data_feed(minimum_time_since_last: u16, include_bones: bool) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();

    let tracker_mask = sp::data_feed::tracker::TrackerDataMask::create(
        &mut fbb,
        &sp::data_feed::tracker::TrackerDataMaskArgs {
            info: true,
            status: true,
            rotation: true,
            position: true,
            raw_angular_velocity: false,
            raw_acceleration: false,
            temp: true,
            linear_acceleration: true,
            rotation_reference_adjusted: true,
            rotation_identity_adjusted: true,
            tps: true,
            raw_magnetic_vector: true,
            stay_aligned: true,
        },
    );
    let device_mask = sp::data_feed::device_data::DeviceDataMask::create(
        &mut fbb,
        &sp::data_feed::device_data::DeviceDataMaskArgs {
            tracker_data: Some(tracker_mask),
            device_data: true,
        },
    );
    let tracker_mask_synthetic = sp::data_feed::tracker::TrackerDataMask::create(
        &mut fbb,
        &sp::data_feed::tracker::TrackerDataMaskArgs {
            info: true,
            status: true,
            rotation: true,
            position: true,
            raw_angular_velocity: false,
            raw_acceleration: false,
            temp: true,
            linear_acceleration: true,
            rotation_reference_adjusted: true,
            rotation_identity_adjusted: true,
            tps: true,
            raw_magnetic_vector: true,
            stay_aligned: true,
        },
    );
    let config = sp::data_feed::DataFeedConfig::create(
        &mut fbb,
        &sp::data_feed::DataFeedConfigArgs {
            minimum_time_since_last,
            data_mask: Some(device_mask),
            synthetic_trackers_mask: Some(tracker_mask_synthetic),
            bone_mask: include_bones,
            stay_aligned_pose_mask: true,
            server_guards_mask: true,
        },
    );
    let configs = fbb.create_vector(&[config]);
    let start = sp::data_feed::StartDataFeed::create(
        &mut fbb,
        &sp::data_feed::StartDataFeedArgs {
            data_feeds: Some(configs),
        },
    );
    let header = sp::data_feed::DataFeedMessageHeader::create(
        &mut fbb,
        &sp::data_feed::DataFeedMessageHeaderArgs {
            message_type: sp::data_feed::DataFeedMessage::StartDataFeed,
            message: Some(start.as_union_value()),
        },
    );
    let data_feed_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        &mut fbb,
        &sp::MessageBundleArgs {
            data_feed_msgs: Some(data_feed_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

pub fn settings_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message =
            sp::rpc::SettingsRequest::create(fbb, &sp::rpc::SettingsRequestArgs::default());
        (
            sp::rpc::RpcMessage::SettingsRequest,
            message.as_union_value(),
        )
    })
}

pub fn skeleton_config_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SkeletonConfigRequest::create(
            fbb,
            &sp::rpc::SkeletonConfigRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SkeletonConfigRequest,
            message.as_union_value(),
        )
    })
}

pub fn server_infos_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message =
            sp::rpc::ServerInfosRequest::create(fbb, &sp::rpc::ServerInfosRequestArgs::default());
        (
            sp::rpc::RpcMessage::ServerInfosRequest,
            message.as_union_value(),
        )
    })
}

pub fn pause_state_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::TrackingPauseStateRequest::create(
            fbb,
            &sp::rpc::TrackingPauseStateRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::TrackingPauseStateRequest,
            message.as_union_value(),
        )
    })
}

pub fn checklist_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::TrackingChecklistRequest::create(
            fbb,
            &sp::rpc::TrackingChecklistRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::TrackingChecklistRequest,
            message.as_union_value(),
        )
    })
}

pub fn set_checklist_step_ignored(step_id: u8, ignore: bool) -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::IgnoreTrackingChecklistStepRequest::create(
            fbb,
            &sp::rpc::IgnoreTrackingChecklistStepRequestArgs {
                step_id: sp::rpc::TrackingChecklistStepId(step_id),
                ignore,
            },
        );
        (
            sp::rpc::RpcMessage::IgnoreTrackingChecklistStepRequest,
            message.as_union_value(),
        )
    })
}

pub fn set_pause_tracking(paused: bool) -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SetPauseTrackingRequest::create(
            fbb,
            &sp::rpc::SetPauseTrackingRequestArgs {
                pauseTracking: paused,
            },
        );
        (
            sp::rpc::RpcMessage::SetPauseTrackingRequest,
            message.as_union_value(),
        )
    })
}

pub fn assign_tracker(
    key: TrackerKey,
    body_part: u8,
    display_name: Option<&str>,
    allow_drift_compensation: bool,
) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let tracker_id = tracker_id(&mut fbb, key);
    let display_name = display_name.map(|name| fbb.create_string(name));
    let message = sp::rpc::AssignTrackerRequest::create(
        &mut fbb,
        &sp::rpc::AssignTrackerRequestArgs {
            tracker_id: Some(tracker_id),
            body_position: sp::datatypes::BodyPart(body_part),
            mounting_orientation: None,
            display_name,
            allow_drift_compensation,
        },
    );
    let header = sp::rpc::RpcMessageHeader::create(
        &mut fbb,
        &sp::rpc::RpcMessageHeaderArgs {
            tx_id: None,
            message_type: sp::rpc::RpcMessage::AssignTrackerRequest,
            message: Some(message.as_union_value()),
        },
    );
    let rpc_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        &mut fbb,
        &sp::MessageBundleArgs {
            rpc_msgs: Some(rpc_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

pub fn reset(reset_type: ResetType, body_parts: &[u8]) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let parts: Vec<sp::datatypes::BodyPart> = body_parts
        .iter()
        .map(|part| sp::datatypes::BodyPart(*part))
        .collect();
    let parts = fbb.create_vector(&parts);
    let reset_type = match reset_type {
        ResetType::Yaw => sp::rpc::ResetType::Yaw,
        ResetType::Full => sp::rpc::ResetType::Full,
        ResetType::Mounting => sp::rpc::ResetType::Mounting,
    };
    let message = sp::rpc::ResetRequest::create(
        &mut fbb,
        &sp::rpc::ResetRequestArgs {
            reset_type,
            body_parts: Some(parts),
            delay: None,
        },
    );
    let header = sp::rpc::RpcMessageHeader::create(
        &mut fbb,
        &sp::rpc::RpcMessageHeaderArgs {
            tx_id: None,
            message_type: sp::rpc::RpcMessage::ResetRequest,
            message: Some(message.as_union_value()),
        },
    );
    let rpc_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        &mut fbb,
        &sp::MessageBundleArgs {
            rpc_msgs: Some(rpc_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

fn change_settings_frame<'a>(
    encode: impl FnOnce(&mut FlatBufferBuilder<'a>) -> WIPOffset<sp::rpc::ChangeSettingsRequest<'a>>,
) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let message = encode(&mut fbb);
    let header = sp::rpc::RpcMessageHeader::create(
        &mut fbb,
        &sp::rpc::RpcMessageHeaderArgs {
            tx_id: None,
            message_type: sp::rpc::RpcMessage::ChangeSettingsRequest,
            message: Some(message.as_union_value()),
        },
    );
    let rpc_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        &mut fbb,
        &sp::MessageBundleArgs {
            rpc_msgs: Some(rpc_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

pub fn set_steam_vr(settings: &SteamVrSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let steam = sp::rpc::SteamVRTrackersSetting::create(
            fbb,
            &sp::rpc::SteamVRTrackersSettingArgs {
                waist: settings.waist,
                chest: settings.chest,
                automaticTrackerToggle: settings.automatic_tracker_toggle,
                left_foot: settings.left_foot,
                right_foot: settings.right_foot,
                left_knee: settings.left_knee,
                right_knee: settings.right_knee,
                left_elbow: settings.left_elbow,
                right_elbow: settings.right_elbow,
                left_hand: settings.left_hand,
                right_hand: settings.right_hand,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                steam_vr_trackers: Some(steam),
                ..Default::default()
            },
        )
    })
}

pub fn set_filtering(settings: &FilteringSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let filtering = sp::rpc::FilteringSettings::create(
            fbb,
            &sp::rpc::FilteringSettingsArgs {
                type_: sp::datatypes::FilteringType(settings.filter_type),
                amount: settings.amount,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                filtering: Some(filtering),
                ..Default::default()
            },
        )
    })
}

pub fn set_drift(settings: &DriftSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let drift = sp::rpc::DriftCompensationSettings::create(
            fbb,
            &sp::rpc::DriftCompensationSettingsArgs {
                enabled: settings.enabled,
                prediction: settings.prediction,
                amount: settings.amount,
                max_resets: settings.max_resets,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                drift_compensation: Some(drift),
                ..Default::default()
            },
        )
    })
}

pub fn set_osc_router(settings: &OscSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let osc = encode_osc(fbb, settings);
        let router = sp::rpc::OSCRouterSettings::create(
            fbb,
            &sp::rpc::OSCRouterSettingsArgs {
                osc_settings: Some(osc),
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                osc_router: Some(router),
                ..Default::default()
            },
        )
    })
}

pub fn set_vrc_osc(settings: &VrcOscSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let osc = encode_osc(fbb, &settings.osc);
        let trackers = sp::rpc::OSCTrackersSetting::create(
            fbb,
            &sp::rpc::OSCTrackersSettingArgs {
                head: settings.head,
                chest: settings.chest,
                waist: settings.waist,
                knees: settings.knees,
                feet: settings.feet,
                elbows: settings.elbows,
                hands: settings.hands,
            },
        );
        let vrc = sp::rpc::VRCOSCSettings::create(
            fbb,
            &sp::rpc::VRCOSCSettingsArgs {
                osc_settings: Some(osc),
                trackers: Some(trackers),
                oscquery_enabled: settings.oscquery_enabled,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                vrc_osc: Some(vrc),
                ..Default::default()
            },
        )
    })
}

pub fn set_vmc_osc(settings: &VmcOscSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let osc = encode_osc(fbb, &settings.osc);
        let vmc = sp::rpc::VMCOSCSettings::create(
            fbb,
            &sp::rpc::VMCOSCSettingsArgs {
                osc_settings: Some(osc),
                anchor_hip: settings.anchor_hip,
                mirror_tracking: settings.mirror_tracking,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                vmc_osc: Some(vmc),
                ..Default::default()
            },
        )
    })
}

pub fn set_model(settings: &ModelSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let toggles = sp::rpc::settings::ModelToggles::create(
            fbb,
            &sp::rpc::settings::ModelTogglesArgs {
                extended_spine: Some(settings.toggles.extended_spine),
                extended_pelvis: Some(settings.toggles.extended_pelvis),
                extended_knee: Some(settings.toggles.extended_knee),
                force_arms_from_hmd: Some(settings.toggles.force_arms_from_hmd),
                floor_clip: Some(settings.toggles.floor_clip),
                skating_correction: Some(settings.toggles.skating_correction),
                toe_snap: Some(settings.toggles.toe_snap),
                foot_plant: Some(settings.toggles.foot_plant),
                self_localization: Some(settings.toggles.self_localization),
                use_position: Some(settings.toggles.use_position),
                enforce_constraints: Some(settings.toggles.enforce_constraints),
                correct_constraints: Some(settings.toggles.correct_constraints),
            },
        );
        let ratios = sp::rpc::settings::ModelRatios::create(
            fbb,
            &sp::rpc::settings::ModelRatiosArgs {
                impute_waist_from_chest_hip: settings.ratios.impute_waist_from_chest_hip,
                impute_waist_from_chest_legs: settings.ratios.impute_waist_from_chest_legs,
                impute_hip_from_chest_legs: settings.ratios.impute_hip_from_chest_legs,
                impute_hip_from_waist_legs: settings.ratios.impute_hip_from_waist_legs,
                interp_hip_legs: settings.ratios.interp_hip_legs,
                interp_knee_tracker_ankle: settings.ratios.interp_knee_tracker_ankle,
                interp_knee_ankle: settings.ratios.interp_knee_ankle,
            },
        );
        let leg_tweaks = sp::rpc::settings::LegTweaksSettings::create(
            fbb,
            &sp::rpc::settings::LegTweaksSettingsArgs {
                correction_strength: settings.correction_strength,
            },
        );
        let height = sp::rpc::settings::SkeletonHeight::create(
            fbb,
            &sp::rpc::settings::SkeletonHeightArgs {
                hmd_height: settings.hmd_height,
                floor_height: settings.floor_height,
            },
        );
        let model = sp::rpc::settings::ModelSettings::create(
            fbb,
            &sp::rpc::settings::ModelSettingsArgs {
                toggles: Some(toggles),
                ratios: Some(ratios),
                leg_tweaks: Some(leg_tweaks),
                skeleton_height: Some(height),
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                model_settings: Some(model),
                ..Default::default()
            },
        )
    })
}

pub fn set_resets(settings: &ResetsSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let resets = sp::rpc::ResetsSettings::create(
            fbb,
            &sp::rpc::ResetsSettingsArgs {
                reset_mounting_feet: settings.reset_mounting_feet,
                arms_mounting_reset_mode: sp::rpc::ArmsMountingResetMode(
                    settings.arms_mounting_reset_mode,
                ),
                yaw_reset_smooth_time: settings.yaw_reset_smooth_time,
                save_mounting_reset: settings.save_mounting_reset,
                reset_hmd_pitch: settings.reset_hmd_pitch,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                resets_settings: Some(resets),
                ..Default::default()
            },
        )
    })
}

pub fn set_stay_aligned(settings: &StayAlignedSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let stay = sp::rpc::StayAlignedSettings::create(
            fbb,
            &sp::rpc::StayAlignedSettingsArgs {
                enabled: settings.enabled,
                extraYawCorrection: false,
                hideYawCorrection: settings.hide_yaw_correction,
                standingEnabled: settings.standing_enabled,
                standingUpperLegAngle: settings.standing_upper_leg_angle,
                standingLowerLegAngle: settings.standing_lower_leg_angle,
                standingFootAngle: settings.standing_foot_angle,
                sittingEnabled: settings.sitting_enabled,
                sittingUpperLegAngle: settings.sitting_upper_leg_angle,
                sittingLowerLegAngle: settings.sitting_lower_leg_angle,
                sittingFootAngle: settings.sitting_foot_angle,
                flatEnabled: settings.flat_enabled,
                flatUpperLegAngle: settings.flat_upper_leg_angle,
                flatLowerLegAngle: settings.flat_lower_leg_angle,
                flatFootAngle: settings.flat_foot_angle,
                setupComplete: settings.setup_complete,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                stay_aligned: Some(stay),
                ..Default::default()
            },
        )
    })
}

pub fn set_tap_detection(settings: &TapDetectionSettings) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let tap = sp::rpc::TapDetectionSettings::create(
            fbb,
            &sp::rpc::TapDetectionSettingsArgs {
                full_reset_delay: settings.full_reset_delay,
                full_reset_enabled: settings.full_reset_enabled,
                full_reset_taps: settings.full_reset_taps,
                yaw_reset_delay: settings.yaw_reset_delay,
                yaw_reset_enabled: settings.yaw_reset_enabled,
                yaw_reset_taps: settings.yaw_reset_taps,
                mounting_reset_delay: settings.mounting_reset_delay,
                mounting_reset_enabled: settings.mounting_reset_enabled,
                mounting_reset_taps: settings.mounting_reset_taps,
                setup_mode: Some(false),
                number_trackers_over_threshold: settings.number_trackers_over_threshold,
                yaw_reset_tracker: None,
                full_reset_tracker: None,
                mounting_reset_tracker: None,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                tap_detection_settings: Some(tap),
                ..Default::default()
            },
        )
    })
}

pub fn set_hid(enabled: bool) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let hid = sp::rpc::HIDSettings::create(
            fbb,
            &sp::rpc::HIDSettingsArgs {
                trackersOverHID: enabled,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                hid_settings: Some(hid),
                ..Default::default()
            },
        )
    })
}

pub fn set_velocity(enabled: bool) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let velocity = sp::rpc::VelocitySettings::create(
            fbb,
            &sp::rpc::VelocitySettingsArgs {
                send_derived_velocity: enabled,
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                velocity_settings: Some(velocity),
                ..Default::default()
            },
        )
    })
}

pub fn set_vrm_json(json: &str) -> Vec<u8> {
    change_settings_frame(|fbb| {
        let json = fbb.create_string(json);
        let vrm = sp::rpc::VRMSettings::create(
            fbb,
            &sp::rpc::VRMSettingsArgs {
                vrm_json: Some(json),
            },
        );
        sp::rpc::ChangeSettingsRequest::create(
            fbb,
            &sp::rpc::ChangeSettingsRequestArgs {
                vrm: Some(vrm),
                ..Default::default()
            },
        )
    })
}

fn encode_osc<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    settings: &OscSettings,
) -> WIPOffset<sp::rpc::OSCSettings<'a>> {
    let address = if settings.address.is_empty() {
        None
    } else {
        Some(fbb.create_string(&settings.address))
    };
    sp::rpc::OSCSettings::create(
        fbb,
        &sp::rpc::OSCSettingsArgs {
            enabled: settings.enabled,
            port_in: settings.port_in,
            port_out: settings.port_out,
            address,
        },
    )
}

pub fn change_skeleton_config(bone: u8, value: f32) -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::ChangeSkeletonConfigRequest::create(
            fbb,
            &sp::rpc::ChangeSkeletonConfigRequestArgs {
                bone: sp::rpc::SkeletonBone(bone),
                value,
            },
        );
        (
            sp::rpc::RpcMessage::ChangeSkeletonConfigRequest,
            message.as_union_value(),
        )
    })
}

pub fn skeleton_reset_all() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SkeletonResetAllRequest::create(
            fbb,
            &sp::rpc::SkeletonResetAllRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SkeletonResetAllRequest,
            message.as_union_value(),
        )
    })
}

pub fn enable_stay_aligned(enable: bool) -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::EnableStayAlignedRequest::create(
            fbb,
            &sp::rpc::EnableStayAlignedRequestArgs { enable },
        );
        (
            sp::rpc::RpcMessage::EnableStayAlignedRequest,
            message.as_union_value(),
        )
    })
}

pub fn settings_reset() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SettingsResetRequest::create(
            fbb,
            &sp::rpc::SettingsResetRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SettingsResetRequest,
            message.as_union_value(),
        )
    })
}

pub fn mag_toggle_request(key: TrackerKey) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let tracker_id = tracker_id(&mut fbb, key);
    let message = sp::rpc::MagToggleRequest::create(
        &mut fbb,
        &sp::rpc::MagToggleRequestArgs {
            tracker_id: Some(tracker_id),
        },
    );
    finish_single_rpc(&mut fbb, sp::rpc::RpcMessage::MagToggleRequest, message)
}

pub fn change_mag_toggle(key: TrackerKey, enable: bool) -> Vec<u8> {
    let mut fbb = FlatBufferBuilder::new();
    let tracker_id = tracker_id(&mut fbb, key);
    let message = sp::rpc::ChangeMagToggleRequest::create(
        &mut fbb,
        &sp::rpc::ChangeMagToggleRequestArgs {
            tracker_id: Some(tracker_id),
            enable,
        },
    );
    finish_single_rpc(
        &mut fbb,
        sp::rpc::RpcMessage::ChangeMagToggleRequest,
        message,
    )
}

pub fn forget_device(mac_address: &str) -> Vec<u8> {
    rpc_frame(|fbb| {
        let mac = fbb.create_string(mac_address);
        let message = sp::rpc::ForgetDeviceRequest::create(
            fbb,
            &sp::rpc::ForgetDeviceRequestArgs {
                mac_address: Some(mac),
            },
        );
        (
            sp::rpc::RpcMessage::ForgetDeviceRequest,
            message.as_union_value(),
        )
    })
}

pub fn auto_bone_process(process: u8) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let message = sp::rpc::AutoBoneProcessRequest::create(
            fbb,
            &sp::rpc::AutoBoneProcessRequestArgs {
                process_type: sp::rpc::AutoBoneProcessType(process),
            },
        );
        (
            sp::rpc::RpcMessage::AutoBoneProcessRequest,
            message.as_union_value(),
        )
    })
}

pub fn auto_bone_stop_recording() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::AutoBoneStopRecordingRequest::create(
            fbb,
            &sp::rpc::AutoBoneStopRecordingRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::AutoBoneStopRecordingRequest,
            message.as_union_value(),
        )
    })
}

pub fn auto_bone_cancel_recording() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::AutoBoneCancelRecordingRequest::create(
            fbb,
            &sp::rpc::AutoBoneCancelRecordingRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::AutoBoneCancelRecordingRequest,
            message.as_union_value(),
        )
    })
}

pub fn auto_bone_apply() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::AutoBoneApplyRequest::create(
            fbb,
            &sp::rpc::AutoBoneApplyRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::AutoBoneApplyRequest,
            message.as_union_value(),
        )
    })
}

pub fn start_user_height_calibration() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::StartUserHeightCalibration::create(
            fbb,
            &sp::rpc::StartUserHeightCalibrationArgs::default(),
        );
        (
            sp::rpc::RpcMessage::StartUserHeightCalibration,
            message.as_union_value(),
        )
    })
}

pub fn cancel_user_height_calibration() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::CancelUserHeightCalibration::create(
            fbb,
            &sp::rpc::CancelUserHeightCalibrationArgs::default(),
        );
        (
            sp::rpc::RpcMessage::CancelUserHeightCalibration,
            message.as_union_value(),
        )
    })
}

pub fn vrc_config_state_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::VRCConfigStateRequest::create(
            fbb,
            &sp::rpc::VRCConfigStateRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::VRCConfigStateRequest,
            message.as_union_value(),
        )
    })
}

pub fn vrc_toggle_mute(key: &str) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let key = fbb.create_string(key);
        let message = sp::rpc::VRCConfigSettingToggleMute::create(
            fbb,
            &sp::rpc::VRCConfigSettingToggleMuteArgs { key: Some(key) },
        );
        (
            sp::rpc::RpcMessage::VRCConfigSettingToggleMute,
            message.as_union_value(),
        )
    })
}

pub fn enable_steamvr_driver() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::EnableSteamVRDriverRequest::create(
            fbb,
            &sp::rpc::EnableSteamVRDriverRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::EnableSteamVRDriverRequest,
            message.as_union_value(),
        )
    })
}

pub fn installed_info_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::InstalledInfoRequest::create(
            fbb,
            &sp::rpc::InstalledInfoRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::InstalledInfoRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_devices_request() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SerialDevicesRequest::create(
            fbb,
            &sp::rpc::SerialDevicesRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SerialDevicesRequest,
            message.as_union_value(),
        )
    })
}

pub fn open_serial(auto: bool, port: Option<&str>) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let port = port.map(|port| fbb.create_string(port));
        let message =
            sp::rpc::OpenSerialRequest::create(fbb, &sp::rpc::OpenSerialRequestArgs { auto, port });
        (
            sp::rpc::RpcMessage::OpenSerialRequest,
            message.as_union_value(),
        )
    })
}

pub fn close_serial() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message =
            sp::rpc::CloseSerialRequest::create(fbb, &sp::rpc::CloseSerialRequestArgs::default());
        (
            sp::rpc::RpcMessage::CloseSerialRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_tracker_reboot() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SerialTrackerRebootRequest::create(
            fbb,
            &sp::rpc::SerialTrackerRebootRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SerialTrackerRebootRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_tracker_get_info() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SerialTrackerGetInfoRequest::create(
            fbb,
            &sp::rpc::SerialTrackerGetInfoRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SerialTrackerGetInfoRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_tracker_factory_reset() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SerialTrackerFactoryResetRequest::create(
            fbb,
            &sp::rpc::SerialTrackerFactoryResetRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SerialTrackerFactoryResetRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_tracker_get_wifi_scan() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::SerialTrackerGetWifiScanRequest::create(
            fbb,
            &sp::rpc::SerialTrackerGetWifiScanRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::SerialTrackerGetWifiScanRequest,
            message.as_union_value(),
        )
    })
}

pub fn serial_tracker_custom_command(command: &str) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let command = fbb.create_string(command);
        let message = sp::rpc::SerialTrackerCustomCommandRequest::create(
            fbb,
            &sp::rpc::SerialTrackerCustomCommandRequestArgs {
                command: Some(command),
            },
        );
        (
            sp::rpc::RpcMessage::SerialTrackerCustomCommandRequest,
            message.as_union_value(),
        )
    })
}

/// Queue an OTA firmware install for one device. The server downloads `url`
/// itself; `digest` is the expected `sha256-<base64>` if known.
pub fn firmware_update_ota(device_id: u8, url: &str, digest: &str) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let part_url = fbb.create_string(url);
        let part_digest = fbb.create_string(digest);
        let part = sp::rpc::FirmwarePart::create(
            fbb,
            &sp::rpc::FirmwarePartArgs {
                url: Some(part_url),
                offset: 0,
                digest: Some(part_digest),
            },
        );
        let device_id = sp::datatypes::DeviceId([device_id]);
        let update = sp::rpc::OTAFirmwareUpdate::create(
            fbb,
            &sp::rpc::OTAFirmwareUpdateArgs {
                device_id: Some(&device_id),
                firmware_part: Some(part),
            },
        );
        let message = sp::rpc::FirmwareUpdateRequest::create(
            fbb,
            &sp::rpc::FirmwareUpdateRequestArgs {
                method_type: sp::rpc::FirmwareUpdateMethod::OTAFirmwareUpdate,
                method: Some(update.as_union_value()),
            },
        );
        (
            sp::rpc::RpcMessage::FirmwareUpdateRequest,
            message.as_union_value(),
        )
    })
}

pub fn firmware_stop_queues() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::FirmwareUpdateStopQueuesRequest::create(
            fbb,
            &sp::rpc::FirmwareUpdateStopQueuesRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::FirmwareUpdateStopQueuesRequest,
            message.as_union_value(),
        )
    })
}

pub fn start_wifi_provisioning(ssid: &str, password: &str) -> Vec<u8> {
    rpc_frame(move |fbb| {
        let ssid = fbb.create_string(ssid);
        let password = fbb.create_string(password);
        let message = sp::rpc::StartWifiProvisioningRequest::create(
            fbb,
            &sp::rpc::StartWifiProvisioningRequestArgs {
                ssid: Some(ssid),
                password: Some(password),
                port: None,
            },
        );
        (
            sp::rpc::RpcMessage::StartWifiProvisioningRequest,
            message.as_union_value(),
        )
    })
}

pub fn stop_wifi_provisioning() -> Vec<u8> {
    rpc_frame(|fbb| {
        let message = sp::rpc::StopWifiProvisioningRequest::create(
            fbb,
            &sp::rpc::StopWifiProvisioningRequestArgs::default(),
        );
        (
            sp::rpc::RpcMessage::StopWifiProvisioningRequest,
            message.as_union_value(),
        )
    })
}

fn finish_single_rpc<T>(
    fbb: &mut FlatBufferBuilder<'_>,
    message_type: sp::rpc::RpcMessage,
    message: WIPOffset<T>,
) -> Vec<u8> {
    let header = sp::rpc::RpcMessageHeader::create(
        fbb,
        &sp::rpc::RpcMessageHeaderArgs {
            tx_id: None,
            message_type,
            message: Some(message.as_union_value()),
        },
    );
    let rpc_msgs = fbb.create_vector(&[header]);
    let bundle = sp::MessageBundle::create(
        fbb,
        &sp::MessageBundleArgs {
            rpc_msgs: Some(rpc_msgs),
            ..Default::default()
        },
    );
    fbb.finish(bundle, None);
    fbb.finished_data().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp::flatbuffers::root;

    /// Parse the single RPC header out of a frame produced by an encoder.
    fn rpc(bytes: &[u8]) -> sp::rpc::RpcMessageHeader<'_> {
        let bundle = root::<sp::MessageBundle>(bytes).expect("valid bundle");
        let rpcs = bundle.rpc_msgs().expect("rpc messages");
        assert_eq!(rpcs.len(), 1, "encoders must emit exactly one RPC");
        rpcs.get(0)
    }

    #[test]
    fn malformed_frame_becomes_event_instead_of_panicking() {
        let events = decode(&[1, 2, 3, 4]);
        assert!(matches!(events.as_slice(), [ServerEvent::Malformed(_)]));
    }

    #[test]
    fn empty_bundle_decodes_to_nothing() {
        let mut fbb = FlatBufferBuilder::new();
        let bundle = sp::MessageBundle::create(&mut fbb, &sp::MessageBundleArgs::default());
        fbb.finish(bundle, None);
        assert!(decode(fbb.finished_data()).is_empty());
    }

    #[test]
    fn start_data_feed_sets_bone_mask_only_when_requested() {
        for include_bones in [false, true] {
            let bytes = start_data_feed(50, include_bones);
            let bundle = root::<sp::MessageBundle>(&bytes).expect("valid bundle");
            let feed = bundle
                .data_feed_msgs()
                .and_then(|msgs| msgs.get(0).message_as_start_data_feed())
                .expect("start data feed");
            let config = feed.data_feeds().expect("configs").get(0);
            assert_eq!(config.minimum_time_since_last(), 50);
            assert_eq!(config.bone_mask(), include_bones);
            // Guards gate calibration buttons; the pose mask feeds Stay Aligned.
            assert!(config.server_guards_mask());
            assert!(config.stay_aligned_pose_mask());
        }
    }

    #[test]
    fn assign_tracker_encodes_key_role_and_drift_flag() {
        let key = TrackerKey {
            device_id: Some(7),
            tracker_num: 2,
        };
        let bytes = assign_tracker(key, 5, Some("waist"), true);
        let header = rpc(&bytes);
        assert_eq!(
            header.message_type(),
            sp::rpc::RpcMessage::AssignTrackerRequest
        );
        let request = header.message_as_assign_tracker_request().expect("assign");
        let id = request.tracker_id().expect("tracker id");
        assert_eq!(id.device_id().map(|id| id.0), Some([7]));
        assert_eq!(id.tracker_num(), 2);
        assert_eq!(request.body_position().0, 5);
        assert_eq!(request.display_name(), Some("waist"));
        assert!(request.allow_drift_compensation());
    }

    #[test]
    fn unassigning_uses_none_body_part() {
        let key = TrackerKey {
            device_id: Some(3),
            tracker_num: 0,
        };
        let bytes = assign_tracker(key, 0, None, false);
        let request = rpc(&bytes)
            .message_as_assign_tracker_request()
            .expect("assign");
        assert_eq!(
            request.body_position().0,
            0,
            "body part 0 clears the assignment"
        );
        assert!(request.display_name().is_none());
    }

    #[test]
    fn computed_tracker_key_has_no_device_id() {
        let key = TrackerKey {
            device_id: None,
            tracker_num: 4,
        };
        let bytes = assign_tracker(key, 9, None, false);
        let request = rpc(&bytes)
            .message_as_assign_tracker_request()
            .expect("assign");
        let id = request.tracker_id().expect("tracker id");
        assert!(id.device_id().is_none(), "computed trackers omit device_id");
        assert_eq!(id.tracker_num(), 4);
    }

    #[test]
    fn reset_request_carries_type_and_body_parts() {
        let bytes = reset(ResetType::Mounting, &[1, 9, 10]);
        let request = rpc(&bytes)
            .message_as_reset_request()
            .expect("reset request");
        assert_eq!(request.reset_type(), sp::rpc::ResetType::Mounting);
        let parts: Vec<u8> = request
            .body_parts()
            .expect("parts")
            .iter()
            .map(|part| part.0)
            .collect();
        assert_eq!(parts, vec![1, 9, 10]);
        assert!(request.delay().is_none(), "no delay for immediate resets");
    }

    #[test]
    fn stay_aligned_change_encodes_full_settings() {
        let settings = StayAlignedSettings {
            enabled: true,
            hide_yaw_correction: true,
            standing_enabled: true,
            standing_upper_leg_angle: 33.0,
            standing_lower_leg_angle: 12.5,
            standing_foot_angle: -4.0,
            sitting_enabled: false,
            sitting_upper_leg_angle: 80.0,
            sitting_lower_leg_angle: 45.0,
            sitting_foot_angle: 0.0,
            flat_enabled: true,
            flat_upper_leg_angle: 1.0,
            flat_lower_leg_angle: 2.0,
            flat_foot_angle: 3.0,
            setup_complete: true,
        };
        let bytes = set_stay_aligned(&settings);
        let request = rpc(&bytes)
            .message_as_change_settings_request()
            .expect("change settings");
        let section = request.stay_aligned().expect("stay aligned section");
        assert!(section.enabled());
        assert!(section.hideYawCorrection());
        assert!((section.standingUpperLegAngle() - 33.0).abs() < 1e-6);
        assert!((section.standingLowerLegAngle() - 12.5).abs() < 1e-6);
        assert!(section.setupComplete());
    }

    #[test]
    fn steam_vr_change_maps_every_body_part() {
        let settings = SteamVrSettings {
            waist: true,
            chest: false,
            left_foot: true,
            right_foot: false,
            left_knee: true,
            right_knee: false,
            left_elbow: true,
            right_elbow: false,
            left_hand: true,
            right_hand: false,
            automatic_tracker_toggle: true,
        };
        let bytes = set_steam_vr(&settings);
        let request = rpc(&bytes)
            .message_as_change_settings_request()
            .expect("change settings");
        let section = request.steam_vr_trackers().expect("steamvr section");
        assert!(section.waist());
        assert!(!section.chest());
        assert!(section.left_foot());
        assert!(!section.right_foot());
        assert!(section.left_knee());
        assert!(!section.right_knee());
        assert!(section.left_elbow());
        assert!(!section.right_elbow());
        assert!(section.left_hand());
        assert!(!section.right_hand());
        assert!(section.automaticTrackerToggle());
    }

    #[test]
    fn vmc_osc_change_sends_ports_and_address() {
        let settings = VmcOscSettings {
            osc: OscSettings {
                enabled: true,
                port_in: 39541,
                port_out: 39540,
                address: "127.0.0.2".to_owned(),
            },
            anchor_hip: true,
            mirror_tracking: true,
        };
        let bytes = set_vmc_osc(&settings);
        let request = rpc(&bytes)
            .message_as_change_settings_request()
            .expect("change settings");
        let vmc = request.vmc_osc().expect("vmc");
        assert!(vmc.anchor_hip());
        assert!(vmc.mirror_tracking());
        let osc = vmc.osc_settings().expect("osc");
        assert!(osc.enabled());
        assert_eq!(osc.port_in(), 39541);
        assert_eq!(osc.port_out(), 39540);
        assert_eq!(osc.address(), Some("127.0.0.2"));
    }

    #[test]
    fn skeleton_change_encodes_single_bone() {
        let bytes = change_skeleton_config(12, 0.42);
        let request = rpc(&bytes)
            .message_as_change_skeleton_config_request()
            .expect("skeleton config");
        assert_eq!(request.bone().0, 12);
        assert!((request.value() - 0.42).abs() < 1e-6);
    }

    #[test]
    fn ota_firmware_request_carries_device_url_and_digest() {
        let bytes = firmware_update_ota(4, "https://example.invalid/fw.bin", "sha256:abc");
        let request = rpc(&bytes)
            .message_as_firmware_update_request()
            .expect("fw");
        assert_eq!(
            request.method_type(),
            sp::rpc::FirmwareUpdateMethod::OTAFirmwareUpdate
        );
        let ota = request.method_as_otafirmware_update().expect("ota");
        let id = ota.device_id().expect("device id");
        assert_eq!(id.0, [4]);
        let part = ota.firmware_part().expect("part");
        assert_eq!(part.url(), Some("https://example.invalid/fw.bin"));
        assert_eq!(part.digest(), Some("sha256:abc"));
    }

    /// Build a `MessageBundle` carrying a single `DataFeedUpdate`.
    fn feed_bundle(
        build: impl for<'a> FnOnce(
            &mut FlatBufferBuilder<'a>,
        ) -> WIPOffset<sp::flatbuffers::UnionWIPOffset>,
    ) -> Vec<u8> {
        let mut fbb = FlatBufferBuilder::new();
        let update = build(&mut fbb);
        let header = sp::data_feed::DataFeedMessageHeader::create(
            &mut fbb,
            &sp::data_feed::DataFeedMessageHeaderArgs {
                message_type: sp::data_feed::DataFeedMessage::DataFeedUpdate,
                message: Some(update),
            },
        );
        let headers = fbb.create_vector(&[header]);
        let bundle = sp::MessageBundle::create(
            &mut fbb,
            &sp::MessageBundleArgs {
                data_feed_msgs: Some(headers),
                ..Default::default()
            },
        );
        fbb.finish(bundle, None);
        fbb.finished_data().to_vec()
    }

    fn update<'a>(
        fbb: &mut FlatBufferBuilder<'a>,
        devices: Option<
            WIPOffset<
                sp::flatbuffers::Vector<
                    'a,
                    sp::flatbuffers::ForwardsUOffset<sp::data_feed::device_data::DeviceData<'a>>,
                >,
            >,
        >,
        bones: Option<
            WIPOffset<
                sp::flatbuffers::Vector<
                    'a,
                    sp::flatbuffers::ForwardsUOffset<sp::data_feed::Bone<'a>>,
                >,
            >,
        >,
        guards: Option<WIPOffset<sp::data_feed::server::ServerGuards<'a>>>,
    ) -> WIPOffset<sp::flatbuffers::UnionWIPOffset> {
        sp::data_feed::DataFeedUpdate::create(
            fbb,
            &sp::data_feed::DataFeedUpdateArgs {
                devices,
                bones,
                server_guards: guards,
                ..Default::default()
            },
        )
        .as_union_value()
    }

    fn tracker<'a>(
        fbb: &mut FlatBufferBuilder<'a>,
        device_id: Option<u8>,
        tracker_num: u8,
        wire_status: u8,
        body_part: u8,
        name: &'static str,
    ) -> WIPOffset<sp::data_feed::tracker::TrackerData<'a>> {
        let device = device_id.map(|id| sp::datatypes::DeviceId([id]));
        let id = sp::datatypes::TrackerId::create(
            fbb,
            &sp::datatypes::TrackerIdArgs {
                device_id: device.as_ref(),
                tracker_num,
            },
        );
        let name = fbb.create_string(name);
        let info = sp::data_feed::tracker::TrackerInfo::create(
            fbb,
            &sp::data_feed::tracker::TrackerInfoArgs {
                body_part: sp::datatypes::BodyPart(body_part),
                is_imu: true,
                editable: true,
                display_name: Some(name),
                ..Default::default()
            },
        );
        sp::data_feed::tracker::TrackerData::create(
            fbb,
            &sp::data_feed::tracker::TrackerDataArgs {
                tracker_id: Some(id),
                info: Some(info),
                status: sp::datatypes::TrackerStatus(wire_status),
                tps: Some(45),
                ..Default::default()
            },
        )
    }

    #[test]
    fn feed_decodes_status_devices_and_per_device_battery() {
        let bytes = feed_bundle(|fbb| {
            let trk_a = tracker(fbb, Some(1), 0, 2, 5, "waist");
            let trk_b = tracker(fbb, Some(1), 1, 6, 9, "left_upper_leg");
            let trackers = fbb.create_vector(&[trk_a, trk_b]);
            let status = sp::datatypes::hardware_info::HardwareStatus::create(
                fbb,
                &sp::datatypes::hardware_info::HardwareStatusArgs {
                    battery_pct_estimate: Some(42),
                    rssi: Some(-60),
                    ..Default::default()
                },
            );
            let id = sp::datatypes::DeviceId([1]);
            let device = sp::data_feed::device_data::DeviceData::create(
                fbb,
                &sp::data_feed::device_data::DeviceDataArgs {
                    id: Some(&id),
                    hardware_status: Some(status),
                    trackers: Some(trackers),
                    ..Default::default()
                },
            );
            let devices = fbb.create_vector(&[device]);
            update(fbb, Some(devices), None, None)
        });

        let events = decode(&bytes);
        let [ServerEvent::Feed(feed)] = events.as_slice() else {
            panic!("expected one feed event, got {events:?}");
        };
        assert_eq!(feed.devices.len(), 1);
        let device = &feed.devices[0];
        assert_eq!(device.id, 1);
        assert_eq!(device.battery_pct, Some(42));
        assert_eq!(device.rssi, Some(-60));
        assert_eq!(device.trackers.len(), 2);

        let waist = &device.trackers[0];
        assert_eq!(waist.key.device_id, Some(1));
        assert_eq!(waist.key.tracker_num, 0);
        assert_eq!(waist.status, TrackerStatus::Ok, "wire status 2 is Ok");
        assert_eq!(waist.body_part, 5);
        assert_eq!(waist.display_name, "waist");
        assert_eq!(waist.tps, Some(45));
        assert!(waist.is_imu && waist.editable);

        let leg = &device.trackers[1];
        assert_eq!(
            leg.status,
            TrackerStatus::TimedOut,
            "wire status 6 is TimedOut"
        );
        assert_eq!(leg.body_part, 9);
    }

    #[test]
    fn feed_decodes_guards_and_bones() {
        let bytes = feed_bundle(|fbb| {
            let head = sp::datatypes::math::Vec3f::new(0.1, 1.5, -0.2);
            let bone = sp::data_feed::Bone::create(
                fbb,
                &sp::data_feed::BoneArgs {
                    body_part: sp::datatypes::BodyPart(10),
                    bone_length: 0.42,
                    head_position_g: Some(&head),
                    ..Default::default()
                },
            );
            let bones = fbb.create_vector(&[bone]);
            let guards = sp::data_feed::server::ServerGuards::create(
                fbb,
                &sp::data_feed::server::ServerGuardsArgs {
                    canDoMounting: true,
                    canDoYawReset: false,
                    canDoUserHeightCalibration: true,
                },
            );
            update(fbb, None, Some(bones), Some(guards))
        });

        let events = decode(&bytes);
        let [ServerEvent::Feed(feed)] = events.as_slice() else {
            panic!("expected one feed event, got {events:?}");
        };
        let guards = feed.guards.expect("guards");
        assert!(guards.can_do_mounting);
        assert!(!guards.can_do_yaw_reset);
        assert!(guards.can_do_user_height_calibration);
        assert_eq!(feed.bones.len(), 1);
        let bone = &feed.bones[0];
        assert_eq!(bone.body_part, 10);
        assert!((bone.bone_length - 0.42).abs() < 1e-6);
        assert!((bone.head_position[1] - 1.5).abs() < 1e-6);
    }
}
