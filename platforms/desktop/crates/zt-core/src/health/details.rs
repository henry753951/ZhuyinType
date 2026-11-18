//! 回報問題's sections beside the checks (the same titles and labels as the macOS report, `HealthReport.swift`):
//!
//! | section | what |
//! |---|---|
//! | 版本與資料 | the data folder, each data file with its size, the data versions this build was made with, the models' load |
//! | 安裝 | the program's path, owner and permissions, the settings file, the user data folder |
//! | 服務 | running, uptime, response time, the endpoint, the last key, the adapters connected |
//! | 更新 | automatic checks, the last check and its result |
//! | 改過的設定 | the settings that differ from their defaults, by key; values only for on/off, choices and numbers |
//! | 顯示與輔助功能 | display scale, 主題, 透明效果, screen reader, reduced motion, high contrast |
//!
//! The 輸入法 section is the platform's (`windows::input_lines`, `linux::input_lines`).  The builders are pure, apart
//! from reading the data files' sizes and the program's permissions, which are cheap.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::activity::Diagnostics;
use super::common::{DataFacts, ServiceFacts, SettingsFacts, UpdateFacts};
use super::report::Section;
use super::{on_off, yes_no};
use crate::settings::{Kind, descriptor};

/// The data versions this build was made with (`resources/assets.json`, the components the app ships).
const ASSETS: &str = include_str!("../../../../../../resources/assets.json");
const SHIPPED: [&str; 6] = ["lexicon", "lm", "lm_chat", "reranker", "v3", "onnx"];

/// What the app knows itself, beside [`super::Inputs`]' facts.
#[derive(Clone, Debug, Default)]
pub struct AppFacts {
    /// The serving copy's facts (this process's service, or another copy's `diagnostics` reply).
    pub diagnostics: Option<Diagnostics>,
    /// The broker's round trip, filled in by [`super::run`].
    pub response_ms: Option<f64>,
    pub models: ModelFacts,
    /// The settings file's stored values (only their keys, and closed-set values, reach the report).
    pub stored_settings: BTreeMap<String, Value>,
    /// The user data folder (personal data, statistics, logs).
    pub user_data: Option<PathBuf>,
    /// This program.
    pub executable: Option<PathBuf>,
    /// ONNX Runtime, where the app found it.
    pub onnx_runtime: Option<PathBuf>,
    pub display: DisplayFacts,
    /// Automatic update checks are on.
    pub automatic_updates: bool,
    /// 「2026-11-18 13:00」.
    pub last_update_check: Option<String>,
    /// Seconds since the Unix epoch now (the service's start time is written from it).
    pub now: u64,
}

/// The engine's and the models' load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelFacts {
    /// The lexicon and language models, in ms.
    pub engine_ms: Option<f64>,
    pub reranker_ms: Option<f64>,
    pub character_ms: Option<f64>,
    /// Why a model did not load.
    pub error: Option<String>,
}

/// The display and accessibility settings the app sees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayFacts {
    /// The settings window's scale factor (None: no window, as for `--health`).
    pub scale: Option<f32>,
    /// 「跟隨系統（深色）」.
    pub theme: Option<String>,
    /// 「開啟（Mica）」.
    pub transparency: Option<String>,
    pub screen_reader: Option<bool>,
    pub reduced_motion: Option<bool>,
    pub high_contrast: Option<bool>,
}

/// 「17.6 MB」, 「861 KB」, 「29 B」.
#[must_use]
pub fn size_text(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "a size for people to read")]
    let value = bytes as f64;
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.0} KB", value / 1e3),
        _ => format!("{:.1} MB", value / 1e6),
    }
}

