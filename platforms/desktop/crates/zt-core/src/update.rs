//! Updates for Windows and Linux, the way Sparkle updates the macOS app: a signed feed, signed installers, checked
//! once a day and installed once nothing is being composed.
//!
//! - The feed (`desktop-update.json` on main, written by release.yml) is an envelope: the feed's JSON as a string and
//!   its Ed25519 signature.  Every installer in it carries its own signature.  Both are made with the Sparkle key, so
//!   [`PUBLIC_KEY`] is the macOS app's `SUPublicEDKey`; nothing unsigned or signed by another key is ever installed.
//! - Installing needs the administrator (Windows) or root (Linux), so the signed-in user's app never installs: it
//!   checks, shows what it found and, when idle, starts the system's updater — a demand-start Windows service any
//!   signed-in user may start, or a systemd unit that polkit lets the active local user start.  That updater runs this
//!   module's [`system_update`] with no input from the user: it fetches the feed itself, verifies it, downloads into a
//!   directory only it can write, verifies the installer and runs it.
//! - Windows: the installer stops the app in every session, installs, then starts it again in each session
//!   (`--relaunch-sessions`).  Linux: the package manager replaces the files; the running app notices its executable
//!   was replaced and starts the new one once idle.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// The feed (`ZT_UPDATE_FEED` overrides it for tests; only the administrator or root can set the updater's
/// environment).
pub const FEED_URL: &str = "https://raw.githubusercontent.com/henry753951/ZhuyinType/main/desktop-update.json";
/// The Sparkle key's public half (platforms/macos/Resources/Info.plist `SUPublicEDKey`).
pub const PUBLIC_KEY: &str = "GM1ROYQ7SH2jz3ul84jJSZuza92/I33Wr2ebEVWZ9tU=";
/// The Windows service and the systemd unit that install updates.
pub const WINDOWS_SERVICE: &str = "ZhuyinTypeUpdate";
pub const LINUX_UNIT: &str = "zhuyintype-update.service";
/// How often the app checks (Sparkle's `SUScheduledCheckInterval` on macOS).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// The settings that turn checking and installing off (`settings.json`).
pub const CHECKS_KEY: &str = "automaticUpdateChecks";
pub const INSTALLS_KEY: &str = "automaticUpdateInstalls";

const FEED_LIMIT: u64 = 1 << 20;
const INSTALLER_LIMIT: u64 = 512 << 20;

/// One installer in the feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub url: String,
    pub length: u64,
    /// Ed25519 over the file's bytes, base64.
    pub signature: String,
}

/// The newest release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feed {
    pub version: String,
    /// The release notes (a web page).
    pub notes: String,
    /// By [`asset_key`]: `windows-x64`, `linux-deb`, `linux-rpm`.
    pub assets: BTreeMap<String, Asset>,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    feed: String,
    signature: String,
}

fn key() -> Result<VerifyingKey> {
    let text = std::env::var("ZT_UPDATE_KEY").ok().filter(|key| !key.is_empty());
    let bytes = STANDARD.decode(text.as_deref().unwrap_or(PUBLIC_KEY)).context("update key")?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("update key: not 32 bytes"))?;
    Ok(VerifyingKey::from_bytes(&bytes)?)
}

/// Whether `signature` (base64) signs `message` with `key`.
fn verify(key: &VerifyingKey, message: &[u8], signature: &str) -> Result<()> {
    let bytes = STANDARD.decode(signature.trim()).context("signature")?;
    let signature = Signature::from_slice(&bytes)?;
    key.verify_strict(message, &signature).map_err(|_| anyhow!("簽章不符"))
}

/// The feed in `envelope`, if its signature is good.
pub fn parse_feed(envelope: &[u8], key: &VerifyingKey) -> Result<Feed> {
    let envelope: Envelope = serde_json::from_slice(envelope).context("update feed")?;
    verify(key, envelope.feed.as_bytes(), &envelope.signature).context("update feed")?;
    let feed: Feed = serde_json::from_str(&envelope.feed).context("update feed")?;
    ensure!(version(&feed.version).is_some(), "update feed: version {}", feed.version);
    Ok(feed)
}

fn version(text: &str) -> Option<[u64; 3]> {
    let mut parts = text.trim_start_matches('v').split('.').map(|part| part.parse::<u64>().ok());
    let version = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(version)
}

/// Whether `candidate` is a later version than `current` (`x.y.z`).
#[must_use]
pub fn newer(candidate: &str, current: &str) -> bool {
    matches!((version(candidate), version(current)), (Some(a), Some(b)) if a > b)
}

