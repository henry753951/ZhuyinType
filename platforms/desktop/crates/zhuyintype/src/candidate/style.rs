//! The settings that shape the candidate window (macOS `CandidateStyle`, `CandidateKeyLabels`), read once per update
//! from the 「視窗與提示」 settings, and the keys the window and the preview name (macOS `CurrentKeys`).

use zt_core::settings::Settings;

use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Vertical,
    Horizontal,
}

/// The key caps the window shows: each command's current first key, or "" when its feature is off or it has none.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq)]
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

impl PanelStyle {
    /// The style the settings ask for now.
    pub fn from_settings(settings: &Settings, keys: &CurrentKeys) -> Self {
        Self {
            layout: if settings.string("candidateLayout") == "horizontal" { Layout::Horizontal } else { Layout::Vertical },
            font_size: settings.number("candidateFontSize").clamp(13., 28.) as f32,
            compact: settings.string("candidateDensity") == "compact",
            shows_zhuyin_hint: settings.bool("showsZhuyinHint"),
            shows_annotations: settings.bool("showsAnnotations"),
            shows_list_switches: settings.bool("showsListSwitches"),
            keys: KeyLabels {
                zhuyin: keys.text("typeZhuyin"),
                single: keys.text("singleCharacterList"),
                corrections: keys.text("correctionsList"),
                zhuyin_clickable: settings.bool("feature.zhuyinRowClick"),
            },
        }
    }
}

/// 「外觀」: the candidate window, the phrase notice and the 中/英 badge follow the system or are forced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAppearance {
    System,
    Light,
    Dark,
}

impl PanelAppearance {
    pub fn from_settings(settings: &Settings) -> Self {
        match settings.string("panelAppearance").as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    /// The palette for a window whose system appearance is `system_dark`.
    pub fn palette(self, system_dark: bool) -> Palette {
        let dark = match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        };
        if dark { Palette::dark() } else { Palette::light() }
    }
}

/// The selection keys of a page: `selectionKeys` cut to `candidatesPerPage`.
pub fn selection_keys(settings: &Settings) -> Vec<char> {
    let per_page = settings.number("candidatesPerPage").clamp(3., 9.) as usize;
    settings.string("selectionKeys").chars().take(per_page).collect()
}

/// The keys of the commands the window and the preview name (macOS `CurrentKeys`).
///
/// With the service running they come from its shortcuts; until zt-core's shortcuts are ported (and in screenshots,
/// where there is no service) they are the defaults of Linux and Windows, minus the commands whose feature is off.
pub struct CurrentKeys {
    service: Option<zt_core::shortcuts::Shortcuts>,
    settings: Settings,
}

/// The default keys (the first one shown first), as `Shortcuts::bindings` labels them on Linux and Windows.
const DEFAULTS: &[(&str, Option<&str>, &[&str])] = &[
    ("singleCharacterList", Some("feature.singleCharacterList"), &["⇧↓"]),
    ("correctionsList", Some("feature.correctionsList"), &["⇧↑"]),
    ("typeZhuyin", Some("feature.typeZhuyin"), &["`"]),
    ("nextPage", Some("feature.pageKeys"), &["Tab", "PgDn", "Alt+↓"]),
    ("previousPage", Some("feature.pageKeys"), &["⇧Tab", "PgUp", "Alt+↑"]),
    ("selectLeft", Some("feature.selectPhrase"), &["⇧←"]),
    ("selectRight", Some("feature.selectPhrase"), &["⇧→"]),
    ("previousCharacter", None, &["Alt+←"]),
    ("nextCharacter", None, &["Alt+→"]),
];

impl CurrentKeys {
    pub fn new(settings: &Settings, service: Option<&zt_core::service::Service>) -> Self {
        Self { service: service.map(|service| service.shortcuts().clone()), settings: settings.clone() }
    }

