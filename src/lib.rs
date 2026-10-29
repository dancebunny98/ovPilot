//! Native ovPilot components for simulator discovery and diagnostics.

mod atc;
mod audio;
mod auth;
mod chat;
mod config;
mod fsd;
mod matching;
mod model_scan;
mod type_codes;
mod update;
mod xplane;

pub use audio::{AudioDevices, MicrophoneLoopback, MicrophoneMonitor};
pub use chat::{ChatChannel, ChatInbox, ChatMessage, OutgoingText};
pub use config::{discover_model_roots, ClientSettings, SettingsStore};
pub use fsd::{FsdDecoder, FsdMessage};
pub use matching::{ModelCatalog, ModelMatcher, ModelRuleSet};
pub use model_scan::{AircraftModel, ModelFormat, ModelScanResult, ModelScanner};
pub use type_codes::{AircraftType, AircraftTypeCatalog};
pub use update::{UpdateChecker, UpdateStatus};
pub use xplane::{XPlaneProbe, XPlaneSnapshot, XPlaneVersion};

/// Public repository used by About and release checks.
pub const GITHUB_REPOSITORY: &str = "https://github.com/dancebunny98/ovPilot";

/// Version of the native ovPilot application.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error type returned by the ovPilot API.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Result type returned by the ovPilot API.
pub type Result<T> = std::result::Result<T, Error>;
pub use atc::{AtcCategory, AtcDirectory, AtcStation};
pub use auth::{AuthClient, AuthToken, PasswordStore};
