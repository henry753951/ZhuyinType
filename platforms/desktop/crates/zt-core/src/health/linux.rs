//! Linux: typing reaches ZhuyinType through Fcitx 5, so it must be installed, running as this session's input
//! method framework, have ZhuyinType in its list and have loaded its addon.  The facts come from the package's
//! helper (`fcitx5-setup status`, packaging/linux) and the session's environment (`probe_linux`).

use super::{Check, Fix, Status};

/// The GNOME Shell extension that shows Fcitx's candidate list at the caret on Wayland.
pub const KIMPANEL_EXTENSION: &str = "kimpanel@kde.org";
const KIMPANEL_PAGE: &str = "https://extensions.gnome.org/extension/261/kimpanel/";

/// What `fcitx5-setup status` printed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FcitxStatus {
    pub installed: bool,
    pub running: bool,
    /// The addon is loaded (None: not known, Fcitx 5 is not running).
    pub addon_loaded: Option<bool>,
    /// ZhuyinType is in the user's Fcitx 5 profile.
    pub in_profile: bool,
    /// `fcitx5`, `fcitx5-next-login`, `other` or `unknown`.
    pub framework: String,
}

impl FcitxStatus {
    /// Reads the helper's `key=value` lines.
    #[must_use]
    pub fn parse(status: &str) -> Self {
        let value = |key: &str| {
            status.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix('=')).unwrap_or_default().trim()
        };
        Self {
            installed: value("fcitx5") == "installed",
            running: value("running") == "yes",
            addon_loaded: match value("addon") {
                "loaded" => Some(true),
                "not-loaded" => Some(false),
                _ => None,
            },
            in_profile: value("profile") == "added",
            framework: value("framework").to_owned(),
        }
    }
}

/// What `probe_linux` read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinuxFacts {
    /// The helper's answer (None: the helper is not installed, as in a development build).
    pub fcitx: Option<FcitxStatus>,
    /// `GTK_IM_MODULE`, `QT_IM_MODULE` and `XMODIFIERS` that name another framework (`GTK_IM_MODULE=ibus`).
    pub other_modules: Vec<String>,
    /// `XDG_SESSION_TYPE` is `wayland`.
    pub wayland: bool,
    /// `XDG_CURRENT_DESKTOP` contains GNOME.
    pub gnome: bool,
    /// The Kimpanel extension is enabled (GNOME on Wayland only; None: not asked).
    pub kimpanel: Option<bool>,
}

/// The checks, in the order the page lists them.
#[must_use]
pub fn checks(facts: &LinuxFacts) -> Vec<Check> {
    let Some(fcitx) = &facts.fcitx else {
        return vec![Check::new(
            "fcitx",
            Status::Info,
            "無法檢查 Fcitx 5",
            "找不到套件附的 fcitx5-setup；這份 ZhuyinType 可能不是由套件安裝的。",
        )];
    };
    if !fcitx.installed {
        return vec![Check::new(
            "fcitx",
            Status::Error,
            "沒有安裝 Fcitx 5",
            "ZhuyinType 透過 Fcitx 5 接收按鍵。請以套件管理員安裝 fcitx5，再登出、登入一次。",
        )];
    }
    let mut checks = vec![framework(fcitx)];
    checks.extend(modules(facts, fcitx));
    checks.push(profile(fcitx));
    checks.extend(addon(fcitx));
    checks.extend(wayland(facts));
    checks
}

fn framework(fcitx: &FcitxStatus) -> Check {
    if fcitx.running {
        return Check::ok("fcitx", "Fcitx 5 正在執行", "這個工作階段的輸入法框架是 Fcitx 5。");
    }
    if fcitx.framework == "fcitx5-next-login" {
        return Check::new(
            "fcitx",
            Status::Warning,
            "Fcitx 5 下次登入後啟用",
            "輸入法框架已設為 Fcitx 5，登出再登入一次後就能用 ZhuyinType 打字。",
        );
    }
    Check::new(
        "fcitx",
        Status::Error,
        "Fcitx 5 沒有在執行",
        "打字時不會用到 ZhuyinType。這台電腦的輸入法框架可能是 IBus（GNOME 的預設）；改用 Fcitx 5 後，登出再登入一次即可。",
    )
    .with_fix(Fix::UseFcitx)
}

