//! Filesystem inventory for aircraft models installed with supported simulators.

use std::fs;
use std::path::{Path, PathBuf};

/// Simulator family identified from an aircraft model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// X-Plane 11 or 12 `.acf` aircraft file.
    XPlane,
    /// Microsoft Flight Simulator or Prepar3D `aircraft.cfg` file.
    SimObjects,
}

/// One aircraft variant found during a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AircraftModel {
    /// Aircraft title from a `[fltsim.*]` entry, `.acf` filename, or parent folder.
    pub name: String,
    /// Absolute or root-relative path to the model file.
    pub path: PathBuf,
    /// Model format indicated by its filename.
    pub format: ModelFormat,
}

/// Complete inventory produced from the requested simulator directories.
#[derive(Debug, Default, Clone)]
pub struct ModelScanResult {
    /// Aircraft variants discovered in the requested roots.
    pub models: Vec<AircraftModel>,
    /// Directories that could not be read.
    pub unreadable_directories: Vec<PathBuf>,
}

/// Scans simulator directories without following directory symlinks.
#[derive(Debug, Default)]
pub struct ModelScanner;

impl ModelScanner {
    /// Recursively discovers X-Plane `.acf` and SimObjects `aircraft.cfg` files.
    pub fn scan(&self, roots: &[PathBuf]) -> ModelScanResult {
        let mut result = ModelScanResult::default();
        let mut pending: Vec<(PathBuf, usize)> =
            roots.iter().cloned().map(|root| (root, 0)).collect();
        while let Some((directory, depth)) = pending.pop() {
            if depth > 64 {
                result.unreadable_directories.push(directory);
                continue;
            }
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(_) => {
                    result.unreadable_directories.push(directory);
                    continue;
                }
            };
            for entry in entries {
                let Ok(entry) = entry else { continue };
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if kind.is_dir() {
                    pending.push((path, depth + 1));
                } else if kind.is_file() {
                    result.models.extend(identify_models(&path));
                }
            }
        }
        result
            .models
            .sort_by(|left, right| left.path.cmp(&right.path).then(left.name.cmp(&right.name)));
        result
    }
}

fn identify_models(path: &Path) -> Vec<AircraftModel> {
    let Some(filename) = path.file_name() else {
        return Vec::new();
    };
    if filename
        .to_string_lossy()
        .eq_ignore_ascii_case("aircraft.cfg")
    {
        let titles = fs::read(path)
            .map(|bytes| parse_fltsim_titles(&String::from_utf8_lossy(&bytes)))
            .unwrap_or_default();
        let titles = if titles.is_empty() {
            vec![path
                .parent()
                .and_then(Path::file_name)
                .unwrap_or(filename)
                .to_string_lossy()
                .into_owned()]
        } else {
            titles
        };
        titles
            .into_iter()
            .map(|name| AircraftModel {
                name,
                path: path.to_path_buf(),
                format: ModelFormat::SimObjects,
            })
            .collect()
    } else if path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("acf"))
    {
        vec![AircraftModel {
            name: path
                .file_stem()
                .unwrap_or(filename)
                .to_string_lossy()
                .into_owned(),
            path: path.to_path_buf(),
            format: ModelFormat::XPlane,
        }]
    } else {
        Vec::new()
    }
}

fn parse_fltsim_titles(content: &str) -> Vec<String> {
    let mut in_fltsim = false;
    let mut titles = Vec::new();
    for line in content.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_fltsim = line[1..line.len() - 1]
                .trim()
                .to_ascii_lowercase()
                .starts_with("fltsim.");
        } else if in_fltsim && !line.starts_with(';') {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("title") {
                    let title = value
                        .split(';')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim();
                    if !title.is_empty() {
                        titles.push(title.to_owned());
                    }
                }
            }
        }
    }
    titles
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_real_model_files_and_ignores_other_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("ovpilot-model-scan-{}-{nonce}", std::process::id()));
        let xplane = root.join("X-Plane 12/Aircraft/Baron");
        let msfs = root.join("Community/SimObjects/Airplanes/A320");
        fs::create_dir_all(&xplane).unwrap();
        fs::create_dir_all(&msfs).unwrap();
        fs::write(xplane.join("Baron.acf"), b"ACF").unwrap();
        fs::write(msfs.join("aircraft.cfg"), b"[GENERAL]").unwrap();
        fs::write(msfs.join("readme.txt"), b"not a model").unwrap();

        let result = ModelScanner.scan(std::slice::from_ref(&root));
        assert_eq!(result.models.len(), 2);
        assert!(result
            .models
            .iter()
            .any(|model| model.format == ModelFormat::XPlane));
        assert!(result
            .models
            .iter()
            .any(|model| model.format == ModelFormat::SimObjects));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn counts_each_aircraft_cfg_fltsim_title() {
        let titles = parse_fltsim_titles("[General]\ntitle=ignored\n[fltsim.0]\ntitle = A320 Default\n[FLTSIM.1]\nTitle=A320 Airline ; comment\n[fltsim.2]\n; title=disabled\n");
        assert_eq!(titles, ["A320 Default", "A320 Airline"]);
    }
}