/// Which installer fits this installation: Windows, or the Linux package format that installed it.  None for a
/// copy the package manager does not own (a development build), which never updates itself.
#[must_use]
pub fn asset_key() -> Option<&'static str> {
    if cfg!(windows) {
        return Some("windows-x64");
    }
    let owned = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    let deb = std::process::Command::new("dpkg-query")
        .args(["-W", "-f=${Status}", "zhuyintype"])
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.ends_with(b"install ok installed"));
    if deb {
        Some("linux-deb")
    } else if owned("rpm", &["-q", "zhuyintype"]) {
        Some("linux-rpm")
    } else {
        None
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .user_agent(concat!("ZhuyinType/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get(url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = agent().get(url).call().with_context(|| format!("下載 {url}"))?;
    let bytes = response.body_mut().with_config().limit(limit).read_to_vec().with_context(|| format!("下載 {url}"))?;
    Ok(bytes)
}

/// Fetches and verifies the feed.
pub fn fetch_feed() -> Result<Feed> {
    let url = std::env::var("ZT_UPDATE_FEED").ok().filter(|url| !url.is_empty());
    let envelope = get(url.as_deref().unwrap_or(FEED_URL), FEED_LIMIT)?;
    parse_feed(&envelope, &key()?)
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// This version is the newest.
    Current,
    /// A newer version, with its notes.
    Newer { version: String, notes: String },
    /// The installation cannot update itself (a development build, or no installer for it in the feed).
    Unsupported,
}

/// Checks the feed against `current` (the signed-in user's app; installs nothing).
pub fn check(current: &str) -> Result<Found> {
    let Some(asset) = asset_key() else { return Ok(Found::Unsupported) };
    let feed = fetch_feed()?;
    if !feed.assets.contains_key(asset) {
        return Ok(Found::Unsupported);
    }
    Ok(if newer(&feed.version, current) {
        Found::Newer { version: feed.version, notes: feed.notes }
    } else {
        Found::Current
    })
}

/// Downloads `asset` into `directory` (which only the updater can write) and verifies its length and signature
/// before it is written.  Returns the file.
fn download(asset: &Asset, directory: &Path, key: &VerifyingKey) -> Result<PathBuf> {
    ensure!(asset.url.starts_with("https://") || std::env::var_os("ZT_UPDATE_FEED").is_some(), "{}", asset.url);
    let bytes = get(&asset.url, INSTALLER_LIMIT)?;
    ensure!(bytes.len() as u64 == asset.length, "下載的檔案大小不符：{} ≠ {}", bytes.len(), asset.length);
    verify(key, &bytes, &asset.signature).context("installer")?;
    let name = asset
        .url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
    let name = name.ok_or_else(|| anyhow!("檔名：{}", asset.url))?;
    std::fs::create_dir_all(directory)?;
    let path = directory.join(name);
    std::fs::write(&path, &bytes).with_context(|| path.display().to_string())?;
    Ok(path)
}

/// Where the updater keeps the installer: `<install dir>\updates` (Windows; only administrators can write there),
/// `/var/cache/zhuyintype` (Linux, root).
fn download_directory() -> Result<PathBuf> {
    if cfg!(windows) {
        let exe = std::env::current_exe()?;
        Ok(exe.parent().ok_or_else(|| anyhow!("no install directory"))?.join("updates"))
    } else {
        Ok(PathBuf::from("/var/cache/zhuyintype"))
    }
}

/// The system's updater (the Windows service, the systemd unit; administrator or root): installs the newest signed
/// release when it is newer than `current`.  Returns the version it started installing, or None when there was
/// nothing to do.  On Windows the installer runs on after this returns (it replaces this executable); on Linux the
/// package manager has finished when this returns.
pub fn system_update(current: &str) -> Result<Option<String>> {
    let Some(asset_key) = asset_key() else { bail!("這份 ZhuyinType 不是由安裝程式或套件安裝的") };
    let key = key()?;
    let feed = fetch_feed()?;
    if !newer(&feed.version, current) {
        return Ok(None);
    }
    let asset = feed.assets.get(asset_key).ok_or_else(|| anyhow!("更新沒有 {asset_key} 的安裝檔"))?;
    let directory = download_directory()?;
    // Earlier downloads are not kept.
    let _ = std::fs::remove_dir_all(&directory);
    let file = download(asset, &directory, &key)?;
    install(asset_key, &file)?;
    Ok(Some(feed.version))
}

#[cfg(windows)]
fn install(_: &str, installer: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    // /UPDATE: after installing, start the app again in every signed-in session (installer.nsi).
    std::process::Command::new(installer)
        .args(["/S", "/UPDATE"])
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .context("安裝程式")?;
    Ok(())
}

#[cfg(not(windows))]
fn install(asset_key: &str, package: &Path) -> Result<()> {
    let package = package.to_string_lossy().into_owned();
    let have = |program: &str| {
        ["/usr/bin", "/bin", "/usr/sbin", "/sbin"].iter().any(|dir| Path::new(dir).join(program).exists())
    };
    let (program, args): (&str, Vec<&str>) = match asset_key {
        "linux-deb" => ("apt-get", vec!["install", "-y", "--no-install-recommends", &package]),
        "linux-rpm" if have("dnf") => ("dnf", vec!["install", "-y", &package]),
        "linux-rpm" if have("zypper") => {
            ("zypper", vec!["--non-interactive", "install", "--allow-unsigned-rpm", &package])
        }
        "linux-rpm" => ("rpm", vec!["-U", &package]),
        other => bail!("unknown package {other}"),
    };
    let output = std::process::Command::new(program)
        .args(&args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .with_context(|| program.to_owned())?;
    if !output.status.success() {
        bail!("{program}：{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// Starts the system's updater from the signed-in user's app.  It installs the newest release itself; this only
/// asks it to run.
pub fn start_system_update() -> Result<()> {
    #[cfg(windows)]
    {
        windows::start_service()
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("systemctl")
            .args(["start", "--no-block", LINUX_UNIT])
            .status()
            .context("systemctl")?;
        ensure!(status.success(), "systemctl start {LINUX_UNIT}: {status}");
        Ok(())
    }
}

/// Linux: the identity (device, inode) of the file at this process's executable path; it changes when an update
/// replaces the executable, and the app then starts the new one.
#[cfg(unix)]
#[must_use]
pub fn executable_identity() -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    let exe = std::fs::read_link("/proc/self/exe").ok()?;
    let path = PathBuf::from(exe.to_string_lossy().trim_end_matches(" (deleted)"));
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
pub mod windows;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    fn signer() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn envelope(feed: &str, key: &SigningKey) -> Vec<u8> {
        let signature = STANDARD.encode(key.sign(feed.as_bytes()).to_bytes());
        serde_json::to_vec(&serde_json::json!({ "feed": feed, "signature": signature })).unwrap()
    }

    const FEED: &str = r#"{"version":"0.6.1","notes":"https://example.org/notes","assets":{"windows-x64":{"url":"https://example.org/a.exe","length":3,"signature":"x"}}}"#;

    #[test]
    fn a_signed_feed_is_read() {
        let key = signer();
        let feed = parse_feed(&envelope(FEED, &key), &key.verifying_key()).unwrap();
        assert_eq!(feed.version, "0.6.1");
        assert_eq!(feed.assets["windows-x64"].length, 3);
    }

    #[test]
    fn a_changed_or_foreign_feed_is_refused() {
        let key = signer();
        let other = SigningKey::from_bytes(&[8; 32]);
        assert!(parse_feed(&envelope(FEED, &other), &key.verifying_key()).is_err());
        let mut changed: serde_json::Value = serde_json::from_slice(&envelope(FEED, &key)).unwrap();
        changed["feed"] = FEED.replace("0.6.1", "9.9.9").into();
        assert!(parse_feed(&serde_json::to_vec(&changed).unwrap(), &key.verifying_key()).is_err());
        assert!(parse_feed(b"{}", &key.verifying_key()).is_err());
    }

    #[test]
    fn the_shipped_key_parses() {
        let bytes = STANDARD.decode(PUBLIC_KEY).unwrap();
        assert!(VerifyingKey::from_bytes(&bytes.try_into().unwrap()).is_ok());
    }

    #[test]
    fn installer_signatures_are_checked() {
        let key = signer();
        let good = STANDARD.encode(key.sign(b"abc").to_bytes());
        assert!(verify(&key.verifying_key(), b"abc", &good).is_ok());
        assert!(verify(&key.verifying_key(), b"abd", &good).is_err());
        assert!(verify(&key.verifying_key(), b"abc", "not base64").is_err());
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(newer("0.6.1", "0.6.0"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("v1.0.0", "0.99.0"));
        assert!(!newer("0.6.0", "0.6.0"));
        assert!(!newer("0.5.9", "0.6.0"));
        assert!(!newer("0.6", "0.5.0"));
        assert!(!newer("0.6.1-beta", "0.6.0"));
    }
}