/// Programs told to use another framework do not reach Fcitx 5.
fn modules(facts: &LinuxFacts, fcitx: &FcitxStatus) -> Option<Check> {
    if facts.other_modules.is_empty() || !fcitx.running {
        return None;
    }
    Some(
        Check::new(
            "modules",
            Status::Warning,
            "有些程式會用其他輸入法框架",
            format!(
                "環境變數 {} 指向其他輸入法框架，這些程式中無法使用 ZhuyinType。改用 Fcitx 5 後，登出再登入一次即可。",
                facts.other_modules.join("、")
            ),
        )
        .with_fix(Fix::UseFcitx),
    )
}

fn profile(fcitx: &FcitxStatus) -> Check {
    if fcitx.in_profile {
        Check::ok(
            "fcitx-list",
            "已加入 Fcitx 5 的輸入法",
            "按 Ctrl＋空白鍵（或 Fcitx 5 設定的切換鍵）切換到 ZhuyinType。",
        )
    } else {
        Check::new(
            "fcitx-list",
            Status::Error,
            "沒有加入 Fcitx 5 的輸入法",
            "Fcitx 5 的輸入法清單中沒有 ZhuyinType。加入後就能切換到它，原有的輸入法都保留。",
        )
        .with_fix(Fix::AddToFcitx)
    }
}

fn addon(fcitx: &FcitxStatus) -> Option<Check> {
    match fcitx.addon_loaded? {
        true => Some(Check::ok("addon", "Fcitx 5 已載入 ZhuyinType", "按鍵會送到 ZhuyinType 的輸入法服務。")),
        false => Some(
            Check::new(
                "addon",
                Status::Error,
                "Fcitx 5 沒有載入 ZhuyinType",
                "Fcitx 5 可能在安裝 ZhuyinType 之前就已啟動。重新載入後即可使用；仍然不行時請重新安裝套件。",
            )
            .with_fix(Fix::AddToFcitx),
        ),
    }
}

