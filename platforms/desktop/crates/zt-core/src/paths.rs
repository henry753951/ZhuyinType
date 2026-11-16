//! Where ZhuyinType keeps its files on Linux and Windows.
//!
//! | | Linux | Windows |
//! |---|---|---|
//! | settings | `$XDG_CONFIG_HOME/zhuyintype/settings.json` | `%APPDATA%\ZhuyinType\settings.json` |
//! | personal data, logs, statistics | `$XDG_DATA_HOME/zhuyintype/` | `%LOCALAPPDATA%\ZhuyinType\` |

use std::path::PathBuf;

const NAME: &str = if cfg!(windows) { "ZhuyinType" } else { "zhuyintype" };

/// The settings file.
pub fn settings_file() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(NAME).join("settings.json"))
}

/// The directory of personal data (learned words, user phrases), statistics and the correction log.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(NAME))
}
