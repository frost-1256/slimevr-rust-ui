//! Local GUI configuration and the little bit of `vrconfig.yml` this UI reads.
//!
//! Server-side settings are *not* stored here: they live in the server and are
//! read/written over RPC. This file only keeps things the Electron GUI also keeps
//! locally (the Java path, the data folder, muted warnings, ...).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::net::DEFAULT_URL;

pub const APP_ID: &str = "dev.slimevr.SlimeVR";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalConfig {
    /// WebSocket URL of the server to attach to.
    pub server_url: String,
    /// Directory holding `slimevr.jar`, if the user picked one.
    pub server_dir: Option<PathBuf>,
    /// Explicit Java executable; `None` means auto-detect.
    pub java_path: Option<PathBuf>,
    /// Launch the server automatically when the UI starts.
    pub autostart_server: bool,
    /// Request computed tracker bones in the data feed.
    pub include_bones: bool,
    /// Ask the server for OSC/VRChat related state on connect.
    pub warn_vrc_settings: bool,
    /// Warning keys the user muted in `vrconfig.yml`.
    pub muted_warnings: Vec<String>,
    /// UI scale multiplier.
    pub ui_scale: f32,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            server_url: DEFAULT_URL.to_string(),
            server_dir: None,
            java_path: None,
            autostart_server: false,
            include_bones: true,
            warn_vrc_settings: true,
            muted_warnings: Vec::new(),
            ui_scale: 1.0,
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_ID))
}

pub fn data_dir() -> Option<PathBuf> {
    // Match the Electron app: it stores the server data next to the user config.
    config_dir()
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("native-ui.json"))
}

/// `vrconfig.yml` in the per-user data directory, or next to the jar if the jar
/// directory contains a `config/` folder (portable installs).
pub fn server_config_path(jar_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = jar_dir {
        let portable = dir.join("vrconfig.yml");
        if portable.is_file() {
            return Some(portable);
        }
    }
    data_dir().map(|dir| dir.join("vrconfig.yml"))
}

impl LocalConfig {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(config) => config,
                Err(error) => {
                    log::warn!("failed to parse {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = config_path() else {
            return;
        };
        if let Some(dir) = path.parent()
            && let Err(error) = std::fs::create_dir_all(dir)
        {
            log::warn!("failed to create {}: {error}", dir.display());
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    log::warn!("failed to write {}: {error}", path.display());
                }
            }
            Err(error) => log::warn!("failed to serialize config: {error}"),
        }
    }
}

/// Extract `vrcConfig.mutedWarnings` from a `vrconfig.yml` without pulling in a
/// YAML parser: find the `mutedWarnings:` key and collect the following list
/// items that are indented at least as far as the key.
pub fn read_muted_warnings(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut muted = Vec::new();
    let mut collecting = false;
    let mut key_indent = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - trimmed.len();

        if collecting {
            if let Some(item) = trimmed.strip_prefix("- ")
                && indent > key_indent
            {
                muted.push(item.trim().trim_matches('"').trim_matches('\'').to_string());
                continue;
            }
            collecting = false;
        }

        if let Some(rest) = trimmed.strip_prefix("mutedWarnings:") {
            let rest = rest.trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                for item in rest.trim_matches(['[', ']']).split(',') {
                    let item = item.trim().trim_matches('"').trim_matches('\'');
                    if !item.is_empty() {
                        muted.push(item.to_string());
                    }
                }
                continue;
            }
            collecting = true;
            key_indent = indent;
        }
    }

    muted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_inline_and_block_muted_warnings() {
        let dir = std::env::temp_dir().join("slimevr-native-ui-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vrconfig.yml");
        std::fs::write(
            &path,
            "vrcConfig:\n  mutedWarnings:\n    - shoulderTracking\n    - legacyMode\nserver:\n  port: 21110\n",
        )
        .unwrap();
        let muted = read_muted_warnings(&path);
        assert_eq!(muted, vec!["shoulderTracking", "legacyMode"]);

        std::fs::write(
            &path,
            "vrcConfig:\n  mutedWarnings: [userHeight, calibrationRange]\n",
        )
        .unwrap();
        assert_eq!(
            read_muted_warnings(&path),
            vec!["userHeight", "calibrationRange"]
        );

        std::fs::remove_file(&path).ok();
    }
}
