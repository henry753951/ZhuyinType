//! 回報問題: the text the user copies or saves and sends us themselves, as Markdown so it pastes into a GitHub issue
//! as headings and lists (and still reads as plain text in a file).  The same layout as the macOS app's
//! (`HealthReport.swift`):
//!
//! - `## ZhuyinType 問題回報`, then the time, the result, the version, the system and the architecture as a list;
//! - `### 檢查結果`: each check as `- [狀態] 標題：說明`, its details indented under it;
//! - one `### 標題` section per group of facts ([`Section`]: versions and data, installation, the input method,
//!   the service, updates, changed settings, display), each a `- 名稱：值` list; empty ones are left out;
//! - `### 最近的錯誤`: the app's own recent warnings and errors in a ```` ```text ```` block.
//!
//! Nothing personal: no typed text, user phrases, learned words, correction log, word or app lists, or other apps'
//! names.  The home directory is written `~`, and the user name (in other users' paths, Windows' SIDs) `<使用者>`.

use std::path::Path;

use super::{Check, summary};

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

/// One `### 標題` section: `- 名稱：值` lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub lines: Vec<(String, String)>,
}

impl Section {
    #[must_use]
    pub fn new(title: impl Into<String>, lines: Vec<(String, String)>) -> Self {
        Self { title: title.into(), lines }
    }
}

/// Everything the report is made from.
#[derive(Clone, Copy, Debug)]
pub struct Report<'a> {
    pub version: &'a str,
    /// 「2026-11-18 14:03」.
    pub generated: &'a str,
    pub system: &'a System,
    pub checks: &'a [Check],
    pub sections: &'a [Section],
    /// [`super::recent::lines`]; None: they could not be read.
    pub errors: Option<&'a [String]>,
    /// The home directory, written `~`.
    pub home: Option<&'a Path>,
    /// The user's name, written `<使用者>` where it shows outside the home directory.
    pub user: Option<&'a str>,
}

/// The user, as the report writes them.
pub const USER: &str = "<使用者>";

/// The report.
#[must_use]
pub fn text(report: &Report<'_>) -> String {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut header = vec![
        format!("- 產生時間：{}", report.generated),
        format!("- 結果：{}", summary(report.checks)),
        format!("- 版本：{}", report.version),
        format!("- 系統：{}", report.system.os),
    ];
    if !report.system.desktop.is_empty() {
        header.push(format!("- 桌面：{}", report.system.desktop));
    }
    header.push(format!("- 架構：{}", report.system.arch));
    blocks.push(vec!["## ZhuyinType 問題回報".to_owned()]);
    blocks.push(header);
    blocks.push(vec!["### 檢查結果".to_owned()]);
    if !report.checks.is_empty() {
        let mut lines = Vec::new();
        for check in report.checks {
            lines.push(format!("- [{}] {}：{}", check.status.label(), check.title, one_line(&check.detail)));
            lines.extend(check.details.iter().map(|(label, value)| format!("  - {label}：{}", one_line(value))));
        }
        blocks.push(lines);
    }
    for section in report.sections.iter().filter(|section| !section.lines.is_empty()) {
        blocks.push(vec![format!("### {}", section.title)]);
        blocks.push(section.lines.iter().map(|(label, value)| format!("- {label}：{}", one_line(value))).collect());
    }
    blocks.push(vec!["### 最近的錯誤".to_owned()]);
    blocks.push(match report.errors {
        None => vec!["（無法讀取）".to_owned()],
        Some([]) => vec!["（沒有）".to_owned()],
        Some(errors) => std::iter::once("```text".to_owned())
            // A line that is itself a fence would end the block early.
            .chain(errors.iter().map(|line| one_line(line).replace("```", "'''")))
            .chain(std::iter::once("```".to_owned()))
            .collect(),
    });
    let text = blocks.into_iter().map(|block| block.join("\n")).collect::<Vec<_>>().join("\n\n") + "\n";
    scrub(&text, report.home, report.user)
}

/// `value` on one line.
fn one_line(value: &str) -> String {
    value.split(['\n', '\r']).map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// `text` with the home directory written `~` and the user's name (in other paths, Windows' SIDs) `<使用者>`.
#[must_use]
pub fn scrub(text: &str, home: Option<&Path>, user: Option<&str>) -> String {
    let mut text = text.to_owned();
    if let Some(home) = home.and_then(Path::to_str).filter(|home| home.len() > 1) {
        let home = home.trim_end_matches(['/', '\\']);
        text = replace_ignoring_case(&text, home, "~", cfg!(windows) || home.contains('\\'));
        // The same directory with the other separator (a path written by another tool).
        if home.contains('\\') {
            text = replace_ignoring_case(&text, &home.replace('\\', "/"), "~", true);
        }
    }
    if let Some(user) = user.filter(|user| user.chars().count() >= 2) {
        for prefix in ["/home/", "/Users/", "\\Users\\", "/Users\\", "/run/user/"] {
            text = replace_ignoring_case(&text, &format!("{prefix}{user}"), &format!("{prefix}{USER}"), true);
        }
        text = replace_ignoring_case(&text, &format!("@{user}"), &format!("@{USER}"), false);
    }
    without_sids(&text)
}

/// Windows' account SIDs (`S-1-5-21-…`, the user's pipe and profile) written `<使用者>`.
fn without_sids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("S-1-5-21-") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + "S-1-5-21-".len()..];
        let end = tail.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap_or(tail.len());
        out.push_str(USER);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// `text` with every `from` that ends a path component replaced by `to` (`/home/amy` but not `/home/amy2`);
