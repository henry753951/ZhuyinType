//! 檢查輸入法: the common reasons typing does not reach ZhuyinType, each explained in a sentence, with a fix where
//! one is safe (the settings window's 關於 › 檢查輸入法, docs/product/DESIGN.md).
//!
//! Reading the system and judging it are kept apart: the `probe` functions (side effects, per system) gather plain
//! facts structs, and the `checks` functions turn facts into [`Check`]s without touching anything, so every verdict
//! is tested with made-up facts on any system.  The app runs a probe off the UI thread, then the checks.
//!
//! | module | what |
//! |---|---|
//! | `common` | the data files, the settings file, the input method service, the version and updates |
//! | `windows` | registered, in this user's list, per-app input, autostart, the update service, Smart App Control |
//! | `linux` | Fcitx 5 running and chosen, ZhuyinType in its list and loaded, Wayland notes |
//! | `report` | 回報問題: the diagnostics text (never typed text or personal data) |
//! | `recent` | the app's own recent warnings and errors, kept in memory for the report |

pub mod common;
pub mod linux;
pub mod recent;
pub mod report;
pub mod windows;

#[cfg(target_os = "linux")]
pub mod probe_linux;
#[cfg(windows)]
pub mod probe_windows;

use std::path::PathBuf;

/// How a check came out, from fine to worst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    /// Works.
    Ok,
    /// Something to know, nothing to fix (a setting that changes how switching behaves).
    Info,
    /// Typing may not work in some places, or something will stop working later.
    Warning,
    /// Typing does not reach ZhuyinType.
    Error,
}

impl Status {
    /// The label the page and the report use.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "正常",
            Status::Info => "說明",
            Status::Warning => "注意",
            Status::Error => "錯誤",
        }
    }

    /// Needs the user's attention (the sidebar's badge, the count on 關於).
    #[must_use]
    pub fn is_problem(self) -> bool {
        self >= Status::Warning
    }
}

/// What a check's button does.  The app carries it out (`zhuyintype::health`); each one changes only this user's
/// own settings, never the system's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    /// Windows: add ZhuyinType to this user's input method list (`regsvr32 /n /i:user`, as Active Setup does).
    AddToInputList,
    /// Linux: make Fcitx 5 this user's input method framework (`fcitx5-setup framework`), from the next sign-in.
    UseFcitx,
    /// Linux: add ZhuyinType to this user's Fcitx 5 input methods (`fcitx5-setup enable`).
    AddToFcitx,
    /// Open a system settings page (`ms-settings:` on Windows) or a web page.
    Open { label: &'static str, url: String },
    /// Show a file in the file manager.
    Reveal(PathBuf),
}

impl Fix {
    /// The button's label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Fix::AddToInputList => "加入輸入法清單",
            Fix::UseFcitx => "改用 Fcitx 5",
            Fix::AddToFcitx => "加入 Fcitx 5",
            Fix::Open { label, .. } => label,
            Fix::Reveal(_) => {
                if cfg!(windows) {
                    "在檔案總管中顯示"
                } else {
                    "在檔案管理員中顯示"
                }
            }
        }
    }
}

/// One check's result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    /// Stable id (anchors, tests).
    pub id: &'static str,
    /// What was found, as a short title (「ZhuyinType 已加入輸入法清單」).
    pub title: String,
    pub status: Status,
    /// One or two sentences: what it means and what to do.
    pub detail: String,
    pub fix: Option<Fix>,
}

impl Check {
    #[must_use]
    pub fn new(id: &'static str, status: Status, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { id, title: title.into(), status, detail: detail.into(), fix: None }
    }

    #[must_use]
    pub fn ok(id: &'static str, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(id, Status::Ok, title, detail)
    }

    #[must_use]
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }
}

/// What the app gives a run of the checks: what it knows itself (the service, the data in use, the settings and
/// the updates); the rest is read from the system.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub version: String,
    pub data: common::DataFacts,
    pub settings: common::SettingsFacts,
    /// `reachable` is filled in by [`run`].
    pub service: common::ServiceFacts,
    /// The broker's endpoint, when the service runs.
    pub endpoint: Option<String>,
    pub updates: common::UpdateFacts,
    /// Linux: the package's `fcitx5-setup`.
    pub fcitx_helper: Option<PathBuf>,
}

/// Reads the system and runs every check (off the UI thread: it runs helpers and connects to the broker).  The
/// system's own checks come first: they are the usual reason typing does not reach ZhuyinType.
#[must_use]
pub fn run(inputs: &Inputs) -> Vec<Check> {
    let mut checks = platform_checks(inputs);
    let mut service = inputs.service.clone();
    if service.running {
        service.reachable = inputs.endpoint.as_deref().map(answers);
    }
    checks.push(common::service_check(&service));
    checks.push(common::data_check(&inputs.data));
    checks.push(common::settings_check(&inputs.settings));
    checks.push(common::update_check(&inputs.version, &inputs.updates));
    checks
}

#[cfg(windows)]
fn platform_checks(_: &Inputs) -> Vec<Check> {
    windows::checks(&probe_windows::probe())
}

#[cfg(target_os = "linux")]
fn platform_checks(inputs: &Inputs) -> Vec<Check> {
    linux::checks(&probe_linux::probe(inputs.fcitx_helper.as_deref()))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_checks(_: &Inputs) -> Vec<Check> {
    Vec::new()
}

/// This computer, for the report.
#[must_use]
pub fn system() -> report::System {
    #[cfg(windows)]
    return probe_windows::system();
    #[cfg(target_os = "linux")]
    return probe_linux::system();
    #[cfg(not(any(windows, target_os = "linux")))]
    report::System {
        os: std::env::consts::OS.to_owned(),
        desktop: String::new(),
        arch: std::env::consts::ARCH.to_owned(),
    }
}

/// Whether the broker at `endpoint` answers a request.  The request is one it does not serve, so it changes
/// nothing; any reply means it is there.
#[must_use]
pub fn answers(endpoint: &str) -> bool {
    crate::broker::Client::connect(endpoint)
        .and_then(|mut client| client.request("health", serde_json::json!({ "context": "health" })))
        .is_ok()
}

/// The number of checks that need attention.
#[must_use]
pub fn problems(checks: &[Check]) -> usize {
    checks.iter().filter(|check| check.status.is_problem()).count()
}

/// The worst status, `Ok` for none.
#[must_use]
pub fn worst(checks: &[Check]) -> Status {
    checks.iter().map(|check| check.status).max().unwrap_or(Status::Ok)
}

/// What 關於 says about the checks: 「全部正常」, or how many need attention.
#[must_use]
pub fn summary(checks: &[Check]) -> String {
    match problems(checks) {
        0 => "全部正常".to_owned(),
        count => format!("有 {count} 個項目需要處理"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_warnings_and_errors_only() {
        assert_eq!(summary(&[]), "全部正常");
        assert_eq!(worst(&[]), Status::Ok);
        let checks = [
            Check::ok("a", "a", ""),
            Check::new("b", Status::Info, "b", ""),
            Check::new("c", Status::Warning, "c", ""),
            Check::new("d", Status::Error, "d", ""),
        ];
        assert_eq!(problems(&checks), 2);
        assert_eq!(summary(&checks), "有 2 個項目需要處理");
        assert_eq!(worst(&checks), Status::Error);
        assert_eq!(summary(&checks[..2]), "全部正常");
    }
}
