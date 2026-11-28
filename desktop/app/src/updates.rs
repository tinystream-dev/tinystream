// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use semver::Version;
use serde::Deserialize;

use crate::servers::{Release, Store};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DAY: u64 = 24 * 60 * 60;

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn mismatch(server: &str) -> bool {
    match (Version::parse(VERSION), Version::parse(server.trim_start_matches('v'))) {
        (Ok(app), Ok(server)) => (app.major, app.minor) != (server.major, server.minor),
        _ => false,
    }
}

pub fn newer(version: &str) -> bool {
    match (Version::parse(VERSION), Version::parse(version.trim_start_matches('v'))) {
        (Ok(app), Ok(latest)) => latest > app,
        _ => false,
    }
}

pub async fn check(store: Arc<Store>) -> Result<Option<Release>> {
    let config = store.config.lock().await.clone();
    if !config.check_updates || now().saturating_sub(config.last_check) < DAY {
        return Ok(config.release.filter(|release| newer(&release.version)));
    }
    // Persist the attempt before I/O, so failures don't cause repeated requests at startup.
    store.save(|config| config.last_check = now()).await?;
    #[derive(Deserialize)]
    struct GithubRelease {
        tag_name: String,
        html_url: String,
        draft: bool,
        prerelease: bool,
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("tinystream-desktop/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client
        .get("https://api.github.com/repos/tinystream-dev/tinystream/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    let release = if response.status() == reqwest::StatusCode::NOT_FOUND {
        None
    } else {
        let release: GithubRelease = response.error_for_status()?.json().await?;
        let trusted = release.html_url.starts_with("https://github.com/tinystream-dev/tinystream/releases/tag/");
        (trusted && !release.draft && !release.prerelease && newer(&release.tag_name))
            .then(|| Release { version: release.tag_name.trim_start_matches('v').into(), url: release.html_url })
    };
    store.save(|config| config.release = release.clone()).await?;
    Ok(release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compatibility_and_release_ordering() {
        let app = Version::parse(VERSION).unwrap();
        assert!(!mismatch(&format!("{}.{}.999", app.major, app.minor)));
        assert!(mismatch(&format!("{}.{}.0", app.major, app.minor + 1)));
        assert!(newer(&format!("v{}.{}.0", app.major + 1, app.minor)));
        assert!(!newer(VERSION));
        assert!(!mismatch("unknown"));
    }
}
