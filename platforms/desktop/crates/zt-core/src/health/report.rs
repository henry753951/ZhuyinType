//! 回報問題: the text the user copies or saves and sends us themselves.  It has the versions, the system, the
//! checks and the app's own recent warnings and errors, and nothing personal: no typed text, user phrases, learned
//! words, correction log or other apps' names; the home directory is written `~`.  The same layout as the macOS
//! app's (`HealthChecks.swift`).

use std::path::Path;

use super::Check;

/// What the report says about this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct System {
    /// 「Windows 11 24H2（組建 26100）」, 「Ubuntu 24.04 LTS，Linux 6.8.0」.
    pub os: String,
    /// 「GNOME，Wayland」 (Linux), empty elsewhere.
    pub desktop: String,
    /// `x86_64`.
    pub arch: String,
}

/// The report.  `errors` are [`super::recent::lines`]; `None`: they could not be read.
#[must_use]
pub fn text(
    version: &str,
    generated: &str,
    system: &System,
    checks: &[Check],
    errors: Option<&[String]>,
    home: Option<&Path>,
) -> String {
    let mut lines = vec![
        "ZhuyinType 問題回報".to_owned(),
        format!("產生時間：{generated}"),
        format!("版本：{version}"),
        format!("系統：{}", system.os),
    ];
    if !system.desktop.is_empty() {
        lines.push(format!("桌面：{}", system.desktop));
    }
    lines.push(format!("架構：{}", system.arch));
    lines.push(String::new());
    lines.push("檢查結果".to_owned());
    lines.extend(checks.iter().map(|check| format!("[{}] {}：{}", check.status.label(), check.title, check.detail)));
    lines.push(String::new());
    lines.push("最近的錯誤".to_owned());
    match errors {
        None => lines.push("（無法讀取）".to_owned()),
        Some([]) => lines.push("（沒有）".to_owned()),
        Some(errors) => lines.extend(errors.iter().cloned()),
    }
    let text = lines.join("\n") + "\n";
    match home.and_then(Path::to_str).filter(|home| home.len() > 1) {
        Some(home) => text.replace(home, "~"),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Check, Status};
    use super::*;

    #[test]
    fn the_layout_matches_the_macos_report() {
        let system = System {
            os: "Ubuntu 24.04 LTS，Linux 6.8.0".into(),
            desktop: "GNOME，Wayland".into(),
            arch: "x86_64".into(),
        };
        let checks = [
            Check::ok("service", "輸入法服務正在執行", "打字的 App 可以連到 ZhuyinType。"),
            Check::new("data", Status::Error, "無法讀取詞庫", "/home/amy/zt/data 無法開啟。"),
        ];
        let errors = vec!["14:03:02 settings: /home/amy/.config/zhuyintype/settings.json cannot be read".to_owned()];
        let text = text("0.6.1", "2026-11-18 14:03", &system, &checks, Some(&errors), Some(Path::new("/home/amy")));
        assert_eq!(
            text,
            "ZhuyinType 問題回報\n產生時間：2026-11-18 14:03\n版本：0.6.1\n系統：Ubuntu 24.04 LTS，Linux 6.8.0\n\
             桌面：GNOME，Wayland\n架構：x86_64\n\n檢查結果\n[正常] 輸入法服務正在執行：打字的 App 可以連到 ZhuyinType。\n\
             [錯誤] 無法讀取詞庫：~/zt/data 無法開啟。\n\n最近的錯誤\n\
             14:03:02 settings: ~/.config/zhuyintype/settings.json cannot be read\n"
        );
        assert!(!text.contains("amy"));
    }

    #[test]
    fn no_errors_and_unreadable_errors() {
        let system = System { os: "Windows 11".into(), desktop: String::new(), arch: "x86_64".into() };
        let none = text("0.6.1", "now", &system, &[], Some(&[]), None);
        assert!(none.ends_with("最近的錯誤\n（沒有）\n"));
        assert!(!none.contains("桌面："));
        let unknown = text("0.6.1", "now", &system, &[], None, Some(Path::new("/")));
        assert!(unknown.ends_with("（無法讀取）\n"));
        assert!(unknown.contains("系統：Windows 11"), "a home of / replaces nothing");
    }
}