/// `ignore_case` for ASCII letters (Windows paths).
fn replace_ignoring_case(text: &str, from: &str, to: &str, ignore_case: bool) -> String {
    if from.is_empty() {
        return text.to_owned();
    }
    let (haystack, needle) = if ignore_case {
        (text.to_ascii_lowercase(), from.to_ascii_lowercase())
    } else {
        (text.to_owned(), from.to_owned())
    };
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (at, _) in haystack.match_indices(&needle) {
        let end = at + needle.len();
        let continues = text[end..].chars().next().is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'));
        if at < last || continues {
            continue;
        }
        out.push_str(&text[last..at]);
        out.push_str(to);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::super::{Check, Status};
    use super::*;

    fn system() -> System {
        System { os: "Ubuntu 24.04 LTS，Linux 6.8.0".into(), desktop: "GNOME，Wayland".into(), arch: "x86_64".into() }
    }

    #[test]
    fn the_layout_matches_the_macos_report() {
        let checks = [
            Check::ok("service", "輸入法服務正在執行", "打字的 App 可以連到 ZhuyinType。")
                .with_detail("回應時間", "2 ms"),
            Check::new("data", Status::Error, "無法讀取詞庫", "/home/amy/zt/data 無法開啟。")
                .with_detail("資料夾", "/home/amy/zt/data")
                .with_detail("詞庫", "lexicon.bin，無法讀取"),
        ];
        let sections = [
            Section::new("版本與資料", vec![("詞庫".into(), "lexicon.bin，17.6 MB".into())]),
            Section::new("空的", Vec::new()),
            Section::new("服務", vec![("已執行".into(), "2 小時 3 分".into())]),
        ];
        let errors =
            vec!["14:03:02 WARN settings: /home/amy/.config/zhuyintype/settings.json cannot be read".to_owned()];
        let text = text(&Report {
            version: "0.7.1",
            generated: "2026-11-18 14:03",
            system: &system(),
            checks: &checks,
            sections: &sections,
            errors: Some(&errors),
            home: Some(Path::new("/home/amy")),
            user: Some("amy"),
        });
        assert_eq!(
            text,
            "## ZhuyinType 問題回報\n\n\
             - 產生時間：2026-11-18 14:03\n- 結果：有 1 個項目需要處理\n- 版本：0.7.1\n\
             - 系統：Ubuntu 24.04 LTS，Linux 6.8.0\n- 桌面：GNOME，Wayland\n- 架構：x86_64\n\n\
             ### 檢查結果\n\n\
             - [正常] 輸入法服務正在執行：打字的 App 可以連到 ZhuyinType。\n  - 回應時間：2 ms\n\
             - [錯誤] 無法讀取詞庫：~/zt/data 無法開啟。\n  - 資料夾：~/zt/data\n  - 詞庫：lexicon.bin，無法讀取\n\n\
             ### 版本與資料\n\n- 詞庫：lexicon.bin，17.6 MB\n\n\
             ### 服務\n\n- 已執行：2 小時 3 分\n\n\
             ### 最近的錯誤\n\n```text\n\
             14:03:02 WARN settings: ~/.config/zhuyintype/settings.json cannot be read\n```\n"
        );
        assert!(!text.contains("amy"));
        assert!(!text.contains("空的"), "an empty section is left out");
    }

    #[test]
    fn no_errors_and_unreadable_errors() {
        let system = System { os: "Windows 11".into(), desktop: String::new(), arch: "x86_64".into() };
        let report = Report {
            version: "0.7.1",
            generated: "now",
            system: &system,
            checks: &[],
            sections: &[],
            errors: Some(&[]),
            home: None,
            user: None,
        };
        let none = text(&report);
        assert!(none.ends_with("### 最近的錯誤\n\n（沒有）\n"), "{none}");
        assert!(none.contains("- 結果：全部正常\n"));
        assert!(!none.contains("桌面："));
        let unknown = text(&Report { errors: None, home: Some(Path::new("/")), ..report });
        assert!(unknown.ends_with("（無法讀取）\n"));
        assert!(unknown.contains("- 系統：Windows 11"), "a home of / replaces nothing");
    }

    #[test]
    fn windows_paths_sids_and_other_users() {
        let scrubbed = scrub(
            r"C:\Users\Amy\AppData\Roaming\ZhuyinType and c:\users\amy\x, \\.\pipe\ZhuyinType-ime-v1-S-1-5-21-1-2-3-1001, /home/amy2/x /run/user/amy",
            Some(Path::new(r"C:\Users\Amy")),
            Some("amy"),
        );
        assert_eq!(
            scrubbed,
            r"~\AppData\Roaming\ZhuyinType and ~\x, \\.\pipe\ZhuyinType-ime-v1-<使用者>, /home/amy2/x /run/user/<使用者>"
        );
        assert_eq!(scrub("Germany amy", None, Some("amy")), "Germany amy", "only in paths");
    }

    #[test]
    fn a_fence_in_an_error_line_does_not_end_the_block() {
        let errors = vec!["oops ``` here\nsecond".to_owned()];
        let system = system();
        let text = text(&Report {
            version: "0.7.1",
            generated: "now",
            system: &system,
            checks: &[],
            sections: &[],
            errors: Some(&errors),
            home: None,
            user: None,
        });
        assert!(text.ends_with("```text\noops ''' here second\n```\n"), "{text}");
    }
}
