//! Codec tests: encode/decode round trips and tolerating malformed frames.

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
    build: impl for<'a> FnOnce(&mut FlatBufferBuilder<'a>) -> WIPOffset<sp::flatbuffers::UnionWIPOffset>,
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
            sp::flatbuffers::Vector<'a, sp::flatbuffers::ForwardsUOffset<sp::data_feed::Bone<'a>>>,
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