/// A file's size, or a folder's (its files, not its subfolders).
fn size_of(path: &Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.is_file() {
        return Some(metadata.len());
    }
    let entries = std::fs::read_dir(path).ok()?;
    Some(entries.flatten().filter_map(|entry| entry.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum())
}

/// 「lexicon.bin，17.6 MB」 for `path` under `root`, or 「…，無法讀取」.
fn file_line(root: &Path, path: &Path) -> String {
    let mut name = path.strip_prefix(root).unwrap_or(path).display().to_string();
    if path.is_dir() {
        name.push('/');
    }
    match size_of(path) {
        Some(size) => format!("{name}，{}", size_text(size)),
        None => format!("{name}，無法讀取"),
    }
}

/// 「lexicon data-7（fb263aaa0525）、lm data-8（68d981d14b78）、…」.
#[must_use]
pub fn pinned_versions() -> String {
    let Ok(assets) = serde_json::from_str::<Value>(ASSETS) else { return "不明".to_owned() };
    SHIPPED
        .iter()
        .filter_map(|name| {
            let component = assets.get("components")?.get(name)?;
            let release = component.get("release")?.as_str()?;
            let hash: String = component.get("sha256")?.as_str()?.chars().take(12).collect();
            Some(format!("{name} {release}（{hash}）"))
        })
        .collect::<Vec<_>>()
        .join("、")
}

fn ms(value: f64) -> String {
    format!("{value:.0} ms")
}

/// 版本與資料.
#[must_use]
pub fn data_section(data: &DataFacts, about: &AppFacts) -> Section {
    let mut lines = Vec::new();
    match &data.files {
        None => lines.push(("詞庫".to_owned(), "找不到".to_owned())),
        Some(files) => {
            let root = &files.directory;
            lines.push(("資料夾".to_owned(), root.display().to_string()));
            let lexicon = if files.compiled { root.join("lexicon.bin") } else { files.lexicon.clone() };
            lines.push(("詞庫".to_owned(), file_line(root, &lexicon)));
            for (name, path) in [
                ("語言模型", &files.language_model),
                ("口語語言模型", &files.chat_model),
                ("其他讀法表", &files.reading_channel),
                ("整句重排模型", &files.reranker),
                ("逐字模型", &files.character_model),
            ] {
                let value = path.as_deref().map_or_else(|| "缺少".to_owned(), |path| file_line(root, path));
                lines.push((name.to_owned(), value));
            }
        }
    }
    lines.push(("隨附的資料版本".to_owned(), pinned_versions()));
    let models = &about.models;
    let loads: Vec<String> =
        [("詞庫與語言模型", models.engine_ms), ("整句重排模型", models.reranker_ms), ("逐字模型", models.character_ms)]
            .into_iter()
            .filter_map(|(name, ms_value)| ms_value.map(|value| format!("{name} {}", ms(value))))
            .collect();
    if !loads.is_empty() {
        lines.push(("模型載入".to_owned(), loads.join("、")));
    }
    if let Some(error) = &models.error {
        lines.push(("模型錯誤".to_owned(), error.clone()));
    }
    if let Some(error) = &data.engine_error {
        lines.push(("引擎錯誤".to_owned(), error.clone()));
    }
    let wants_runtime =
        data.files.as_ref().is_some_and(|files| files.reranker.is_some() || files.character_model.is_some());
    if wants_runtime || about.onnx_runtime.is_some() {
        let runtime =
            about.onnx_runtime.as_ref().map_or_else(|| "找不到".to_owned(), |path| path.display().to_string());
        lines.push(("ONNX Runtime".to_owned(), runtime));
    }
    Section::new("版本與資料", lines)
}

/// 安裝; `platform` are the system's own lines (Windows: the install folder's permissions and the files in it).
#[must_use]
pub fn install_section(settings: &SettingsFacts, about: &AppFacts, platform: Vec<(String, String)>) -> Section {
    let mut lines = Vec::new();
    if let Some(executable) = &about.executable {
        lines.push(("位置".to_owned(), executable.display().to_string()));
        lines.extend(permissions(executable));
    }
    lines.extend(platform);
    let settings_file =
        settings.path.as_ref().map_or_else(|| "只在記憶體中".to_owned(), |path| path.display().to_string());
    lines.push(("設定檔".to_owned(), settings_file));
    if let Some(folder) = &about.user_data {
        lines.push(("使用者資料".to_owned(), folder.display().to_string()));
    }
    Section::new("安裝", lines)
}

/// 擁有者 and 權限 of `path` (Unix; Windows writes its install folder's in its own lines).
#[cfg(unix)]
fn permissions(path: &Path) -> Vec<(String, String)> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let Ok(metadata) = std::fs::metadata(path) else { return vec![("權限".to_owned(), "無法讀取".to_owned())] };
    let owner = match metadata.uid() {
        0 => "root".to_owned(),
        uid if uid == rustix::process::getuid().as_raw() => super::report::USER.to_owned(),
        uid => format!("uid {uid}"),
    };
    vec![("擁有者".to_owned(), owner), ("權限".to_owned(), mode_text(metadata.permissions().mode()))]
}

