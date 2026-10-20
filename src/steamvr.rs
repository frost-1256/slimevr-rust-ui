//! SteamVR OpenVR driver installation status.
//!
//! SteamVR keeps the list of registered external drivers in
//! `openvrpaths.vrpath` (`%LOCALAPPDATA%\openvr` on Windows). SlimeVR
//! registers its driver there with `vrpathreg adddriver`, so reading the
//! registry file tells us whether the driver is installed without launching
//! SteamVR. The manifest name identifies the driver, and the binary check
//! catches registrations that point at a deleted or partial install.

use std::fs;
use std::path::{Path, PathBuf};

/// Result of inspecting the OpenVR driver registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverStatus {
    /// `openvrpaths.vrpath` does not exist: SteamVR never ran on this account.
    SteamVrNotFound,
    /// SteamVR is registered, but no SlimeVR driver is listed.
    NotRegistered,
    /// A SlimeVR driver is registered. `binary` is `None` when the DLL is missing.
    Registered {
        dir: PathBuf,
        binary: Option<PathBuf>,
    },
    /// The registry file exists but could not be read or parsed.
    Error(String),
}

/// Location of `openvrpaths.vrpath` for the current platform.
pub fn openvrpaths_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir().map(|dir| dir.join("openvr").join("openvrpaths.vrpath"))
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|home| home.join("Library/Application Support/OpenVR/openvrpaths.vrpath"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::home_dir().map(|home| home.join(".config/openvr/openvrpaths.vrpath"))
    }
}

/// Checks the driver registry of the current user.
pub fn check() -> DriverStatus {
    match openvrpaths_path() {
        Some(path) => check_at(&path),
        None => DriverStatus::SteamVrNotFound,
    }
}

/// Checks an explicit registry file. Separated for tests and for callers that
/// resolve the path themselves.
pub fn check_at(paths_file: &Path) -> DriverStatus {
    let json = match fs::read_to_string(paths_file) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return DriverStatus::SteamVrNotFound;
        }
        Err(error) => {
            return DriverStatus::Error(format!("{}: {error}", paths_file.display()));
        }
    };

    let drivers = match external_drivers(&json) {
        Ok(drivers) => drivers,
        Err(error) => return DriverStatus::Error(error),
    };

    for dir in drivers {
        if let Some(root) = slimevr_driver_root(&dir) {
            let binary = driver_binary(&root);
            return DriverStatus::Registered { dir: root, binary };
        }
    }
    DriverStatus::NotRegistered
}

fn external_drivers(json: &str) -> Result<Vec<PathBuf>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("openvrpaths.vrpath: {error}"))?;
    let list = value
        .get("external_drivers")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "openvrpaths.vrpath: external_drivers がありません".to_string())?;
    Ok(list
        .iter()
        .filter_map(|value| value.as_str())
        .map(PathBuf::from)
        .collect())
}

/// Finds the directory that holds `driver.vrdrivermanifest` for the SlimeVR
/// driver. Release packages use both `<dir>/` and `<dir>/slimevr/` layouts.
fn slimevr_driver_root(dir: &Path) -> Option<PathBuf> {
    [dir.to_path_buf(), dir.join("slimevr")]
        .into_iter()
        .find(|candidate| manifest_name(candidate).as_deref() == Some("slimevr"))
}

fn manifest_name(dir: &Path) -> Option<String> {
    let json = fs::read_to_string(dir.join("driver.vrdrivermanifest")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    Some(value.get("name")?.as_str()?.to_string())
}

fn driver_binary(dir: &Path) -> Option<PathBuf> {
    [
        "bin/win64/driver_slimevr.dll",
        "bin/linux64/driver_slimevr.so",
        "driver_slimevr.dll",
        "driver_slimevr.so",
    ]
    .iter()
    .map(|relative| dir.join(relative))
    .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("slimevr-steamvr-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(path, contents).expect("write");
    }

    fn registry(paths: &[&Path]) -> String {
        let list: Vec<String> = paths
            .iter()
            .map(|path| format!("{:?}", path.to_string_lossy()))
            .collect();
        format!(
            "{{\"jsonid\":\"vrpathreg\",\"version\":1,\"external_drivers\":[{}]}}",
            list.join(",")
        )
    }

    #[test]
    fn missing_runtime_file_means_steamvr_missing() {
        let temp = TempDir::new();
        assert_eq!(
            check_at(&temp.path().join("openvrpaths.vrpath")),
            DriverStatus::SteamVrNotFound
        );
    }

    #[test]
    fn no_external_drivers_is_not_registered() {
        let temp = TempDir::new();
        let file = temp.path().join("openvrpaths.vrpath");
        write(&file, &registry(&[]));
        assert_eq!(check_at(&file), DriverStatus::NotRegistered);
    }

    #[test]
    fn finds_slimevr_registered_at_the_driver_root() {
        let temp = TempDir::new();
        let driver = temp.path().join("slimevr-openvr-driver-win64");
        write(
            &driver.join("driver.vrdrivermanifest"),
            "{\"name\":\"slimevr\",\"alwaysActivate\":true}",
        );
        write(&driver.join("bin/win64/driver_slimevr.dll"), "");
        let file = temp.path().join("openvrpaths.vrpath");
        write(&file, &registry(&[driver.as_path()]));

        match check_at(&file) {
            DriverStatus::Registered { dir, binary } => {
                assert_eq!(dir, driver);
                assert_eq!(binary, Some(driver.join("bin/win64/driver_slimevr.dll")));
            }
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn finds_nested_driver_layout_and_reports_missing_binary() {
        let temp = TempDir::new();
        let driver = temp.path().join("slimevr-openvr-driver-win64");
        write(
            &driver.join("slimevr/driver.vrdrivermanifest"),
            "{\"name\":\"slimevr\"}",
        );
        let file = temp.path().join("openvrpaths.vrpath");
        write(&file, &registry(&[driver.as_path()]));

        match check_at(&file) {
            DriverStatus::Registered { dir, binary } => {
                assert_eq!(dir, driver.join("slimevr"));
                assert_eq!(binary, None);
            }
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn ignores_other_drivers() {
        let temp = TempDir::new();
        let driver = temp.path().join("other-driver");
        write(
            &driver.join("driver.vrdrivermanifest"),
            "{\"name\":\"other\"}",
        );
        let file = temp.path().join("openvrpaths.vrpath");
        write(&file, &registry(&[driver.as_path()]));
        assert_eq!(check_at(&file), DriverStatus::NotRegistered);
    }

    #[test]
    fn broken_registry_is_reported() {
        let temp = TempDir::new();
        let file = temp.path().join("openvrpaths.vrpath");
        write(&file, "not json");
        assert!(matches!(check_at(&file), DriverStatus::Error(_)));
    }
}
