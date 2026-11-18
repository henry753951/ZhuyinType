//! Windows: whether the text service is registered and in this user's input method list, and the system settings
//! that change where typing goes.  The facts come from `probe_windows` (registry, service manager,
//! `SystemParametersInfo`); judging them is here so it runs anywhere.

use super::{Check, Fix, Status};

/// Microsoft Bopomofo's profile (`0404:{CLSID}{PROFILE}`), the system's own Bopomofo input method.
pub const MICROSOFT_BOPOMOFO: &str =
    "0404:{B115690A-EA02-48D5-A231-E3578D2FDF80}{B2F9C502-1742-11D4-9790-0080C882687E}";
/// ZhuyinType's profile as Windows lists it (crates/zt-tsf/src/module.rs: `SERVICE_ID`, `PROFILE_ID`).
pub const ZHUYINTYPE: &str = "0404:{25BC3548-B611-4F92-91D0-C9291D427EA8}{E8207CF3-7CE3-42AE-9B18-453284A73901}";

/// Smart App Control (`HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy`, `VerifiedAndReputablePolicyState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SmartAppControl {
    #[default]
    Off,
    /// Watching, not blocking yet.
    Evaluation,
    On,
}

/// What `probe_windows` read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsFacts {
    /// The 64-bit text service is registered (its COM class and TSF profile) and its DLL exists.
    pub registered_64: bool,
    /// The same for the 32-bit one (32-bit applications).
    pub registered_32: bool,
    /// ZhuyinType's profile is in this user's input method list (`HKCU\Control Panel\International\User Profile`).
    /// None when the list could not be read.
    pub in_user_list: Option<bool>,
    /// The profiles in this user's list, in order (`0404:{…}{…}`, or a keyboard layout `0404:00000404`).
    pub user_list: Vec<String>,
    /// The default input method chosen in Settings (`InputMethodOverride`), if one is.
    pub default_override: Option<String>,
    /// 「允許我為每個應用程式視窗使用不同的輸入法」 (`SPI_GETTHREADLOCALINPUTSETTINGS`).
    pub per_app_input: Option<bool>,
    /// The sign-in entry that starts `zhuyintype --background` (HKLM or HKCU `Run`).
    pub autostart: bool,
    /// The installer's update service exists (`ZhuyinTypeUpdate`).
    pub update_service: bool,
    /// Installed by the installer (its Active Setup entry is there); a development copy has none of the above.
    pub installed: bool,
    pub smart_app_control: SmartAppControl,
    /// Code Integrity events naming our files (blocked or would be blocked), when the log could be read.
    pub blocked_files: Option<Vec<String>>,
    /// The DLLs the 64- and 32-bit COM classes name (registered or not).
    pub dll_64: Option<String>,
    pub dll_32: Option<String>,
    /// This user's languages, in order (`zh-Hant-TW`, `en-US`).
    pub languages: Vec<String>,
    /// The program's folder and our files in it, with their sizes (None: missing).
    pub install_dir: Option<String>,
    pub install_files: Vec<(String, Option<u64>)>,
    /// Who may read the program's folder: app containers (`ALL APPLICATION PACKAGES`, which Start's search and
    /// Store apps run as) and users, from its security descriptor; None when it could not be read.
    pub folder_access: Option<FolderAccess>,
}

/// Read access to the program's folder, as its security descriptor grants it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FolderAccess {
    /// `ALL APPLICATION PACKAGES` (`AC`).
    pub app_packages: bool,
    /// `ALL RESTRICTED APPLICATION PACKAGES` (`S-1-15-2-2`).
    pub restricted_packages: bool,
    /// `Users` (`BU`).
    pub users: bool,
}

