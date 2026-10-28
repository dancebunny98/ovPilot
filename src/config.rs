//! Persistent, portable client settings without simulator credentials.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User-selected locations that contain aircraft model files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientSettings {
    /// Directories scanned for simulator aircraft.
    pub model_roots: Vec<PathBuf>,
    /// Ordered paths to custom `.vmr` model matching rule sets.
    pub custom_rule_paths: Vec<PathBuf>,
    /// Local text kept in the Notes tab.
    pub notes: String,
    /// Width of the controller list in the main window, in logical pixels.
    pub controllers_width: f32,
    /// Controller list type size in points.
    pub controller_font_pt: u8,
    /// Text message type size in points.
    pub message_font_pt: u8,
    /// Uses a monospaced font for the controller list.
    pub controller_monospace: bool,
    /// Uses a monospaced font for text messages.
    pub message_monospace: bool,
    /// Selected microphone device name, or empty for the system default.
    pub microphone_device: String,
    /// Selected output device name, or empty for the system default.
    pub output_device: String,
    /// Suppresses radio effect processing when voice support is available.
    pub disable_radio_effects: bool,
    /// Suppresses HF background noise when voice support is available.
    pub disable_hf_noise: bool,
    /// Output gain for local audio playback, in decibels.
    pub output_volume_db: i8,
    /// Microphone gain for captured audio, in decibels.
    pub microphone_gain_db: i8,
    /// Logical keyboard key assigned to push-to-talk, or empty when unassigned.
    pub ptt_key: String,
    /// VATSIM CID. Its password is stored separately in the system credential vault.
    pub cid: String,
    /// Full name displayed to the network after a future FSD login.
    pub full_name: String,
    /// Pilot's home airport identifier.
    pub home_airport: String,
    /// Callsign last entered in the Connect dialog.
    pub last_callsign: String,
    /// ICAO aircraft type last entered in the Connect dialog.
    pub last_type_code: String,
    /// Optional SELCAL code last entered in the Connect dialog.
    pub last_selcal: String,
    /// Last observer-mode choice in the Connect dialog.
    pub observer_mode: bool,
    /// Notification preferences in the order shown on the Settings page.
    pub notifications: [bool; 5],
    /// Maximum range for future remote-aircraft rendering.
    pub aircraft_range_nm: u32,
    /// Maximum number of future remote aircraft to show.
    pub max_aircraft: u32,
    /// Enables automatic update checks at startup.
    pub auto_check_updates: bool,
    /// Enables future automatic transponder mode switching on takeoff.
    pub auto_mode_c: bool,
    /// Keeps the main window visible over other application windows.
    pub keep_window_visible: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            model_roots: Vec::new(),
            custom_rule_paths: Vec::new(),
            notes: String::new(),
            controllers_width: 235.0,
            controller_font_pt: 9,
            message_font_pt: 14,
            controller_monospace: false,
            message_monospace: true,
            microphone_device: String::new(),
            output_device: String::new(),
            disable_radio_effects: false,
            disable_hf_noise: false,
            output_volume_db: 0,
            microphone_gain_db: 0,
            ptt_key: String::new(),
            cid: String::new(),
            full_name: String::new(),
            home_airport: String::new(),
            last_callsign: String::new(),
            last_type_code: String::new(),
            last_selcal: String::new(),
            observer_mode: false,
            notifications: [true; 5],
            aircraft_range_nm: 80,
            max_aircraft: 999,
            auto_check_updates: true,
            auto_mode_c: true,
            keep_window_visible: false,
        }
    }
}

/// Owns the JSON settings file for one installation of ovPilot.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    /// Uses the platform's user configuration directory.
    pub fn discover() -> Result<Self> {
        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                })
        }
        .ok_or("cannot determine the user settings directory")?;
        Ok(Self::at(base.join("ovPilot").join("settings.json")))
    }

    /// Creates a store at an explicit path, useful for portable deployments.
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the settings file path.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Loads saved roots, or discovers existing simulator directories on first use.
    pub fn load(&self) -> Result<ClientSettings> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ClientSettings {
                model_roots: discover_model_roots(),
                ..Default::default()
            }),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes the currently selected model directories.
    pub fn save(&self, settings: &ClientSettings) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_vec_pretty(settings)?)?;
        Ok(())
    }
}

/// Detects existing common simulator aircraft locations and optional overrides.
pub fn discover_model_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["OVPILOT_MODEL_PATH", "XPLANE_HOME"] {
        if let Some(path) = std::env::var_os(key) {
            let path = PathBuf::from(path);
            roots.push(if key == "XPLANE_HOME" {
                path.join("Aircraft")
            } else {
                path
            });
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        for name in ["X-Plane 11", "X-Plane 12"] {
            roots.push(home.join("Desktop").join(name).join("Aircraft"));
            roots.push(home.join("Applications").join(name).join("Aircraft"));
        }
        roots.push(home.join("AppData/Roaming/Microsoft Flight Simulator/Packages/Community"));
        roots.push(home.join("AppData/Local/Packages/Microsoft.FlightSimulator_8wekyb3d8bbwe/LocalCache/Packages/Community"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        for simulator in [
            "Microsoft Flight Simulator",
            "Microsoft Flight Simulator 2024",
        ] {
            let config = PathBuf::from(&appdata).join(simulator).join("UserCfg.opt");
            if let Ok(contents) = fs::read_to_string(config) {
                if let Some(packages) = installed_packages_path(&contents) {
                    for folder in [
                        "Community",
                        "Community2024",
                        "Official",
                        "Official2024",
                        "StreamedPackages",
                    ] {
                        roots.push(packages.join(folder));
                    }
                }
            }
        }
    }
    roots.retain(|path| path.is_dir());
    roots.sort();
    roots.dedup();
    roots
}

fn installed_packages_path(config: &str) -> Option<PathBuf> {
    config.lines().find_map(|line| {
        let value = line.trim().strip_prefix("InstalledPackagesPath")?.trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        (!value.is_empty()).then(|| PathBuf::from(value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_selected_roots() {
        let path =
            std::env::temp_dir().join(format!("ovpilot-settings-{}.json", std::process::id()));
        let store = SettingsStore::at(path.clone());
        let settings = ClientSettings {
            model_roots: vec![PathBuf::from("Aircraft"), PathBuf::from("Community")],
            controllers_width: 317.0,
            controller_font_pt: 11,
            output_device: "Test output".into(),
            ptt_key: "Space".into(),
            last_callsign: "CCA1220".into(),
            last_type_code: "A320".into(),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.model_roots, settings.model_roots);
        assert_eq!(loaded.controllers_width, 317.0);
        assert_eq!(loaded.controller_font_pt, 11);
        assert_eq!(loaded.output_device, "Test output");
        assert_eq!(loaded.ptt_key, "Space");
        assert_eq!(loaded.last_callsign, "CCA1220");
        assert_eq!(loaded.last_type_code, "A320");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_custom_msfs_packages_location() {
        let config = "SomeOption 1\nInstalledPackagesPath \"F:\\Simulators\\MSFS2024\"\n";
        assert_eq!(
            installed_packages_path(config),
            Some(PathBuf::from("F:\\Simulators\\MSFS2024"))
        );
    }
}
