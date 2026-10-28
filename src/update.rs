//! GitHub Releases version checks for the Rust ovPilot application.

use semver::Version;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::{Result, VERSION};

const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/dancebunny98/ovPilot/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// Outcome of comparing the current version with the latest GitHub Release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// The repository has no published stable release yet.
    NoRelease,
    /// The current application is at least as new as the latest release.
    Current { latest: Version, url: String },
    /// A newer stable release is available.
    Available { latest: Version, url: String },
}

/// Queries the project's latest published GitHub Release.
#[derive(Debug, Default, Clone, Copy)]
pub struct UpdateChecker;

impl UpdateChecker {
    /// Creates a checker with an embedded HTTPS client.
    pub fn new() -> Self {
        Self
    }

    /// Compares the running application with the latest stable release.
    pub fn check(&self) -> Result<UpdateStatus> {
        let tls = ureq::native_tls::TlsConnector::new()?;
        let response = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .tls_connector(Arc::new(tls))
            .build()
            .get(LATEST_RELEASE_URL)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", &format!("ovPilot/{VERSION}"))
            .call();
        match response {
            Ok(response) => classify(serde_json::from_reader(response.into_reader())?, VERSION),
            Err(ureq::Error::Status(404, _)) => Ok(UpdateStatus::NoRelease),
            Err(error) => Err(format!("GitHub release request failed: {error}").into()),
        }
    }
}

fn classify(release: GitHubRelease, current: &str) -> Result<UpdateStatus> {
    let latest = Version::parse(release.tag_name.trim_start_matches('v'))?;
    let current = Version::parse(current)?;
    if latest > current {
        Ok(UpdateStatus::Available {
            latest,
            url: release.html_url,
        })
    } else {
        Ok(UpdateStatus::Current {
            latest,
            url: release.html_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_newer_tag() {
        let release: GitHubRelease = serde_json::from_str(
            r#"{"tag_name":"v1.1.0","html_url":"https://github.com/dancebunny98/ovPilot/releases/tag/v1.1.0"}"#,
        )
        .unwrap();
        assert!(matches!(
            classify(release, "1.0.0").unwrap(),
            UpdateStatus::Available { .. }
        ));
    }

    #[test]
    fn rejects_malformed_release_data() {
        let release: GitHubRelease = serde_json::from_str(
            r#"{"tag_name":"latest","html_url":"https://github.com/dancebunny98/ovPilot"}"#,
        )
        .unwrap();
        assert!(classify(release, "1.0.0").is_err());
    }

    #[test]
    #[ignore = "requires access to GitHub"]
    fn connects_to_github_over_https() {
        UpdateChecker::new().check().unwrap();
    }
}
