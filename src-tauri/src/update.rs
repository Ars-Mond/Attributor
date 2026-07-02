//! Update check (notify-only). Queries the GitHub Releases API for the newest release and compares
//! its numeric version core (`X.Y.Z`, ignoring any `-beta`/`-rc` suffix) against the running app
//! version. It never downloads or installs — it only reports whether a newer version exists and
//! links to the releases page. Pure Rust (reqwest + rustls); no new dependency.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;

// Releases live on GitHub (the release workflows publish there). Include pre-releases so a beta of a
// higher version is seen; the `-beta` suffix is stripped when comparing (see `version_core`).
const RELEASES_URL: &str = "https://api.github.com/repos/Ars-Mond/Attributor/releases?per_page=30";

/// Result of an update check (camelCase across the IPC boundary).
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub url: String,
    pub notes: String,
}

/// One GitHub release — only the fields we use.
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Numeric version core from a tag: strip a leading `v`/`V`, drop everything from the first `-`
/// (e.g. `-beta.2`, `-rc.1`), then read up to three dot-separated numbers. Missing/garbage parts
/// become 0, so `v1.1.0-beta.2` -> (1, 1, 0).
fn version_core(tag: &str) -> (u64, u64, u64) {
    let s = tag.trim().trim_start_matches(['v', 'V']);
    let s = s.split('-').next().unwrap_or("");
    let mut it = s.split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// Check GitHub for the newest release by numeric core. Notify-only: never downloads or installs.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let current_core = version_core(&current);

    let resp = reqwest::Client::new()
        .get(RELEASES_URL)
        .header("User-Agent", "Attributor")
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let releases: Vec<Release> = resp.json().await.map_err(|e| e.to_string())?;

    // Highest numeric core among non-draft releases; on a tie prefer a stable (non-prerelease) one.
    let best = releases.iter().filter(|r| !r.draft).max_by(|a, b| {
        version_core(&a.tag_name)
            .cmp(&version_core(&b.tag_name))
            .then_with(|| (!a.prerelease).cmp(&(!b.prerelease)))
    });

    let Some(rel) = best else {
        return Ok(UpdateInfo { available: false, current_version: current, ..Default::default() });
    };

    let available = version_core(&rel.tag_name) > current_core;
    if available {
        log::info!("update available: {current} -> {}", rel.tag_name);
    }
    Ok(UpdateInfo {
        available,
        current_version: current,
        latest_version: rel.tag_name.trim_start_matches(['v', 'V']).to_string(),
        url: rel.html_url.clone(),
        notes: rel.name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_core_strips_prefix_and_suffix() {
        assert_eq!(version_core("v1.1.0"), (1, 1, 0));
        assert_eq!(version_core("1.2.3"), (1, 2, 3));
        assert_eq!(version_core("v1.1.0-beta.2"), (1, 1, 0)); // beta suffix ignored
        assert_eq!(version_core("v2.0.0-rc.1"), (2, 0, 0));
        assert_eq!(version_core("v1.1"), (1, 1, 0)); // missing patch -> 0
        assert_eq!(version_core("garbage"), (0, 0, 0));
    }

    #[test]
    fn core_comparison_ignores_beta_of_same_version() {
        // A beta of the installed version is NOT newer (equal core), so it is not an update.
        assert!(!(version_core("v1.1.0-beta.2") > version_core("1.1.0")));
        // A higher core (stable or beta) IS newer.
        assert!(version_core("v1.2.0-beta.1") > version_core("1.1.0"));
        assert!(version_core("v1.1.1") > version_core("1.1.0"));
    }
}
