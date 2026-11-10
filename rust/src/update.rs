//! Opt-in, user-controlled update checks for official GitHub releases.
//!
//! The caller authorizes downloads by confirmation or a saved automatic-install
//! preference. Only verified release payloads are staged; this worker never
//! launches installers or replaces application files.

use semver::Version;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[path = "update_http.rs"]
mod update_http;

pub use crate::settings::DEFAULT_UPDATE_REPOSITORY as DEFAULT_REPOSITORY;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
pub(crate) const MAX_EXECUTABLE_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    #[default]
    Executable,
    Msi,
}
impl AssetKind {
    pub fn asset_name(self, version: &str) -> String {
        match self {
            Self::Executable => "claude-mascot-windows-x64.exe".into(),
            Self::Msi => format!("claude-mascot-{version}-x64.msi"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub kind: AssetKind,
    pub executable_sha256: Option<String>,
    pub version: String,
    pub tag: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedUpdate {
    pub kind: AssetKind,
    pub executable_sha256: Option<String>,
    pub executable: std::path::PathBuf,
    pub sha256: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateEvent {
    Checked {
        release: Option<ReleaseInfo>,
        manual: bool,
    },
    Downloaded(StagedUpdate),
    Error {
        message: String,
        manual: bool,
    },
}

enum Command {
    Check { manual: bool },
    Download(ReleaseInfo, bool),
    SetEnabled(bool),
    Stop,
}

/// Background update service. Startup and periodic checks are enabled only
/// when `enabled` is true. Downloads require an explicit caller request.
pub struct UpdateService {
    commands: Sender<Command>,
    events: Receiver<UpdateEvent>,
    worker: Option<JoinHandle<()>>,
}

impl UpdateService {
    pub fn new(repo: String, enabled: bool) -> Self {
        Self::new_for_kind(repo, enabled, AssetKind::Executable)
    }

    pub fn new_for_kind(repo: String, enabled: bool, kind: AssetKind) -> Self {
        Self::new_deferred(repo, enabled, kind, false)
    }

    pub fn new_deferred(repo: String, enabled: bool, kind: AssetKind, skip_first: bool) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let worker_events = event_tx.clone();
        let worker = thread::Builder::new()
            .name("mascot-update-check".into())
            .spawn(move || {
                update_worker(repo, enabled, kind, skip_first, command_rx, worker_events)
            });
        match worker {
            Ok(worker) => Self {
                commands: command_tx,
                events: event_rx,
                worker: Some(worker),
            },
            Err(error) => {
                let _ = event_tx.send(UpdateEvent::Error {
                    message: format!("Cannot start update worker: {error}"),
                    manual: false,
                });
                Self {
                    commands: command_tx,
                    events: event_rx,
                    worker: None,
                }
            }
        }
    }

    /// Queue an immediate check regardless of the automatic-update preference.
    pub fn request_check(&self) {
        let _ = self.commands.send(Command::Check { manual: true });
    }

    /// Queue a download after the UI has explicitly obtained user consent.
    pub fn request_download(&self, release: ReleaseInfo) {
        self.request_download_with_mode(release, true);
    }

    pub fn request_download_with_mode(&self, release: ReleaseInfo, manual: bool) {
        let _ = self.commands.send(Command::Download(release, manual));
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.commands.send(Command::SetEnabled(enabled));
    }

    /// Drain completed events without waiting for network or disk operations.
    pub fn poll(&self) -> Vec<UpdateEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for UpdateService {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        // Network calls have finite WinHTTP timeouts. Detaching keeps UI shutdown
        // nonblocking even if the operating system is currently in a slow call.
        let _ = self.worker.take();
    }
}

fn update_worker(
    repo: String,
    initially_enabled: bool,
    kind: AssetKind,
    skip_first: bool,
    commands: Receiver<Command>,
    events: Sender<UpdateEvent>,
) {
    let mut enabled = initially_enabled;
    let mut next_check = Instant::now()
        + if skip_first {
            CHECK_INTERVAL
        } else {
            Duration::ZERO
        };
    loop {
        let timeout = if enabled {
            next_check.saturating_duration_since(Instant::now())
        } else {
            Duration::from_secs(24 * 60 * 60)
        };
        match commands.recv_timeout(timeout) {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::SetEnabled(value)) => {
                if value && !enabled {
                    next_check = Instant::now();
                }
                enabled = value;
            }
            Ok(Command::Check { manual }) => {
                let result = check_latest(&repo, kind);
                send_check_result(&events, result, manual);
                if enabled {
                    next_check = Instant::now() + CHECK_INTERVAL;
                }
            }
            Ok(Command::Download(release, manual)) => {
                let result = if release.kind == kind {
                    download_release(&repo, &release)
                } else {
                    Err("Update package type does not match this installation".into())
                };
                let event = match result {
                    Ok(staged) => UpdateEvent::Downloaded(staged),
                    Err(message) => UpdateEvent::Error { message, manual },
                };
                let _ = events.send(event);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let result = check_latest(&repo, kind);
                send_check_result(&events, result, false);
                next_check = Instant::now() + CHECK_INTERVAL;
            }
        }
    }
}

fn send_check_result(
    events: &Sender<UpdateEvent>,
    result: Result<Option<ReleaseInfo>, String>,
    manual: bool,
) {
    let event = match result {
        Ok(release) => UpdateEvent::Checked { release, manual },
        Err(message) => UpdateEvent::Error { message, manual },
    };
    let _ = events.send(event);
}

fn check_latest(repo: &str, kind: AssetKind) -> Result<Option<ReleaseInfo>, String> {
    #[cfg(windows)]
    {
        let json = update_http::fetch_latest_json(repo)?;
        let Some(json) = json else {
            return Ok(None);
        };
        parse_release_for_kind(&json, repo, env!("CARGO_PKG_VERSION"), kind)
    }
    #[cfg(not(windows))]
    {
        let _ = (repo, kind);
        Err("Automatic update checks are supported on Windows only".into())
    }
}

fn download_release(repo: &str, release: &ReleaseInfo) -> Result<StagedUpdate, String> {
    #[cfg(windows)]
    {
        update_http::download_and_stage(repo, release)
    }
    #[cfg(not(windows))]
    {
        let _ = (repo, release);
        Err("Update downloads are supported on Windows only".into())
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

/// Validate GitHub release metadata without performing I/O.
pub fn parse_release(json: &str, repo: &str, current: &str) -> Result<Option<ReleaseInfo>, String> {
    parse_release_for_kind(json, repo, current, AssetKind::Executable)
}

pub fn parse_release_for_kind(
    json: &str,
    repo: &str,
    current: &str,
    kind: AssetKind,
) -> Result<Option<ReleaseInfo>, String> {
    validate_repository(repo)?;
    let release: GithubRelease = serde_json::from_str(json)
        .map_err(|error| format!("Invalid GitHub release metadata: {error}"))?;
    if release.draft || release.prerelease {
        return Ok(None);
    }

    let version_text = release
        .tag_name
        .strip_prefix("rust-v")
        .or_else(|| release.tag_name.strip_prefix('v'))
        .ok_or_else(|| "Release tag must start with v or rust-v".to_string())?;
    let version = Version::parse(version_text)
        .map_err(|error| format!("Invalid release version: {error}"))?;
    if !version.pre.is_empty() {
        return Ok(None);
    }
    let current = Version::parse(current)
        .map_err(|error| format!("Invalid current application version: {error}"))?;
    if version <= current {
        return Ok(None);
    }

    if kind == AssetKind::Msi
        && (!version.build.is_empty()
            || version.major > 255
            || version.minor > 255
            || version.patch > 65535)
    {
        return Err("Release version cannot be represented by Windows Installer".into());
    }
    let asset_name = kind.asset_name(&version.to_string());
    let mut matching = release
        .assets
        .iter()
        .filter(|asset| asset.name == asset_name);
    let asset = matching
        .next()
        .ok_or_else(|| format!("Release is missing required asset {asset_name}"))?;
    if matching.next().is_some() {
        return Err("Release contains ambiguous duplicate Windows assets".into());
    }
    if asset.size == 0 || asset.size > MAX_EXECUTABLE_SIZE {
        return Err(format!(
            "Release asset size is outside the allowed limit: {}",
            asset.size
        ));
    }
    let expected_url = format!(
        "https://github.com/{repo}/releases/download/{}/{asset_name}",
        release.tag_name
    );
    if asset.browser_download_url != expected_url {
        return Err("Release asset URL is not the canonical repository download URL".into());
    }
    let digest = asset
        .digest
        .as_deref()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or_else(|| "Release asset is missing its SHA-256 digest".to_string())?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Release asset SHA-256 digest is malformed".into());
    }

    Ok(Some(ReleaseInfo {
        kind,
        executable_sha256: if kind == AssetKind::Msi {
            Some(
                parse_release_for_kind(json, repo, &current.to_string(), AssetKind::Executable)?
                    .ok_or("Missing companion executable metadata")?
                    .sha256,
            )
        } else {
            None
        },
        version: version.to_string(),
        tag: release.tag_name,
        url: expected_url,
        sha256: digest.to_ascii_lowercase(),
        size: asset.size,
    }))
}

fn validate_repository(repo: &str) -> Result<(), String> {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err("Repository must be an owner/name pair".into());
    };
    let valid_component = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && !part.starts_with('.')
            && !part.ends_with('.')
            && !part.contains("..")
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if valid_component(owner) && valid_component(name) {
        Ok(())
    } else {
        Err("Repository contains invalid owner or name characters".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_release, validate_repository};
    use serde_json::json;

    fn msi_fixture() -> serde_json::Value {
        let mut value: serde_json::Value = serde_json::from_str(&fixture(
            "1.2.3",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        ))
        .unwrap();
        value["assets"].as_array_mut().unwrap().push(json!({
            "name":"claude-mascot-1.2.3-x64.msi", "size":2048,
            "digest":format!("sha256:{}", "b".repeat(64)),
            "browser_download_url":"https://github.com/owner/repo/releases/download/v1.2.3/claude-mascot-1.2.3-x64.msi"
        }));
        value
    }

    #[test]
    fn msi_update_selects_versioned_installer_and_requires_both_digests() {
        let value = msi_fixture();
        let release = super::parse_release_for_kind(
            &value.to_string(),
            "owner/repo",
            "1.0.0",
            super::AssetKind::Msi,
        )
        .unwrap()
        .unwrap();
        assert_eq!(release.kind, super::AssetKind::Msi);
        assert!(release.url.ends_with("claude-mascot-1.2.3-x64.msi"));
        assert_eq!(release.sha256, "b".repeat(64));
        assert_eq!(release.executable_sha256, Some("a".repeat(64)));
        for index in [0, 1] {
            let mut broken = value.clone();
            broken["assets"][index]["digest"] = serde_json::Value::Null;
            assert!(super::parse_release_for_kind(
                &broken.to_string(),
                "owner/repo",
                "1.0.0",
                super::AssetKind::Msi
            )
            .is_err());
        }
    }

    #[test]
    fn msi_update_rejects_wrong_url_name_duplicates_and_oversize() {
        for (key, invalid) in [
            (
                "browser_download_url",
                json!("https://attacker.invalid/package.msi"),
            ),
            ("name", json!("claude-mascot-1.2.2-x64.msi")),
            ("size", json!(64 * 1024 * 1024 + 1)),
        ] {
            let mut value = msi_fixture();
            value["assets"][1][key] = invalid;
            assert!(super::parse_release_for_kind(
                &value.to_string(),
                "owner/repo",
                "1.0.0",
                super::AssetKind::Msi
            )
            .is_err());
        }
        let mut duplicate = msi_fixture();
        let asset = duplicate["assets"][1].clone();
        duplicate["assets"].as_array_mut().unwrap().push(asset);
        assert!(super::parse_release_for_kind(
            &duplicate.to_string(),
            "owner/repo",
            "1.0.0",
            super::AssetKind::Msi
        )
        .is_err());
    }

    fn fixture(
        version: &str,
        repo: &str,
        asset_name: &str,
        digest: Option<&str>,
        size: u64,
    ) -> String {
        let tag = format!("v{version}");
        let asset = json!({
            "name": asset_name,
            "browser_download_url": format!("https://github.com/{repo}/releases/download/{tag}/{asset_name}"),
            "size": size,
            "digest": digest,
        });
        json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "assets": [asset],
        })
        .to_string()
    }

    #[test]
    fn accepts_canonical_stable_update_with_required_digest() {
        let json = fixture(
            "0.2.1",
            "kerimovernes/claude-mascot",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        );
        let update = parse_release(&json, "kerimovernes/claude-mascot", "0.2.0")
            .unwrap()
            .unwrap();
        assert_eq!(update.version, "0.2.1");
        assert_eq!(update.sha256, "a".repeat(64));
    }

    #[test]
    fn rejects_wrong_asset_name_and_noncanonical_url() {
        let wrong_name = fixture(
            "0.2.1",
            "owner/repo",
            "other.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        );
        assert!(parse_release(&wrong_name, "owner/repo", "0.2.0").is_err());

        let mut value: serde_json::Value = serde_json::from_str(&fixture(
            "0.2.1",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        ))
        .unwrap();
        value["assets"][0]["browser_download_url"] = json!("https://evil.example/file.exe");
        assert!(parse_release(&value.to_string(), "owner/repo", "0.2.0").is_err());
    }

    #[test]
    fn rejects_missing_or_malformed_digest() {
        for digest in [None, Some("sha256:1234")] {
            let json = fixture(
                "0.2.1",
                "owner/repo",
                "claude-mascot-windows-x64.exe",
                digest,
                1024,
            );
            assert!(parse_release(&json, "owner/repo", "0.2.0").is_err());
        }
    }

    #[test]
    fn rejects_invalid_old_and_prerelease_versions() {
        let malformed = fixture(
            "not-semver",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        );
        assert!(parse_release(&malformed, "owner/repo", "0.2.0").is_err());
        let old = fixture(
            "0.1.9",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        );
        assert_eq!(parse_release(&old, "owner/repo", "0.2.0").unwrap(), None);

        let mut pre: serde_json::Value = serde_json::from_str(&fixture(
            "0.3.0-rc.1",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            1024,
        ))
        .unwrap();
        pre["prerelease"] = json!(true);
        assert_eq!(
            parse_release(&pre.to_string(), "owner/repo", "0.2.0").unwrap(),
            None
        );
    }

    #[test]
    fn rejects_bad_repository_and_oversize_asset() {
        for repo in ["", "owner", "owner/name/extra", "../repo", "owner/name?x=y"] {
            assert!(validate_repository(repo).is_err(), "accepted {repo}");
        }
        let oversize = fixture(
            "0.2.1",
            "owner/repo",
            "claude-mascot-windows-x64.exe",
            Some(&format!("sha256:{}", "a".repeat(64))),
            64 * 1024 * 1024 + 1,
        );
        assert!(parse_release(&oversize, "owner/repo", "0.2.0").is_err());
    }
}
