# SlimeVR Native UI (Rust + egui)

Standalone desktop UI for SlimeVR written in Rust with egui/eframe. It talks to
an already running SlimeVR server over the same SolarXR WebSocket RPC API
(`ws://127.0.0.1:21110`) that the Kotlin/Electron GUI uses. The Electron GUI is
untouched; both can be used side by side, but only one should own reset/assignment
actions at a time.

Japanese UI text, Material 3 components (`egui-material3` on egui/eframe 0.33),
and embedded Noto Sans JP + Material Symbols fonts, so Japanese text and icons
render without any system CJK or icon fonts.

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
  paths), one-click download of `slimevr.jar` (latest GitHub release) and of an
  Adoptium JRE 17 into the per-user data directory, server log.
- The dashboard starts with a server card (state, PID, start/stop), the same
  controls live on the server page.

Starting and stopping run on a supervisor thread: the button queues a request
and the frame loop only drains events, so the UI stays responsive while the JVM
boots or shuts down (`Stopped → Starting → Running → Stopping`). On Windows the
JVM is spawned with `CREATE_NO_WINDOW`, so no console window appears. A panic
writes a backtrace to `<data dir>/slimevr-native-ui/crash.log` in addition to
stderr.

The server also starts automatically when the UI launches (toggle
「起動時にサーバーを自動起動」 on the server page). It is skipped when the
jar/Java is missing or port 21110 is already taken, so an external server is
attached instead of fought over. Autostart is disabled during `--smoke-test`.

## Run

```sh
cargo run
```

Server discovery order for the jar is: `--path`, resources, `/usr/share/slimevr`,
the downloaded jar (`<data dir>/slimevr-native-ui/slimevr.jar`), the executable
directory, the current directory, and the in-tree Gradle output
(`server/server/desktop/build/libs/slimevr.jar`). The data directory is
`%LOCALAPPDATA%\slimevr-native-ui` on Windows and
`$XDG_DATA_HOME/slimevr-native-ui` (usually `~/.local/share/...`) elsewhere. A
downloaded runtime is picked up as `<data dir>/jre/bin/java`.

Downloads come from the latest GitHub release asset
(`.../releases/latest/download/slimevr.jar`) and from Adoptium's
`api.adoptium.net` JRE 17 for the running OS/arch. The server page shows live
progress and re-detects jar/Java as soon as a download finishes.

Smoke test (renders for 3 seconds, prints `smoke-ok`, exits 0):

```sh
cargo run -- --smoke-test
```

## Windows bundle

The Windows artifact is a ready-to-run folder: the executable, `slimevr.jar`
built from a pinned SlimeVR-Server commit, and a `jlink` runtime under `jre/`
(so no system Java is needed). CI verifies the bundle by running
`slimevr-native-ui.exe --start-server`, which boots the jar and waits for port 21110.

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
- The Windows bundle ships its own jar and JRE; source builds need a
  `slimevr.jar` and Java 17+ on the machine (found next to the executable, via
  `JAVA_HOME`, `/usr/lib/jvm`, or PATH).
- Only Windows x64 artifacts are produced by CI; other platforms build from
  source.
- SteamVR trackers/the driver bridge sections are hidden when the server reports
  no bridge (e.g. no driver installed).

## Font license

`assets/fonts/NotoSansJP-Regular.ttf` is Noto Sans JP (weight 400, subset) under
the SIL Open Font License 1.1. `assets/fonts/MaterialSymbolsOutlined.ttf` is a
subset of Google's Material Symbols Outlined under the Apache License 2.0. Both
are embedded into the executable; see `assets/fonts/LICENSE.txt` for the subset
recipe and license text links.