impl FolderAccess {
    /// Reads an SDDL string's allow entries.
    #[must_use]
    pub fn from_sddl(sddl: &str) -> Self {
        let allows = |trustee: &str| {
            sddl.split('(')
                .filter_map(|ace| ace.strip_suffix(')').or(Some(ace)))
                .any(|ace| ace.starts_with("A;") && ace.trim_end_matches(')').ends_with(&format!(";;;{trustee}")))
        };
        Self {
            app_packages: allows("AC") || allows("S-1-15-2-1"),
            restricted_packages: allows("S-1-15-2-2"),
            users: allows("BU") || allows("S-1-5-32-545"),
        }
    }
}

/// A profile ID with its name when it is a known one.
#[must_use]
pub fn profile_name(profile: &str) -> String {
    if profile.eq_ignore_ascii_case(ZHUYINTYPE) {
        format!("ZhuyinType（{profile}）")
    } else if profile.eq_ignore_ascii_case(MICROSOFT_BOPOMOFO) {
        format!("微軟注音（{profile}）")
    } else {
        profile.to_owned()
    }
}

/// The report's 輸入法 lines.
#[must_use]
pub fn input_lines(facts: &WindowsFacts) -> Vec<(String, String)> {
    let list = |items: &[String]| if items.is_empty() { "不明".to_owned() } else { items.join("、") };
    let profiles: Vec<String> = facts.user_list.iter().map(|profile| profile_name(profile)).collect();
    vec![
        ("語言".to_owned(), list(&facts.languages)),
        ("輸入法清單".to_owned(), list(&profiles)),
        (
            "預設的輸入法".to_owned(),
            facts.default_override.as_deref().map_or_else(|| "清單中的第一個".to_owned(), profile_name),
        ),
        ("每個應用程式視窗使用不同的輸入法".to_owned(), facts.per_app_input.map_or("不明", super::on_off).to_owned()),
        ("文字服務（64 位元）".to_owned(), facts.dll_64.clone().unwrap_or_else(|| "沒有登記".to_owned())),
        ("文字服務（32 位元）".to_owned(), facts.dll_32.clone().unwrap_or_else(|| "沒有登記".to_owned())),
        ("智慧型應用程式控制".to_owned(), smart_app_control_text(facts.smart_app_control).to_owned()),
    ]
}

fn smart_app_control_text(state: SmartAppControl) -> &'static str {
    match state {
        SmartAppControl::Off => "關閉",
        SmartAppControl::Evaluation => "評估中",
        SmartAppControl::On => "開啟",
    }
}

/// The report's 安裝 lines: the folder, who may read it, our files in it.
#[must_use]
pub fn install_lines(facts: &WindowsFacts) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    if let Some(dir) = &facts.install_dir {
        lines.push(("資料夾".to_owned(), dir.clone()));
    }
    lines.push(("由安裝程式安裝".to_owned(), super::yes_no(facts.installed).to_owned()));
    if let Some(access) = facts.folder_access {
        let who: Vec<&str> = [
            (access.users, "使用者"),
            (access.app_packages, "所有應用程式套件"),
            (access.restricted_packages, "受限的應用程式套件"),
        ]
        .into_iter()
        .filter_map(|(allowed, name)| allowed.then_some(name))
        .collect();
        lines.push((
            "可讀取資料夾".to_owned(),
            if who.is_empty() { "只有系統管理員".to_owned() } else { who.join("、") },
        ));
    }
    for (name, size) in &facts.install_files {
        let value = size.map_or_else(|| "缺少".to_owned(), super::details::size_text);
        lines.push((name.clone(), value));
    }
    lines.push(("登入時啟動".to_owned(), super::yes_no(facts.autostart).to_owned()));
    lines.push(("更新服務".to_owned(), super::yes_no(facts.update_service).to_owned()));
    lines
}