#[cfg(not(unix))]
fn permissions(_: &Path) -> Vec<(String, String)> {
    Vec::new()
}

/// 「-rwxr-xr-x」.
#[must_use]
pub fn mode_text(mode: u32) -> String {
    let kind = if mode & 0o170_000 == 0o040_000 { 'd' } else { '-' };
    let bits = "rwxrwxrwx";
    std::iter::once(kind)
        .chain(bits.chars().enumerate().map(|(index, c)| if mode & (0o400 >> index) != 0 { c } else { '-' }))
        .collect()
}

/// 「2 小時 3 分」, 「3 天 4 小時」, 「不到 1 分」.
#[must_use]
pub fn duration_text(seconds: u64) -> String {
    let (days, hours, minutes) = (seconds / 86_400, seconds / 3_600 % 24, seconds / 60 % 60);
    match (days, hours, minutes) {
        (0, 0, 0) => "不到 1 分".to_owned(),
        (0, 0, minutes) => format!("{minutes} 分"),
        (0, hours, minutes) => format!("{hours} 小時 {minutes} 分"),
        (days, hours, _) => format!("{days} 天 {hours} 小時"),
    }
}

/// 服務.
#[must_use]
pub fn service_section(service: &ServiceFacts, endpoint: Option<&str>, about: &AppFacts) -> Section {
    let mut lines = vec![(
        "狀態".to_owned(),
        match (service.running, service.reachable) {
            (false, _) => format!("沒有執行（{}）", service.error.as_deref().unwrap_or("原因不明")),
            (true, Some(false)) => "執行中，沒有回應".to_owned(),
            (true, _) => "執行中".to_owned(),
        },
    )];
    if let Some(facts) = &about.diagnostics {
        if !facts.version.is_empty() {
            lines.push(("服務的版本".to_owned(), facts.version.clone()));
        }
        if facts.started > 0 {
            lines.push(("啟動時間".to_owned(), local_time(facts.started)));
        }
        lines.push(("已執行".to_owned(), duration_text(facts.uptime)));
        lines.push((
            "最近一次按鍵".to_owned(),
            facts.last_key.map_or_else(|| "還沒有收到按鍵".to_owned(), |ago| format!("{}前", duration_text(ago))),
        ));
        lines.push(("連線數".to_owned(), facts.connections.to_string()));
    }
    if let Some(response) = about.response_ms {
        lines.push(("回應時間".to_owned(), format!("{response:.1} ms")));
    }
    lines.push(("端點".to_owned(), endpoint.unwrap_or("沒有").to_owned()));
    Section::new("服務", lines)
}