    /// All keys of `command` now (empty when its feature is off).
    pub fn all(&self, command: &str) -> Vec<String> {
        if let Some(shortcuts) = &self.service {
            if !shortcuts.is_enabled(command) {
                return Vec::new();
            }
            return shortcuts.bindings(command).into_iter().map(|binding| binding.label).collect();
        }
        match DEFAULTS.iter().find(|(id, _, _)| *id == command) {
            Some((_, feature, keys)) if feature.is_none_or(|feature| self.settings.bool(feature)) => {
                keys.iter().map(|key| (*key).to_owned()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The keys `command` has whether or not its feature is on (what 「關閉時，… 交給 App」 hands over).
    pub fn assigned(&self, command: &str) -> Vec<String> {
        match &self.service {
            Some(shortcuts) => shortcuts.bindings(command).into_iter().map(|binding| binding.label).collect(),
            None => DEFAULTS
                .iter()
                .find(|(id, _, _)| *id == command)
                .map(|(_, _, keys)| keys.iter().map(|key| (*key).to_owned()).collect())
                .unwrap_or_default(),
        }
    }

    /// The command's first key, or "" when it has none or its feature is off.
    pub fn text(&self, command: &str) -> String {
        match &self.service {
            Some(shortcuts) => shortcuts.active_label(command),
            None => self.all(command).into_iter().next().unwrap_or_default(),
        }
    }

    /// The command's first key as key caps (「⇧↑」 → 「⇧」「↑」, 「Alt+↓」 → 「Alt」「↓」), or None.
    pub fn caps(&self, command: &str) -> Option<Vec<String>> {
        let text = self.text(command);
        (!text.is_empty()).then(|| split_caps(&text))
    }

    /// The key that switches 中/英: 「Shift」, 「左 Shift」, 「右 Shift」, 「Caps Lock」, or None.
    pub fn mode_key(&self) -> Option<String> {
        if self.settings.bool("shiftSwitch") {
            return Some(
                match self.settings.string("shiftToggle").as_str() {
                    "left" => "左 Shift",
                    "right" => "右 Shift",
                    _ => "Shift",
                }
                .to_owned(),
            );
        }
        self.settings.bool("feature.capsLockEnglish").then(|| "Caps Lock".to_owned())
    }

    /// The selection keys as the page names them: 「1–9」, 「1–5」, 「A S D F G」.
    pub fn selection(&self) -> String {
        let keys = selection_keys(&self.settings);
        if keys.iter().all(char::is_ascii_digit) {
            match (keys.first(), keys.last()) {
                (Some(first), Some(last)) => format!("{first}–{last}"),
                _ => String::new(),
            }
        } else {
            keys.iter().map(|key| key.to_ascii_uppercase().to_string()).collect::<Vec<_>>().join(" ")
        }
    }
}

/// One key label as separate caps: modifiers first (「⇧」, 「Alt」, 「Ctrl」), then the key.
pub fn split_caps(label: &str) -> Vec<String> {
    let mut caps = Vec::new();
    let mut rest = label;
    loop {
        if let Some(after) = rest.strip_prefix('⇧').filter(|after| !after.is_empty()) {
            caps.push("⇧".to_owned());
            rest = after;
        } else if let Some((modifier, after)) = rest.split_once('+').filter(|(m, a)| !m.is_empty() && !a.is_empty()) {
            caps.push(modifier.to_owned());
            rest = after;
        } else {
            caps.push(rest.to_owned());
            return caps;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_split_into_caps() {
        assert_eq!(split_caps("⇧↓"), ["⇧", "↓"]);
        assert_eq!(split_caps("Alt+↓"), ["Alt", "↓"]);
        assert_eq!(split_caps("Ctrl+Alt+K"), ["Ctrl", "Alt", "K"]);
        assert_eq!(split_caps("Tab"), ["Tab"]);
        assert_eq!(split_caps("⇧"), ["⇧"]);
    }

    #[test]
    fn keys_follow_the_feature_switches() {
        let settings = Settings::open(None);
        let keys = CurrentKeys::new(&settings, None);
        assert_eq!(keys.text("correctionsList"), "⇧↑");
        assert_eq!(keys.selection(), "1–9");
        settings.set("feature.correctionsList", false.into()).unwrap();
        settings.set("selectionKeys", "asdfghjkl".into()).unwrap();
        settings.set("candidatesPerPage", 5.into()).unwrap();
        let keys = CurrentKeys::new(&settings, None);
        assert_eq!(keys.text("correctionsList"), "");
        assert_eq!(keys.assigned("correctionsList"), ["⇧↑"]);
        assert_eq!(keys.selection(), "A S D F G");
        assert_eq!(PanelStyle::from_settings(&settings, &keys).keys.corrections, "");
    }
}