/// The checks, in the order the page lists them.
#[must_use]
pub fn checks(facts: &WindowsFacts) -> Vec<Check> {
    let shown = |dll: &Option<String>, registered| match (dll, registered) {
        (Some(path), true) => path.clone(),
        (Some(path), false) => format!("{path}（找不到檔案）"),
        (None, _) => "沒有登記".to_owned(),
    };
    let registration = registration(facts)
        .with_detail("64 位元", shown(&facts.dll_64, facts.registered_64))
        .with_detail("32 位元", shown(&facts.dll_32, facts.registered_32));
    let profiles: Vec<String> = facts.user_list.iter().map(|profile| profile_name(profile)).collect();
    let user_list = user_list(facts)
        .with_detail("在清單中", facts.in_user_list.map_or("不明", super::yes_no))
        .with_detail("輸入法清單", if profiles.is_empty() { "不明".to_owned() } else { profiles.join("、") });
    let mut checks = vec![registration, user_list];
    checks.extend(other_default(facts));
    checks.push(per_app(facts));
    if facts.installed {
        checks.push(autostart(facts));
        checks.push(update_service(facts));
    }
    checks.extend(smart_app_control(facts));
    checks
}

fn registration(facts: &WindowsFacts) -> Check {
    match (facts.registered_64, facts.registered_32) {
        (true, true) => Check::ok("registered", "文字服務已登記", "64 與 32 位元的程式都能使用 ZhuyinType。"),
        (true, false) => Check::new(
            "registered",
            Status::Warning,
            "32 位元的文字服務沒有登記",
            "較舊的 32 位元程式中無法使用 ZhuyinType。請重新安裝 ZhuyinType。",
        ),
        (false, _) => Check::new(
            "registered",
            Status::Error,
            "文字服務沒有登記",
            "Windows 找不到 ZhuyinType 的文字服務，打字時不會用到它。請重新安裝 ZhuyinType。",
        ),
    }
}

fn user_list(facts: &WindowsFacts) -> Check {
    match facts.in_user_list {
        Some(true) => Check::ok(
            "user-list",
            "已加入你的輸入法清單",
            "按 Windows 鍵＋空白鍵，或點工作列的輸入指示器，就能切換到 ZhuyinType。",
        ),
        Some(false) => Check::new(
            "user-list",
            Status::Error,
            "沒有加入你的輸入法清單",
            "這個帳號的輸入法清單中沒有 ZhuyinType（以系統管理員密碼安裝時，常只加到系統管理員的清單）。加入後就能切換到它。",
        )
        .with_fix(Fix::AddToInputList),
        None => Check::new(
            "user-list",
            Status::Warning,
            "無法讀取你的輸入法清單",
            "請在「設定 › 時間與語言 › 語言與地區」確認「中文（繁體，台灣）」的鍵盤中有 ZhuyinType。",
        )
        .with_fix(region_settings()),
    }
}

/// Another input method is the default: informational, switching still works.
fn other_default(facts: &WindowsFacts) -> Option<Check> {
    let default = facts.default_override.as_deref().or_else(|| facts.user_list.first().map(String::as_str))?;
    if default.eq_ignore_ascii_case(ZHUYINTYPE) || facts.in_user_list != Some(true) {
        return None;
    }
    let name = if default.eq_ignore_ascii_case(MICROSOFT_BOPOMOFO) { "微軟注音" } else { "另一個輸入法" };
    let from = if facts.default_override.is_some() { "進階鍵盤設定" } else { "清單中的第一個" };
    Some(
        Check::new(
            "default",
            Status::Info,
            format!("預設的輸入法是{name}"),
            format!(
                "登入後與新開的視窗先用{name}，需要時按 Windows 鍵＋空白鍵切換到 ZhuyinType。要一開始就用 ZhuyinType，在「進階鍵盤設定」把它設為預設的輸入法。"
            ),
        )
        .with_fix(advanced_keyboard())
        .with_detail("預設的輸入法", profile_name(default))
        .with_detail("來自", from),
    )
}

fn per_app(facts: &WindowsFacts) -> Check {
    per_app_verdict(facts)
        .with_detail("每個應用程式視窗使用不同的輸入法", facts.per_app_input.map_or("不明", super::on_off))
}

