//! Nearby network controller directory derived from FSD position updates.

use crate::FsdMessage;
use std::collections::HashMap;

/// Group displayed in the Controllers In Range panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AtcCategory {
    Center,
    ApproachDeparture,
    Tower,
    Ground,
    Ramp,
    ClearanceDelivery,
    Atis,
    Observers,
}

impl AtcCategory {
    /// Returns the category heading shown in the pilot window.
    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "Center",
            Self::ApproachDeparture => "Approach/Departure",
            Self::Tower => "Tower",
            Self::Ground => "Ground",
            Self::Ramp => "Ramp",
            Self::ClearanceDelivery => "Clearance Delivery",
            Self::Atis => "ATIS",
            Self::Observers => "Observers",
        }
    }

    /// Returns all headings in the same order as the reference client.
    pub const fn all() -> [Self; 8] {
        [
            Self::Center,
            Self::ApproachDeparture,
            Self::Tower,
            Self::Ground,
            Self::Ramp,
            Self::ClearanceDelivery,
            Self::Atis,
            Self::Observers,
        ]
    }
}

/// One controller advertised by the network.
#[derive(Debug, Clone, PartialEq)]
pub struct AtcStation {
    /// Controller callsign, such as `EGLL_APP`.
    pub callsign: String,
    /// Radio frequency in kilohertz.
    pub frequency_khz: u32,
    /// Category used by the Controllers In Range panel.
    pub category: AtcCategory,
    /// Controller position in degrees.
    pub latitude: f64,
    /// Controller position in degrees.
    pub longitude: f64,
    /// Radius advertised by the controller in nautical miles.
    pub visible_range_nm: u16,
}

impl AtcStation {
    /// Formats a frequency as `123.450` MHz.
    pub fn frequency(&self) -> String {
        format!(
            "{}.{:03}",
            self.frequency_khz / 1000,
            self.frequency_khz % 1000
        )
    }
}

/// Tracks controller updates and removes stations that disconnect.
#[derive(Debug, Default)]
pub struct AtcDirectory {
    stations: HashMap<String, AtcStation>,
}

impl AtcDirectory {
    /// Creates an empty controller directory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies a decoded FSD packet when it affects a controller.
    pub fn apply(&mut self, message: &FsdMessage) {
        match message {
            FsdMessage::ControllerUpdate {
                callsign,
                frequency_khz,
                facility,
                visible_range_nm,
                latitude,
                longitude,
            } => {
                let station = AtcStation {
                    callsign: callsign.clone(),
                    frequency_khz: *frequency_khz,
                    category: category(callsign, *facility),
                    latitude: *latitude,
                    longitude: *longitude,
                    visible_range_nm: *visible_range_nm,
                };
                self.stations.insert(callsign.to_ascii_uppercase(), station);
            }
            FsdMessage::ControllerOffline { callsign } => {
                self.stations.remove(&callsign.to_ascii_uppercase());
            }
            _ => {}
        }
    }

    /// Returns controllers whose advertised range contains the own aircraft.
    pub fn nearby(&self, latitude: f64, longitude: f64) -> Vec<&AtcStation> {
        let mut stations = self
            .stations
            .values()
            .filter(|station| {
                distance_nm(latitude, longitude, station.latitude, station.longitude)
                    <= f64::from(station.visible_range_nm)
            })
            .collect::<Vec<_>>();
        stations.sort_by(|left, right| {
            left.category
                .cmp(&right.category)
                .then(left.callsign.cmp(&right.callsign))
        });
        stations
    }
}

fn category(callsign: &str, facility: u8) -> AtcCategory {
    let callsign = callsign.to_ascii_uppercase();
    if callsign.ends_with("_ATIS") {
        return AtcCategory::Atis;
    }
    if callsign.ends_with("_RMP") || callsign.ends_with("_RAMP") {
        return AtcCategory::Ramp;
    }
    match facility {
        0 => AtcCategory::Observers,
        2 => AtcCategory::ClearanceDelivery,
        3 => AtcCategory::Ground,
        4 => AtcCategory::Tower,
        5 => AtcCategory::ApproachDeparture,
        6 => AtcCategory::Center,
        _ => AtcCategory::Observers,
    }
}

fn distance_nm(from_lat: f64, from_lon: f64, to_lat: f64, to_lon: f64) -> f64 {
    let lat1 = from_lat.to_radians();
    let lat2 = to_lat.to_radians();
    let delta_lat = (to_lat - from_lat).to_radians();
    let delta_lon = (to_lon - from_lon).to_radians();
    let haversine =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    3440.065 * 2.0 * haversine.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_nearby_controllers_and_removes_disconnections() {
        let mut directory = AtcDirectory::new();
        directory.apply(&FsdMessage::ControllerUpdate {
            callsign: "EGLL_APP".into(),
            frequency_khz: 119_350,
            facility: 5,
            visible_range_nm: 80,
            latitude: 51.47,
            longitude: -0.4543,
        });
        let nearby = directory.nearby(51.5, -0.5);
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].category, AtcCategory::ApproachDeparture);
        assert_eq!(nearby[0].frequency(), "119.350");
        assert!(directory.nearby(40.0, -74.0).is_empty());
        directory.apply(&FsdMessage::ControllerOffline {
            callsign: "egll_app".into(),
        });
        assert!(directory.nearby(51.5, -0.5).is_empty());
    }
}