/// Wayland notes: where the candidate list appears, and the programs that need a flag.
fn wayland(facts: &LinuxFacts) -> Vec<Check> {
    if !facts.wayland {
        return Vec::new();
    }
    let mut checks = Vec::new();
    if facts.gnome && facts.kimpanel == Some(false) {
        checks.push(
            Check::new(
                "kimpanel",
                Status::Info,
                "候選字清單可能不在游標旁",
                "GNOME 的 Wayland 工作階段需要「Input Method Panel」（Kimpanel）擴充功能，才會把候選字清單畫在游標旁。",
            )
            .with_fix(Fix::Open { label: "開啟擴充功能頁面", url: KIMPANEL_PAGE.to_owned() }),
        );
    }
    checks.push(Check::new(
        "wayland-apps",
        Status::Info,
        "Wayland 上的 Chrome 與 Electron App",
        "以 Wayland 執行時，這些程式要加上 --enable-wayland-ime 參數才會使用輸入法。",
    ));
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(running: &str, addon: &str, profile: &str, framework: &str) -> FcitxStatus {
        FcitxStatus::parse(&format!(
            "fcitx5=installed\nrunning={running}\naddon={addon}\nprofile={profile}\nframework={framework}\n"
        ))
    }

    fn fine() -> LinuxFacts {
        LinuxFacts {
            fcitx: Some(status("yes", "loaded", "added", "fcitx5")),
            other_modules: Vec::new(),
            wayland: false,
            gnome: false,
            kimpanel: None,
        }
    }

    fn find<'a>(checks: &'a [Check], id: &str) -> Option<&'a Check> {
        checks.iter().find(|check| check.id == id)
    }

    #[test]
    fn the_helpers_status_is_read() {
        let parsed = status("yes", "not-loaded", "absent", "other");
        assert!(parsed.installed && parsed.running && !parsed.in_profile);
        assert_eq!((parsed.addon_loaded, parsed.framework.as_str()), (Some(false), "other"));
        assert_eq!(FcitxStatus::parse(""), FcitxStatus::default());
        assert!(!FcitxStatus::parse("fcitx5=missing\n").installed);
    }

    #[test]
    fn a_working_setup_has_no_problems() {
        let checks = checks(&fine());
        assert_eq!(super::super::problems(&checks), 0, "{checks:#?}");
        let ids: Vec<_> = checks.iter().map(|check| check.id).collect();
        assert_eq!(ids, ["fcitx", "fcitx-list", "addon"]);
    }

    #[test]
    fn ibus_offers_to_switch_to_fcitx() {
        let facts = LinuxFacts {
            fcitx: Some(status("no", "unknown", "added", "other")),
            other_modules: vec!["GTK_IM_MODULE=ibus".into()],
            ..fine()
        };
        let checks = checks(&facts);
        let check = find(&checks, "fcitx").unwrap();
        assert_eq!((check.status, check.fix.clone()), (Status::Error, Some(Fix::UseFcitx)));
        assert!(find(&checks, "modules").is_none(), "one fix for one cause");
        assert!(find(&checks, "addon").is_none(), "not known while Fcitx is not running");
        let next =
            super::checks(&LinuxFacts { fcitx: Some(status("no", "unknown", "added", "fcitx5-next-login")), ..fine() });
        assert_eq!(find(&next, "fcitx").unwrap().status, Status::Warning);
    }

    #[test]
    fn other_modules_while_fcitx_runs() {
        let checks = checks(&LinuxFacts { other_modules: vec!["QT_IM_MODULE=ibus".into()], ..fine() });
        let check = find(&checks, "modules").unwrap();
        assert!(check.detail.contains("QT_IM_MODULE=ibus"));
        assert_eq!(check.fix, Some(Fix::UseFcitx));
    }

    #[test]
    fn missing_from_the_list_or_not_loaded_offers_to_add_it() {
        let absent = checks(&LinuxFacts { fcitx: Some(status("yes", "loaded", "absent", "fcitx5")), ..fine() });
        assert_eq!(find(&absent, "fcitx-list").unwrap().fix, Some(Fix::AddToFcitx));
        let unloaded = checks(&LinuxFacts { fcitx: Some(status("yes", "not-loaded", "added", "fcitx5")), ..fine() });
        assert_eq!(find(&unloaded, "addon").unwrap().status, Status::Error);
    }

    #[test]
    fn no_fcitx_or_no_helper() {
        let missing = checks(&LinuxFacts { fcitx: Some(FcitxStatus::parse("fcitx5=missing\n")), ..fine() });
        assert_eq!((missing.len(), missing[0].status), (1, Status::Error));
        let unknown = checks(&LinuxFacts { fcitx: None, ..fine() });
        assert_eq!((unknown.len(), unknown[0].status), (1, Status::Info));
    }

    #[test]
    fn wayland_notes() {
        let gnome = checks(&LinuxFacts { wayland: true, gnome: true, kimpanel: Some(false), ..fine() });
        assert_eq!(find(&gnome, "kimpanel").unwrap().status, Status::Info);
        assert!(find(&gnome, "wayland-apps").is_some());
        assert_eq!(super::super::problems(&gnome), 0, "notes, not problems");
        let with_extension = checks(&LinuxFacts { wayland: true, gnome: true, kimpanel: Some(true), ..fine() });
        assert!(find(&with_extension, "kimpanel").is_none());
    }
}
