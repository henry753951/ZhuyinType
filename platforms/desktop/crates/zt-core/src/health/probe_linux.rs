//! Linux: reads the facts `linux::checks` judges.  Runs the package's helper (`fcitx5-setup status`) and, on GNOME
//! under Wayland, `gnome-extensions`; reads the session's environment and `/etc/os-release`.  Off the UI thread.

use std::path::Path;
use std::process::Command;

use super::linux::{FcitxProfile, FcitxStatus, IM_VARIABLES, KIMPANEL_EXTENSION, LinuxFacts};
use super::report::System;

/// The facts; `helper` is `fcitx5-setup` when it is installed.
#[must_use]
pub fn probe(helper: Option<&Path>) -> LinuxFacts {
    let fcitx = helper.and_then(|helper| {
        let output = Command::new(helper).arg("status").output().ok()?;
        Some(FcitxStatus::parse(&String::from_utf8_lossy(&output.stdout)))
    });
    let other_modules = ["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"]
        .into_iter()
        .filter_map(|name| {
            let value = std::env::var(name).ok().filter(|value| !value.is_empty())?;
            (!value.contains("fcitx")).then(|| format!("{name}={value}"))
        })
        .collect();
    let wayland = std::env::var("XDG_SESSION_TYPE").is_ok_and(|kind| kind == "wayland");
    let gnome = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.to_uppercase().contains("GNOME"));
    let kimpanel = (wayland && gnome).then(kimpanel_enabled).flatten();
    let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    LinuxFacts {
        fcitx,
        other_modules,
        wayland,
        gnome,
        kimpanel,
        fcitx_version: fcitx_version(),
        profile: profile(),
        im_variables: IM_VARIABLES
            .iter()
            .map(|name| ((*name).to_owned(), std::env::var(name).ok().filter(|value| !value.is_empty())))
            .collect(),
        session_type: std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        wayland_display: set("WAYLAND_DISPLAY"),
        x_display: set("DISPLAY"),
    }
}

/// `fcitx5 --version`, its first line.
fn fcitx_version() -> Option<String> {
    let output = Command::new("fcitx5").arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    (output.status.success() && !line.is_empty()).then(|| line.to_owned())
}

/// This user's Fcitx 5 profile.
fn profile() -> Option<FcitxProfile> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".config")))?;
    std::fs::read_to_string(config.join("fcitx5/profile")).ok().map(|text| FcitxProfile::parse(&text))
}

fn kimpanel_enabled() -> Option<bool> {
    let output = Command::new("gnome-extensions").args(["list", "--enabled"]).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).lines().any(|line| line == KIMPANEL_EXTENSION))
}

/// The distribution and kernel, the desktop and the session type.
#[must_use]
pub fn system() -> System {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let name = release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_owned())
        .unwrap_or_else(|| "Linux".to_owned());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let os = if kernel.trim().is_empty() { name } else { format!("{name}，Linux {}", kernel.trim()) };
    let desktop = [std::env::var("XDG_CURRENT_DESKTOP"), std::env::var("XDG_SESSION_TYPE")]
        .into_iter()
        .filter_map(|value| value.ok().filter(|value| !value.is_empty()))
        .collect::<Vec<_>>()
        .join("，");
    System { os, desktop, arch: std::env::consts::ARCH.to_owned() }
}
