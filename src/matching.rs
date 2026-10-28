//! Custom `.vmr` aircraft model matching with ordered rule sets.

use crate::{AircraftModel, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Indexed installed aircraft models used to resolve rule model names.
pub struct ModelCatalog {
    models: Vec<AircraftModel>,
    by_name: HashMap<String, usize>,
}

impl ModelCatalog {
    /// Builds a case-insensitive model-name index from an aircraft scan.
    pub fn new(models: Vec<AircraftModel>) -> Self {
        let mut by_name = HashMap::new();
        for (index, model) in models.iter().enumerate() {
            by_name
                .entry(model.name.to_ascii_lowercase())
                .or_insert(index);
        }
        Self { models, by_name }
    }

    /// Returns the number of installed aircraft variants.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns whether there are no installed aircraft variants.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    fn by_name(&self, name: &str) -> Option<&AircraftModel> {
        self.by_name
            .get(&name.to_ascii_lowercase())
            .map(|index| &self.models[*index])
    }
}

/// One custom rule loaded from a `.vmr` file.
#[derive(Debug, Clone)]
struct ModelRule {
    callsign_prefix: Option<String>,
    type_code: String,
    flight_number_range: Option<(u32, u32)>,
    model_names: Vec<String>,
}

impl ModelRule {
    fn applies(&self, callsign: &str, type_code: &str) -> bool {
        if !self.type_code.eq_ignore_ascii_case(type_code) {
            return false;
        }
        let suffix = if let Some(prefix) = &self.callsign_prefix {
            let Some(suffix) = callsign
                .get(..prefix.len())
                .filter(|head| head.eq_ignore_ascii_case(prefix))
                .and_then(|_| callsign.get(prefix.len()..))
            else {
                return false;
            };
            suffix
        } else {
            callsign
        };
        self.flight_number_range.is_none_or(|(start, end)| {
            suffix
                .parse::<u32>()
                .is_ok_and(|number| (start..=end).contains(&number))
        })
    }
}

/// Parsed contents of one custom model matching rule set.
#[derive(Debug, Clone)]
pub struct ModelRuleSet {
    name: String,
    rules: Vec<ModelRule>,
}

impl ModelRuleSet {
    /// Reads and validates a `.vmr` XML file.
    pub fn load(path: &Path) -> Result<Self> {
        let xml = fs::read_to_string(path)?;
        Self::parse(
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
            &xml,
        )
    }

    /// Parses a named XML rule set using the documented vPilot VMR schema.
    pub fn parse(name: &str, xml: &str) -> Result<Self> {
        let document = roxmltree::Document::parse(xml)?;
        if !document.root_element().has_tag_name("ModelMatchRuleSet") {
            return Err("VMR root must be ModelMatchRuleSet".into());
        }
        let mut rules = Vec::new();
        for node in document
            .root_element()
            .children()
            .filter(|node| node.has_tag_name("ModelMatchRule"))
        {
            let type_code = node
                .attribute("TypeCode")
                .filter(|value| !value.trim().is_empty())
                .ok_or("VMR rule has no TypeCode")?
                .trim()
                .to_ascii_uppercase();
            let model_names = node
                .attribute("ModelName")
                .ok_or("VMR rule has no ModelName")?
                .split("//")
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if model_names.is_empty() {
                return Err("VMR rule has no usable ModelName".into());
            }
            let flight_number_range = node
                .attribute("FlightNumberRange")
                .map(|range| {
                    let (start, end) = range
                        .split_once('-')
                        .ok_or("invalid VMR FlightNumberRange")?;
                    let start = start.parse::<u32>()?;
                    let end = end.parse::<u32>()?;
                    if start > end {
                        return Err("invalid VMR FlightNumberRange".into());
                    }
                    Ok::<_, crate::Error>((start, end))
                })
                .transpose()?;
            rules.push(ModelRule {
                callsign_prefix: node
                    .attribute("CallsignPrefix")
                    .filter(|value| !value.is_empty())
                    .map(str::to_ascii_uppercase),
                type_code,
                flight_number_range,
                model_names,
            });
        }
        Ok(Self {
            name: name.into(),
            rules,
        })
    }

    /// Returns the display name associated with the rule set.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of rules in the set.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns whether the set has no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Applies custom rule sets in priority order before any automatic fallback.
#[derive(Default)]
pub struct ModelMatcher {
    rule_sets: Vec<ModelRuleSet>,
}

impl ModelMatcher {
    /// Creates a matcher from rule sets ordered from highest to lowest priority.
    pub fn new(rule_sets: Vec<ModelRuleSet>) -> Self {
        Self { rule_sets }
    }

    /// Finds an installed model for a network aircraft's callsign and type code.
    pub fn match_custom<'a>(
        &self,
        callsign: &str,
        type_code: &str,
        catalog: &'a ModelCatalog,
    ) -> Option<&'a AircraftModel> {
        for set in &self.rule_sets {
            for rule in &set.rules {
                if !rule.applies(callsign, type_code) {
                    continue;
                }
                let available = rule
                    .model_names
                    .iter()
                    .filter_map(|name| catalog.by_name(name))
                    .collect::<Vec<_>>();
                if !available.is_empty() {
                    let hash = callsign.bytes().fold(2166136261_u32, |hash, byte| {
                        (hash ^ u32::from(byte)).wrapping_mul(16777619)
                    });
                    return Some(available[(hash as usize) % available.len()]);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelFormat;
    use std::path::PathBuf;

    #[test]
    fn matches_documented_vmr_rules_by_priority_and_flight_number() {
        let xml = r#"<ModelMatchRuleSet>
            <ModelMatchRule CallsignPrefix="SKW" FlightNumberRange="4439-4858" TypeCode="CRJ2" ModelName="Missing//Skywest CRJ" />
            <ModelMatchRule TypeCode="C172" ModelName="Cessna Skyhawk" />
        </ModelMatchRuleSet>"#;
        let set = ModelRuleSet::parse("Airline", xml).unwrap();
        assert_eq!(set.len(), 2);
        let catalog = ModelCatalog::new(vec![AircraftModel {
            name: "Skywest CRJ".into(),
            path: PathBuf::from("aircraft.cfg"),
            format: ModelFormat::SimObjects,
        }]);
        let matcher = ModelMatcher::new(vec![set]);
        assert_eq!(
            matcher
                .match_custom("SKW4500", "CRJ2", &catalog)
                .unwrap()
                .name,
            "Skywest CRJ"
        );
        assert!(matcher.match_custom("SKW5000", "CRJ2", &catalog).is_none());
        assert!(matcher.match_custom("SKW4500", "B738", &catalog).is_none());
    }

    #[test]
    fn rejects_invalid_or_unusable_rules() {
        assert!(ModelRuleSet::parse("Bad", "<Other />").is_err());
        assert!(ModelRuleSet::parse(
            "Bad",
            "<ModelMatchRuleSet><ModelMatchRule TypeCode=\"A320\" /></ModelMatchRuleSet>"
        )
        .is_err());
    }
}