/// 「2026-11-18 09:09」 in this computer's time zone.
fn local_time(unix: u64) -> String {
    i64::try_from(unix)
        .ok()
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map_or_else(String::new, |time| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
}

/// 更新.
#[must_use]
pub fn update_section(updates: &UpdateFacts, about: &AppFacts) -> Section {
    let result = match updates {
        UpdateFacts::Unknown => "不明".to_owned(),
        UpdateFacts::ChecksOff => "自動檢查已關閉".to_owned(),
        UpdateFacts::Current => "已是最新版本".to_owned(),
        UpdateFacts::Newer(version) => format!("有新版本 {version}"),
        UpdateFacts::Unsupported => "不是由安裝程式或套件安裝，無法自動更新".to_owned(),
        UpdateFacts::Failed(error) => format!("無法檢查：{error}"),
    };
    Section::new(
        "更新",
        vec![
            ("自動檢查".to_owned(), on_off(about.automatic_updates).to_owned()),
            ("上次檢查".to_owned(), about.last_update_check.clone().unwrap_or_else(|| "還沒有檢查過".to_owned())),
            ("結果".to_owned(), result),
        ],
    )
}

/// 改過的設定: each stored setting that differs from its default.  On/off, choices and numbers show their value;
/// anything else (free text, lists such as the 在候選字視窗顯示組字的 App, keys of other versions) only that it is set.
#[must_use]
pub fn changed_settings(stored: &BTreeMap<String, Value>) -> Vec<(String, String)> {
    const SET: &str = "（已設定）";
    stored
        .iter()
        .filter_map(|(key, value)| {
            let Some(descriptor) = descriptor(key) else { return Some((key.clone(), SET.to_owned())) };
            if *value == descriptor.default {
                return None;
            }
            let shown = match (descriptor.kind, value) {
                _ if !descriptor.accepts(value) => SET.to_owned(),
                (Kind::Toggle, Value::Bool(on)) => on_off(*on).to_owned(),
                (Kind::Number, Value::Number(number)) => number.to_string(),
                (Kind::Choice, Value::String(choice)) => choice.clone(),
                (Kind::Choice, Value::Number(number)) => number.to_string(),
                (Kind::Choice, Value::Bool(on)) => on_off(*on).to_owned(),
                _ => SET.to_owned(),
            };
            Some((key.clone(), shown))
        })
        .collect()
}

/// 顯示與輔助功能.
#[must_use]
pub fn display_section(display: &DisplayFacts) -> Section {
    let mut lines = Vec::new();
    if let Some(scale) = display.scale {
        lines.push(("螢幕縮放".to_owned(), format!("{:.0}%", scale * 100.)));
    }
    if let Some(theme) = &display.theme {
        lines.push(("主題".to_owned(), theme.clone()));
    }
    if let Some(transparency) = &display.transparency {
        lines.push(("透明效果".to_owned(), transparency.clone()));
    }
    for (label, value) in [
        ("螢幕閱讀器", display.screen_reader),
        ("減少動態效果", display.reduced_motion),
        ("高對比", display.high_contrast),
    ] {
        lines.push((label.to_owned(), value.map_or("不明", on_off).to_owned()));
    }
    Section::new("顯示與輔助功能", lines)
}

/// 「是」 when `value`, for the platform modules.
#[must_use]
pub fn flag(value: Option<bool>) -> String {
    value.map_or("不明", yes_no).to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::engine::DataFiles;

    #[test]
    fn sizes_durations_and_modes() {
        assert_eq!(size_text(29), "29 B");
        assert_eq!(size_text(861_000), "861 KB");
        assert_eq!(size_text(17_600_000), "17.6 MB");
        assert_eq!(duration_text(30), "不到 1 分");
        assert_eq!(duration_text(7_380), "2 小時 3 分");
        assert_eq!(duration_text(3 * 86_400 + 4 * 3_600 + 59), "3 天 4 小時");
        assert_eq!(mode_text(0o100_755), "-rwxr-xr-x");
        assert_eq!(mode_text(0o040_700), "drwx------");
    }

    #[test]
    fn the_pinned_versions_name_each_shipped_component() {
        let versions = pinned_versions();
        assert!(versions.starts_with("lexicon data-"), "{versions}");
        assert!(versions.contains("、v3 data-") && versions.contains("（"), "{versions}");
        assert!(!versions.contains("checkpoints") && !versions.contains("eval-sets"));
    }

    #[test]
    fn the_data_section_lists_each_file_and_what_is_missing() {
        let dir = std::env::temp_dir().join(format!("zt-health-data-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("lm")).unwrap();
        std::fs::write(dir.join("lexicon.bin"), vec![0u8; 2_000]).unwrap();
        std::fs::write(dir.join("lexicon.tsv"), b"x").unwrap();
        std::fs::write(dir.join("lm/lm.bin"), vec![0u8; 1_500_000]).unwrap();
        let files = DataFiles::locate(&dir).unwrap();
        let data = DataFacts { files: Some(files), lexicon_readable: true, engine_error: None, model_error: None };
        let about = AppFacts {
            models: ModelFacts {
                engine_ms: Some(118.4),
                error: Some("找不到 ONNX Runtime".into()),
                ..ModelFacts::default()
            },
            ..AppFacts::default()
        };
        let section = data_section(&data, &about);
        let line = |label: &str| section.lines.iter().find(|(l, _)| l == label).map(|(_, v)| v.clone());
        assert_eq!(line("詞庫").as_deref(), Some("lexicon.bin，2 KB"));
        assert_eq!(line("語言模型").as_deref(), Some("lm/，1.5 MB"));
        assert_eq!(line("整句重排模型").as_deref(), Some("缺少"));
        assert_eq!(line("模型載入").as_deref(), Some("詞庫與語言模型 118 ms"));
        assert_eq!(line("模型錯誤").as_deref(), Some("找不到 ONNX Runtime"));
        assert_eq!(line("ONNX Runtime"), None, "no model files, no runtime needed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn changed_settings_show_closed_values_and_only_keys_otherwise() {
        let stored = BTreeMap::from([
            ("spaceAction".to_owned(), json!("candidates")),
            ("escape".to_owned(), json!("composition")),
            ("candidateFontSize".to_owned(), json!(22)),
            ("popupCompositionApps".to_owned(), json!(["secret-app.exe"])),
            ("dataDirectory".to_owned(), json!("/home/amy/data")),
        ]);
        let changed = changed_settings(&stored);
        let keys: Vec<_> = changed.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["candidateFontSize", "dataDirectory", "popupCompositionApps", "spaceAction"]);
        assert!(changed.contains(&("spaceAction".into(), "candidates".into())));
        assert!(changed.contains(&("candidateFontSize".into(), "22".into())));
        let text = format!("{changed:?}");
        assert!(!text.contains("secret-app") && !text.contains("amy"), "{text}");
    }

    #[test]
    fn the_service_and_updates() {
        let about = AppFacts {
            diagnostics: Some(Diagnostics {
                version: "0.7.1".into(),
                started: 0,
                uptime: 7_380,
                last_key: Some(125),
                connections: 2,
                recent: Vec::new(),
            }),
            response_ms: Some(1.25),
            automatic_updates: true,
            last_update_check: Some("2026-11-18 13:00".into()),
            ..AppFacts::default()
        };
        let service = ServiceFacts { running: true, error: None, reachable: Some(true) };
        let lines = service_section(&service, Some("/run/user/1000/zhuyintype/ime-v1"), &about).lines;
        let text = format!("{lines:?}");
        assert!(text.contains("\"已執行\", \"2 小時 3 分\""), "{text}");
        assert!(text.contains("\"最近一次按鍵\", \"2 分前\""), "{text}");
        assert!(text.contains("\"回應時間\", \"1.2 ms\"") || text.contains("\"回應時間\", \"1.3 ms\""), "{text}");
        let down = ServiceFacts { running: false, error: Some("無法建立通訊端".into()), reachable: None };
        let lines = service_section(&down, None, &AppFacts::default()).lines;
        assert_eq!(lines[0].1, "沒有執行（無法建立通訊端）");
        let updates = update_section(&UpdateFacts::Newer("0.7.2".into()), &about).lines;
        assert_eq!(updates[2], ("結果".into(), "有新版本 0.7.2".into()));
    }
}
