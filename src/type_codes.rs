//! Offline ICAO aircraft type suggestions for the Connect dialog.

use serde::Deserialize;

/// One aircraft type designator and its human-readable aircraft name.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AircraftType {
    /// ICAO type designator, such as `A320` or `B738`.
    pub code: String,
    /// Manufacturer and model associated with the designator.
    pub aircraft: String,
}

/// Searchable offline catalog of aircraft type designators.
pub struct AircraftTypeCatalog {
    entries: Vec<AircraftType>,
}

impl AircraftTypeCatalog {
    /// Loads the bundled open aircraft type data without a network request.
    pub fn bundled() -> Self {
        let entries = serde_json::from_str(include_str!("../data/aircraft_types.json"))
            .expect("bundled aircraft type data must be valid JSON");
        Self { entries }
    }

    /// Finds designators beginning with `query`, ignoring case.
    pub fn suggestions(&self, query: &str, limit: usize) -> Vec<&AircraftType> {
        let query = query.trim().to_ascii_uppercase();
        if query.is_empty() {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|entry| entry.code.starts_with(&query))
            .take(limit)
            .collect()
    }

    /// Returns the number of distinct bundled designators.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the catalog has no designators.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_type_codes_case_insensitively() {
        let catalog = AircraftTypeCatalog::bundled();
        assert!(catalog.len() > 2_000);
        let matches = catalog.suggestions("a32", 8);
        assert!(matches.iter().any(|entry| entry.code == "A320"));
        assert!(matches.iter().all(|entry| entry.code.starts_with("A32")));
        assert!(catalog.suggestions("", 8).is_empty());
        assert_eq!(catalog.suggestions("a", 3).len(), 3);
    }
}
