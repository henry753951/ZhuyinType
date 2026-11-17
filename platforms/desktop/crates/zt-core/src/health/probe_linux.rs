//! Linux: reads the facts `linux::checks` judges.  Runs the package's helper (`fcitx5-setup status`) and, on GNOME
//! under Wayland, `gnome-extensions`; reads the session's environment and `/etc/os-release`.  Off the UI thread.

use std::path::Path;
use std::process::Command;

use super::linux::{FcitxStatus, KIMPANEL_EXTENSION, LinuxFacts};
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
    LinuxFacts { fcitx, other_modules, wayland, gnome, kimpanel }
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
