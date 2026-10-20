# SlimeVR Native UI (mock)

Standalone Rust + egui desktop UI for SlimeVR. This is a **UI mock**: it uses
fixture data and does not connect to the SlimeVR server yet. The existing
Kotlin/Electron GUI is untouched; both applications can be launched independently.

## What is implemented

- Dark SlimeVR-style theme, header and status bar
- Dashboard with server/tracker summary cards
- Tracker list with battery, RSSI, TPS and mock reset
- Calibration flow with progress and stage indicators
- Settings form (port, OSC, VMC, SteamVR, auto-bone, smoothing)
- `--smoke-test` mode that renders for 3 seconds and exits (CI / Xvfb)

## Run

```sh
cargo run
```

Smoke test (requires an X server or Xvfb on Linux):

```sh
cargo run -- --smoke-test
```

## Windows build

GitHub Actions builds `windows-latest` on every push and uploads
`slimevr-native-ui-win64` containing `slimevr-native-ui.exe`.

## Not implemented yet

- SolarXR / protocol connection
- Real tracker state and calibration
- Settings persistence
- Server bundle integration or switching logic
