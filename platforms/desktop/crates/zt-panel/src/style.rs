//! The settings that shape the candidate window (macOS `CandidateStyle`, `CandidateKeyLabels`), and the window's
//! appearance and placement as the broker sends them to the Windows text service.

use serde::{Deserialize, Serialize};

use crate::a11y::AccentChoice;
use crate::model::PanelModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Vertical,
    Horizontal,
}

/// The key caps the window shows: each command's current first key, or "" when its feature is off or it has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyLabels {
    /// Types the highlighted span's Zhuyin (the reading row's key cap).
    pub zhuyin: String,
    pub single: String,
    pub corrections: String,
    /// Clicking the reading row types the Zhuyin itself.
    pub zhuyin_clickable: bool,
}

impl Default for KeyLabels {
    fn default() -> Self {
        Self { zhuyin: "`".into(), single: "⇧↓".into(), corrections: "⇧↑".into(), zhuyin_clickable: true }
    }
}

/// Everything on the 「視窗與提示」 page that changes how the window looks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelStyle {
    pub layout: Layout,
    pub font_size: f32,
    pub compact: bool,
    pub shows_zhuyin_hint: bool,
    pub shows_annotations: bool,
    pub shows_list_switches: bool,
    pub keys: KeyLabels,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            layout: Layout::Vertical,
            font_size: 18.,
            compact: false,
            shows_zhuyin_hint: true,
            shows_annotations: true,
            shows_list_switches: true,
            keys: KeyLabels::default(),
        }
    }
}

/// 「外觀」: the window follows the system's light or dark, or is forced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// Dark, for a system whose own appearance is `system_dark`.
    #[must_use]
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

fn translucent_by_default() -> bool {
    true
}

/// The window as the broker sends it to a native adapter that draws it (`IPC.md` §候選字視窗).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub model: PanelModel,
    pub style: PanelStyle,
    pub appearance: Appearance,
    /// 「透明效果」: the system's backdrop shows through the window where it has one (Windows 11's acrylic).
    #[serde(default = "translucent_by_default")]
    pub translucent: bool,
    /// The 動畫 setting: page turns slide.
    pub animates: bool,
    /// 「位置」: above the caret's line rather than below.
    pub above: bool,
    /// 「強調色」: the selection follows the system's accent colour, or keeps ZhuyinType's own look.
    #[serde(default)]
    pub accent: AccentChoice,
    /// 符號與 Emoji is open: its grid, which the adapter draws instead of `model` (the highlighted row as a list, for
    /// adapters that do not know the grid).  Left out otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picker: Option<crate::picker::PickerView>,
}
