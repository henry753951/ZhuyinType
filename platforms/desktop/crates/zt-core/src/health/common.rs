//! Checks every system shares: the engine's data files, the settings file, the input method service and the
//! version with its updates.

use std::path::{Path, PathBuf};

use super::{Check, Fix, Status};
use crate::engine::DataFiles;

/// The engine's data, as found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataFacts {
    /// The data in use (or found), with a lexicon.
    pub files: Option<DataFiles>,
    /// The lexicon could be opened and read.
    pub lexicon_readable: bool,
    /// Why the engine did not start, if it did not.
    pub engine_error: Option<String>,
}

impl DataFacts {
    /// Reads the first bytes of the lexicon of `files` (cheap: the file is memory-mapped by the engine anyway).
    #[must_use]
    pub fn probe(files: Option<DataFiles>, engine_error: Option<String>) -> Self {
        let lexicon_readable = files.as_ref().is_some_and(|files| {
            let path = if files.compiled { files.directory.join("lexicon.bin") } else { files.lexicon.clone() };
            readable(&path)
        });
        Self { files, lexicon_readable, engine_error }
    }
}

/// Whether the first bytes of `path` can be read.
#[must_use]
pub fn readable(path: &Path) -> bool {
    use std::io::Read as _;
    let mut buffer = [0u8; 64];
    std::fs::File::open(path).and_then(|mut file| file.read(&mut buffer)).is_ok_and(|read| read > 0)
}

/// The settings file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsFacts {
    /// Where it is (None: kept in memory only).
    pub path: Option<PathBuf>,
    /// It is there but could not be read: the defaults are in use.
    pub unreadable: bool,
    /// An earlier unreadable file kept beside it (`settings.unreadable.json`).
    pub set_aside: Option<PathBuf>,
}

impl SettingsFacts {
    #[must_use]
    pub fn probe(path: Option<PathBuf>, unreadable: bool) -> Self {
        let set_aside =
            path.as_ref().map(|path| path.with_extension("unreadable.json")).filter(|aside| aside.is_file());
        Self { path, unreadable, set_aside }
    }
}

/// The input method service of this session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceFacts {
    /// It runs in this process.
    pub running: bool,
    /// Why it did not start.
    pub error: Option<String>,
    /// Its endpoint answered a request (None: not tried, as when it does not run).
    pub reachable: Option<bool>,
}

/// What the app knows about updates (`zhuyintype::updates`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateFacts {
    /// Not checked yet (or the checks do not run in this copy).
    #[default]
    Unknown,
    /// Automatic checks are off.
    ChecksOff,
    Current,
    Newer(String),
    /// Not installed by the installer or a package.
    Unsupported,
    /// The last check failed.
    Failed(String),
}

/// The data files.
#[must_use]
pub fn data_check(facts: &DataFacts) -> Check {
    let Some(files) = &facts.files else {
        return Check::new("data", Status::Error, "找不到詞庫", "沒有詞庫就無法轉換注音。請重新安裝 ZhuyinType。");
    };
    if !facts.lexicon_readable {
        return Check::new(
            "data",
            Status::Error,
            "無法讀取詞庫",
            format!("{} 無法開啟。請重新安裝 ZhuyinType。", files.directory.display()),
        );
    }
    if let Some(error) = &facts.engine_error {
        return Check::new("data", Status::Error, "引擎沒有啟動", format!("{error}。請重新安裝 ZhuyinType。"));
    }
    let missing: Vec<&str> = [
        (files.language_model.is_none(), "語言模型"),
        (files.reranker.is_none(), "整句重排模型"),
        (files.character_model.is_none(), "逐字模型"),
    ]
    .into_iter()
    .filter_map(|(missing, name)| missing.then_some(name))
    .collect();
    if missing.is_empty() {
        Check::ok("data", "詞庫與模型都在", "選字用到的資料都已載入。")
    } else {
        Check::new(
            "data",
            Status::Warning,
            format!("缺少{}", missing.join("、")),
            "仍然可以打字，但選字較不準確。重新安裝 ZhuyinType 可以補回。",
        )
    }
}

/// The settings file.
#[must_use]
pub fn settings_check(facts: &SettingsFacts) -> Check {
    if facts.unreadable {
        let check = Check::new(
            "settings",
            Status::Error,
            "設定檔無法讀取",
            "目前使用預設的設定與按鍵。改任何一個設定時，原本的檔案會留成 settings.unreadable.json，不會被蓋掉。",
        );
        return match &facts.path {
            Some(path) => check.with_fix(Fix::Reveal(path.clone())),
            None => check,
        };
    }
    if let Some(aside) = &facts.set_aside {
        return Check::new(
            "settings",
            Status::Info,
            "留有一份無法讀取的設定檔",
            "先前無法讀取的設定檔留在 settings.unreadable.json；需要時可以從中找回原本的設定。",
        )
        .with_fix(Fix::Reveal(aside.clone()));
    }
    Check::ok("settings", "設定檔正常", "設定與按鍵都已讀取。")
}

/// The input method service and its endpoint.
#[must_use]
pub fn service_check(facts: &ServiceFacts) -> Check {
    if !facts.running {
        let reason = facts.error.as_deref().unwrap_or("原因不明");
        return Check::new(
            "service",
            Status::Error,
            "輸入法服務沒有啟動",
            format!("打字時不會用到 ZhuyinType：{reason}。登出再登入一次，或重新開啟 ZhuyinType。"),
        );
    }
    match facts.reachable {
        Some(false) => Check::new(
            "service",
            Status::Error,
            "輸入法服務沒有回應",
            "服務在執行，但連不上它。登出再登入一次，或重新開啟 ZhuyinType。",
        ),
        _ => Check::ok("service", "輸入法服務正在執行", "打字的 App 可以連到 ZhuyinType。"),
    }
}

