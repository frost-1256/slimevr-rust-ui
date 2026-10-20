# SlimeVR Native UI (Rust + egui)

Standalone desktop UI for SlimeVR written in Rust with egui/eframe. It talks to
an already running SlimeVR server over the same SolarXR WebSocket RPC API
(`ws://127.0.0.1:21110`) that the Kotlin/Electron GUI uses. The Electron GUI is
untouched; both can be used side by side, but only one should own reset/assignment
actions at a time.

Japanese UI text, dark SlimeVR-style theme, and an embedded Japanese font
(IPAexGothic) so it renders without system CJK fonts.

## Features

- **Dashboard**: tracker and device counts, recalibration state, reset cards
  (Yaw / Full / Mounting) with progress and stage, reset event log.
- **Trackers**: live list (battery, RSSI, TPS, status), tracker detail, role
  assignment (all assignable body parts, plus unassign), rename, forget device.
- **Reset**: yaw/full/mounting resets with optional feet/finger scope, delay,
  per-part progress.
- **Proportions**: skeleton bone lengths (1–21), reset-all, feed-bones view,
  height calibration (HMD height, start/cancel), AutoBone record/process/apply.
- **Checklist**: server checklist steps with ignore toggles.
- **Settings**: SteamVR trackers, filtering, drift compensation, OSC router, VRC
  OSC, VMC OSC, model toggles/ratios, resets, Stay Aligned, tap detection, HID,
  velocity prediction, VRM JSON — applied per card.
- **Console**: serial devices, WiFi provisioning/scan, OTA firmware update,
  server log tail.
- **Server**: connection state, SteamVR driver install, udev/Wayland info,
  start/stop the Java server from the UI (jar + Java auto-detection or explicit
  paths), server log.

## Run

```sh
cargo run
```

Server discovery order for the jar is: `--path`, resources, `/usr/share/slimevr`,
the executable directory, the current directory, and the in-tree Gradle output
(`server/server/desktop/build/libs/slimevr.jar`).

Smoke test (renders for 3 seconds, prints `smoke-ok`, exits 0):

```sh
cargo run -- --smoke-test
```

## Windows build

GitHub Actions runs the test suite and an Xvfb smoke test on Linux, builds
`target/release/slimevr-native-ui.exe`, and uploads
`slimevr-native-ui-win64`.

## Tests

```sh
cargo test --locked
```

The suite covers protocol compatibility quirks rather than the UI itself:

- SolarXR frame encoders/decoders (assign, reset, settings sections, OTA,
  data-feed status mapping, per-device battery, guards, bones)
- partial `SettingsResponse` merge semantics
- tracker status / body part / skeleton bone / checklist wire enums
- checklist visibility and remaining-step counting
- local config parsing
- MAC address formatting

## Known limitations

- No automatic firmware release lookup; OTA requires a URL and digest.
- PubSub (`solarxr-protocol` pub/sub) is not implemented; the UI uses RPC and a
  single data feed.
- The server process panel requires an existing `slimevr.jar` and Java 17+; the
  UI does not download or install Java.
- Only Windows x64 artifacts are produced by CI; other platforms build from
  source.
- SteamVR trackers/the driver bridge sections are hidden when the server reports
  no bridge (e.g. no driver installed).

## Font license

`assets/fonts/ipaexg.ttf` is IPAexGothic, redistributed unmodified under the IPA
Font License v1.0 (see `assets/fonts/LICENSE.txt`).