fn per_app_verdict(facts: &WindowsFacts) -> Check {
    match facts.per_app_input {
        Some(true) => Check::new(
            "per-app",
            Status::Info,
            "每個視窗各自記住輸入法",
            "「允許我為每個應用程式視窗使用不同的輸入法」已開啟：切換到 ZhuyinType 只對目前的視窗有效，其他視窗和工作列可能仍顯示原本的輸入法（例如 ENG）。",
        )
        .with_fix(advanced_keyboard()),
        _ => Check::ok("per-app", "所有視窗使用同一個輸入法", "切換到 ZhuyinType 後，每個視窗都用它。"),
    }
}

fn autostart(facts: &WindowsFacts) -> Check {
    autostart_verdict(facts).with_detail("登入項目", super::yes_no(facts.autostart))
}

fn autostart_verdict(facts: &WindowsFacts) -> Check {
    if facts.autostart {
        Check::ok("autostart", "登入時啟動", "登入後輸入法服務會自動執行。")
    } else {
        Check::new(
            "autostart",
            Status::Warning,
            "登入時不會啟動",
            "登入後要先開啟「ZhuyinType 設定」才能用 ZhuyinType 打字。重新安裝 ZhuyinType 可以補回。",
        )
    }
}

fn update_service(facts: &WindowsFacts) -> Check {
    update_service_verdict(facts).with_detail("ZhuyinTypeUpdate 服務", super::yes_no(facts.update_service))
}

fn update_service_verdict(facts: &WindowsFacts) -> Check {
    if facts.update_service {
        Check::ok("update-service", "更新服務已安裝", "新版本可以在背景安裝，不需要系統管理員密碼。")
    } else {
        Check::new(
            "update-service",
            Status::Warning,
            "更新服務沒有安裝",
            "無法自動安裝新版本。重新安裝 ZhuyinType 可以補回。",
        )
    }
}

fn smart_app_control(facts: &WindowsFacts) -> Option<Check> {
    let blocked = match &facts.blocked_files {
        None => "無法讀取記錄".to_owned(),
        Some(files) if files.is_empty() => "沒有".to_owned(),
        Some(files) => files.join("、"),
    };
    smart_app_control_verdict(facts).map(|check| {
        check.with_detail("狀態", smart_app_control_text(facts.smart_app_control)).with_detail("被擋下的檔案", blocked)
    })
}

fn smart_app_control_verdict(facts: &WindowsFacts) -> Option<Check> {
    if let Some(files) = facts.blocked_files.as_ref().filter(|files| !files.is_empty()) {
        return Some(
            Check::new(
                "smart-app-control",
                Status::Error,
                "Windows 擋下了 ZhuyinType 的檔案",
                format!(
                    "{} 被系統的應用程式控制擋下，部分程式中無法使用 ZhuyinType。可以在「Windows 安全性 › 應用程式與瀏覽器控制」查看。",
                    files.join("、")
                ),
            )
            .with_fix(app_control()),
        );
    }
    match facts.smart_app_control {
        SmartAppControl::On => Some(
            Check::new(
                "smart-app-control",
                Status::Warning,
                "智慧型應用程式控制已開啟",
                "它可能擋下沒有數位簽章的程式，ZhuyinType 目前沒有簽章。若在某些程式中無法打字，請到「Windows 安全性」查看封鎖記錄。",
            )
            .with_fix(app_control()),
        ),
        SmartAppControl::Evaluation | SmartAppControl::Off => None,
    }
}

