//! The settings that shape the candidate window (macOS `CandidateStyle`, `CandidateKeyLabels`), read once per update
//! from the 「視窗與提示」 settings, and the keys the window and the preview name (macOS `CurrentKeys`).

use zt_core::settings::Settings;

use crate::theme::Palette;

pub use zt_panel::style::{KeyLabels, Layout, PanelStyle};

/// The style the settings ask for now (`zt_core::panel::style` with these keys).
pub fn style_from_settings(settings: &Settings, keys: &CurrentKeys) -> PanelStyle {
    zt_core::panel::style(settings, |command| keys.text(command))
}

/// 「候選字視窗」: the candidate window, the phrase notice and the 中/英 badge follow the 主題 (and so perhaps the
/// system) or are forced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAppearance {
    System,
    Light,
    Dark,
}

impl PanelAppearance {
    pub fn from_settings(settings: &Settings) -> Self {
        crate::theme::candidate::set_accent(zt_core::system_look::accent(settings));
        match zt_core::appearance::panel_theme(settings) {
            zt_core::appearance::Choice::Light => Self::Light,
            zt_core::appearance::Choice::Dark => Self::Dark,
            zt_core::appearance::Choice::System => Self::System,
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
/// They come from the service's shortcuts; without a service (screenshots, before the engine loads) from the same
/// rules on the settings file (`zt_core::shortcuts`), minus the commands whose feature is off.
pub struct CurrentKeys {
    shortcuts: zt_core::shortcuts::Shortcuts,
    settings: Settings,
}

impl CurrentKeys {
    pub fn new(settings: &Settings, service: Option<&zt_core::service::Service>) -> Self {
        let shortcuts = match service {
            Some(service) => service.shortcuts().clone(),
            None => zt_core::shortcuts::Shortcuts::new(settings.clone()),
        };
        Self { shortcuts, settings: settings.clone() }
    }

    /// The keys stored in `settings` itself (the setup wizard's draft, which has its own copy of the keys).
    pub fn sandboxed(settings: &Settings) -> Self {
        Self::new(settings, None)
    }

    /// All keys of `command` now (empty when its feature is off).
    pub fn all(&self, command: &str) -> Vec<String> {
        if !self.shortcuts.is_enabled(command) {
            return Vec::new();
        }
        self.shortcuts.bindings(command).into_iter().map(|binding| binding.label).collect()
    }

    /// The keys `command` has whether or not its feature is on (what 「關閉時，… 交給 App」 hands over).
    #[cfg(test)]
    pub fn assigned(&self, command: &str) -> Vec<String> {
        self.shortcuts.bindings(command).into_iter().map(|binding| binding.label).collect()
    }

    /// The command's first key, or "" when it has none or its feature is off.
    pub fn text(&self, command: &str) -> String {
        self.shortcuts.active_label(command)
    }

    /// The command's first key for running text (「Shift+↑」), or "".
    pub fn spoken(&self, command: &str) -> String {
        crate::ui::keycap::prose(&self.text(command))
    }

    /// The command's first key as key caps (「⇧↑」 → 「⇧」「↑」, 「Ctrl+↓」 → 「Ctrl」「↓」), or None.
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
        assert_eq!(super::style_from_settings(&settings, &keys).keys.corrections, "");
    }
}