/// The version and updates.
#[must_use]
pub fn update_check(version: &str, facts: &UpdateFacts) -> Check {
    let title = format!("版本 {version}");
    match facts {
        UpdateFacts::Newer(next) => Check::new(
            "update",
            Status::Warning,
            format!("有新版本 {next}"),
            format!("目前是 {version}。可以在「關於」安裝新版本。"),
        ),
        UpdateFacts::Failed(error) => Check::new("update", Status::Info, title, format!("無法檢查更新：{error}。")),
        UpdateFacts::ChecksOff => Check::new("update", Status::Info, title, "自動檢查更新已關閉。"),
        UpdateFacts::Unsupported => {
            Check::new("update", Status::Info, title, "這份 ZhuyinType 不是由安裝程式或套件安裝的，無法自動更新。")
        }
        UpdateFacts::Current => Check::ok("update", title, "已是最新版本。"),
        UpdateFacts::Unknown => Check::ok("update", title, "還沒有檢查過更新。"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &Path) -> DataFiles {
        DataFiles {
            directory: dir.to_path_buf(),
            lexicon: dir.join("lexicon.tsv"),
            compiled: true,
            language_model: Some(dir.join("lm")),
            chat_model: None,
            reading_channel: None,
            reranker: Some(dir.join("reranker")),
            character_model: Some(dir.join("v3")),
        }
    }

    #[test]
    fn data_files() {
        let dir = Path::new("/data");
        let missing = data_check(&DataFacts::default());
        assert_eq!((missing.status, missing.title.as_str()), (Status::Error, "找不到詞庫"));
        let unreadable = DataFacts { files: Some(files(dir)), lexicon_readable: false, engine_error: None };
        assert_eq!(data_check(&unreadable).title, "無法讀取詞庫");
        let fine = DataFacts { files: Some(files(dir)), lexicon_readable: true, engine_error: None };
        assert_eq!(data_check(&fine).status, Status::Ok);
        let mut partial = fine.clone();
        if let Some(files) = partial.files.as_mut() {
            files.reranker = None;
            files.character_model = None;
        }
        let check = data_check(&partial);
        assert_eq!((check.status, check.title.as_str()), (Status::Warning, "缺少整句重排模型、逐字模型"));
        let broken = DataFacts { engine_error: Some("詞庫格式不符".into()), ..fine };
        assert_eq!(data_check(&broken).status, Status::Error);
    }

    #[test]
    fn a_readable_file_has_bytes() {
        let dir = std::env::temp_dir().join(format!("zt-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (full, empty) = (dir.join("full"), dir.join("empty"));
        std::fs::write(&full, b"x").unwrap();
        std::fs::write(&empty, b"").unwrap();
        assert!(readable(&full));
        assert!(!readable(&empty));
        assert!(!readable(&dir.join("absent")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_settings_file_and_the_one_set_aside() {
        let path = PathBuf::from("/c/settings.json");
        let fine = SettingsFacts { path: Some(path.clone()), unreadable: false, set_aside: None };
        assert_eq!(settings_check(&fine).status, Status::Ok);
        let broken = SettingsFacts { unreadable: true, ..fine.clone() };
        let check = settings_check(&broken);
        assert_eq!(check.status, Status::Error);
        assert_eq!(check.fix, Some(Fix::Reveal(path.clone())));
        let aside = PathBuf::from("/c/settings.unreadable.json");
        let kept = SettingsFacts { set_aside: Some(aside.clone()), ..fine };
        let check = settings_check(&kept);
        assert_eq!((check.status, check.fix), (Status::Info, Some(Fix::Reveal(aside))));
    }

    #[test]
    fn the_set_aside_file_is_found_beside_the_settings() {
        let dir = std::env::temp_dir().join(format!("zt-health-aside-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert_eq!(SettingsFacts::probe(Some(path.clone()), false).set_aside, None);
        std::fs::write(dir.join("settings.unreadable.json"), b"{").unwrap();
        assert_eq!(SettingsFacts::probe(Some(path), false).set_aside, Some(dir.join("settings.unreadable.json")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_service() {
        let down = ServiceFacts { running: false, error: Some("無法建立通訊端".into()), reachable: None };
        let check = service_check(&down);
        assert_eq!(check.status, Status::Error);
        assert!(check.detail.contains("無法建立通訊端"));
        let silent = ServiceFacts { running: true, error: None, reachable: Some(false) };
        assert_eq!(service_check(&silent).title, "輸入法服務沒有回應");
        let fine = ServiceFacts { running: true, error: None, reachable: Some(true) };
        assert_eq!(service_check(&fine).status, Status::Ok);
    }

    #[test]
    fn updates() {
        assert_eq!(update_check("0.6.1", &UpdateFacts::Newer("0.6.2".into())).status, Status::Warning);
        assert_eq!(update_check("0.6.1", &UpdateFacts::Current).title, "版本 0.6.1");
        assert_eq!(update_check("0.6.1", &UpdateFacts::Failed("逾時".into())).status, Status::Info);
        assert_eq!(update_check("0.6.1", &UpdateFacts::Unknown).status, Status::Ok);
    }
}