/// Our file names in Code Integrity's events (`wevtutil qe … /f:text`), each once, sorted.
#[must_use]
pub fn blocked_files_in(log: &str) -> Vec<String> {
    let mut files: Vec<String> = log
        .split(|c: char| c.is_whitespace() || c == '"')
        .filter_map(|word| {
            let name = word.rsplit(['\\', '/']).next()?.trim_end_matches(['.', ',', ';']);
            let lower = name.to_ascii_lowercase();
            (lower.contains("zhuyintype") && (lower.ends_with(".dll") || lower.ends_with(".exe")))
                .then(|| name.to_owned())
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

fn region_settings() -> Fix {
    Fix::Open { label: "開啟語言設定", url: "ms-settings:regionlanguage".to_owned() }
}

fn advanced_keyboard() -> Fix {
    Fix::Open { label: "開啟進階鍵盤設定", url: "ms-settings:typing".to_owned() }
}

fn app_control() -> Fix {
    Fix::Open { label: "開啟 Windows 安全性", url: "windowsdefender://smartapp/".to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fine() -> WindowsFacts {
        WindowsFacts {
            registered_64: true,
            registered_32: true,
            in_user_list: Some(true),
            user_list: vec![ZHUYINTYPE.to_owned()],
            default_override: None,
            per_app_input: Some(false),
            autostart: true,
            update_service: true,
            installed: true,
            smart_app_control: SmartAppControl::Off,
            blocked_files: Some(Vec::new()),
            ..WindowsFacts::default()
        }
    }

    fn find<'a>(checks: &'a [Check], id: &str) -> Option<&'a Check> {
        checks.iter().find(|check| check.id == id)
    }

    #[test]
    fn a_working_installation_has_no_problems() {
        let checks = checks(&fine());
        assert_eq!(super::super::problems(&checks), 0, "{checks:#?}");
        let ids: Vec<_> = checks.iter().map(|check| check.id).collect();
        assert_eq!(ids, ["registered", "user-list", "per-app", "autostart", "update-service"]);
    }

    #[test]
    fn missing_from_this_users_list_offers_to_add_it() {
        let facts = WindowsFacts { in_user_list: Some(false), user_list: vec![MICROSOFT_BOPOMOFO.into()], ..fine() };
        let checks = checks(&facts);
        let check = find(&checks, "user-list").unwrap();
        assert_eq!((check.status, check.fix.clone()), (Status::Error, Some(Fix::AddToInputList)));
        assert!(find(&checks, "default").is_none(), "no advice about the default before it is in the list");
        let unknown = super::checks(&WindowsFacts { in_user_list: None, ..fine() });
        assert_eq!(find(&unknown, "user-list").unwrap().status, Status::Warning);
    }

    #[test]
    fn registration_of_each_bitness() {
        let no_32 = checks(&WindowsFacts { registered_32: false, ..fine() });
        assert_eq!(find(&no_32, "registered").unwrap().status, Status::Warning);
        let none = checks(&WindowsFacts { registered_64: false, registered_32: false, ..fine() });
        assert_eq!(find(&none, "registered").unwrap().status, Status::Error);
    }

    #[test]
    fn another_default_input_method_is_information() {
        let bopomofo = WindowsFacts { user_list: vec![MICROSOFT_BOPOMOFO.into(), ZHUYINTYPE.into()], ..fine() };
        let check = find(&checks(&bopomofo), "default").cloned().unwrap();
        assert_eq!((check.status, check.title.as_str()), (Status::Info, "預設的輸入法是微軟注音"));
        let chosen = WindowsFacts { default_override: Some(ZHUYINTYPE.to_lowercase()), ..bopomofo.clone() };
        assert!(find(&checks(&chosen), "default").is_none(), "the override wins over the list's order");
        let other = WindowsFacts { default_override: Some("0409:00000409".into()), ..bopomofo };
        assert_eq!(find(&checks(&other), "default").unwrap().title, "預設的輸入法是另一個輸入法");
    }

    #[test]
    fn per_app_input_is_explained() {
        let checks = checks(&WindowsFacts { per_app_input: Some(true), ..fine() });
        let check = find(&checks, "per-app").unwrap();
        assert_eq!(check.status, Status::Info);
        assert!(check.detail.contains("ENG"));
    }

    #[test]
    fn a_development_copy_skips_what_the_installer_sets_up() {
        let checks = checks(&WindowsFacts { installed: false, autostart: false, update_service: false, ..fine() });
        assert!(find(&checks, "autostart").is_none());
        assert!(find(&checks, "update-service").is_none());
        let installed = super::checks(&WindowsFacts { autostart: false, update_service: false, ..fine() });
        assert_eq!(super::super::problems(&installed), 2);
    }

    #[test]
    fn our_files_are_found_in_the_code_integrity_log() {
        let log = "Event[0]:\n  Description:\nCode Integrity determined that a process (\\Device\\HarddiskVolume3\\Windows\\explorer.exe) \
                   attempted to load \\Device\\HarddiskVolume3\\Program Files\\ZhuyinType\\zhuyintype-tsf-x64.dll that did \
                   not meet the Microsoft signing level requirements.\nEvent[1]:\n  ... C:\\Program Files\\ZhuyinType\\zhuyintype.exe.\n\
                   Event[2]: ... \\Device\\HarddiskVolume3\\Program Files\\ZhuyinType\\zhuyintype-tsf-x64.dll again";
        assert_eq!(blocked_files_in(log), ["zhuyintype-tsf-x64.dll", "zhuyintype.exe"]);
        assert!(blocked_files_in("Event[0]: other.dll blocked").is_empty());
        assert!(blocked_files_in("").is_empty());
    }

    #[test]
    fn checks_and_lines_say_what_was_found() {
        let facts = WindowsFacts {
            dll_64: Some(r"C:\Program Files\ZhuyinType\zhuyintype-tsf-x64.dll".into()),
            dll_32: None,
            registered_32: false,
            user_list: vec![MICROSOFT_BOPOMOFO.into(), ZHUYINTYPE.into(), "0409:00000409".into()],
            languages: vec!["zh-Hant-TW".into(), "en-US".into()],
            ..fine()
        };
        let checks = checks(&facts);
        let registered = find(&checks, "registered").unwrap();
        assert_eq!(registered.details[1], ("32 位元".to_owned(), "沒有登記".to_owned()));
        let list = &find(&checks, "user-list").unwrap().details[1].1;
        assert!(list.starts_with("微軟注音（0404:{B115") && list.ends_with("0409:00000409"), "{list}");
        assert_eq!(find(&checks, "default").unwrap().details[1].1, "清單中的第一個");
        let lines = input_lines(&facts);
        assert_eq!(lines[0], ("語言".to_owned(), "zh-Hant-TW、en-US".to_owned()));
        let install = install_lines(&WindowsFacts {
            install_files: vec![("zhuyintype.exe".into(), Some(12_000_000)), ("onnxruntime.dll".into(), None)],
            folder_access: Some(FolderAccess { app_packages: true, restricted_packages: false, users: true }),
            ..fine()
        });
        assert!(install.contains(&("可讀取資料夾".to_owned(), "使用者、所有應用程式套件".to_owned())));
        assert!(install.contains(&("onnxruntime.dll".to_owned(), "缺少".to_owned())));
    }

    #[test]
    fn folder_access_from_the_installers_descriptor() {
        // What installer.nsi's icacls leaves: SYSTEM and Administrators full, Users and both package groups read.
        let sddl = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)(A;OICI;0x1200a9;;;AC)(A;OICI;0x1200a9;;;S-1-15-2-2)";
        assert_eq!(
            FolderAccess::from_sddl(sddl),
            FolderAccess { app_packages: true, restricted_packages: true, users: true }
        );
        assert_eq!(FolderAccess::from_sddl("O:BAD:(A;OICI;FA;;;SY)(D;;FA;;;AC)"), FolderAccess::default());
    }

    #[test]
    fn smart_app_control() {
        let on = checks(&WindowsFacts { smart_app_control: SmartAppControl::On, ..fine() });
        assert_eq!(find(&on, "smart-app-control").unwrap().status, Status::Warning);
        let evaluation = checks(&WindowsFacts { smart_app_control: SmartAppControl::Evaluation, ..fine() });
        assert!(find(&evaluation, "smart-app-control").is_none());
        let blocked = checks(&WindowsFacts { blocked_files: Some(vec!["zhuyintype_tsf_x64.dll".into()]), ..fine() });
        let check = find(&blocked, "smart-app-control").unwrap();
        assert_eq!(check.status, Status::Error);
        assert!(check.detail.contains("zhuyintype_tsf_x64.dll"));
    }
}
