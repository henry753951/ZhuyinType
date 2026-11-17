//! The candidate window as the settings shape it (`zt_panel`): its style, appearance and placement, for the GPUI app
//! and for the native adapters that draw the window themselves (the Windows text service, `IPC.md` §候選字視窗).

use zt_panel::style::{Appearance, KeyLabels, Layout, Panel, PanelStyle};

use crate::presentation::Presentation;
use crate::settings::Settings;

/// The style the settings ask for now; `key` gives a command's current first key ("" when off or unbound).
pub fn style(settings: &Settings, key: impl Fn(&str) -> String) -> PanelStyle {
    PanelStyle {
        layout: if settings.string("candidateLayout") == "horizontal" { Layout::Horizontal } else { Layout::Vertical },
        font_size: settings.number("candidateFontSize").clamp(13., 36.) as f32,
        compact: settings.string("candidateDensity") == "compact",
        shows_zhuyin_hint: settings.bool("showsZhuyinHint"),
        shows_annotations: settings.bool("showsAnnotations"),
        shows_list_switches: settings.bool("showsListSwitches"),
        keys: KeyLabels {
            zhuyin: key("typeZhuyin"),
            single: key("singleCharacterList"),
            corrections: key("correctionsList"),
            zhuyin_clickable: settings.bool("feature.zhuyinRowClick"),
        },
    }
}

/// 「候選字視窗」 (its own choice, else the 主題).
pub fn appearance(settings: &Settings) -> Appearance {
    match crate::appearance::panel_theme(settings) {
        crate::appearance::Choice::Light => Appearance::Light,
        crate::appearance::Choice::Dark => Appearance::Dark,
        crate::appearance::Choice::System => Appearance::System,
    }
}

/// The whole window for `p`; `inline_preedit`: the application shows the composition itself.
pub fn panel(settings: &Settings, key: impl Fn(&str) -> String, p: &Presentation, inline_preedit: bool) -> Panel {
    Panel {
        model: p.panel_model((!inline_preedit).then_some(p.composition.as_str())),
        style: style(settings, key),
        appearance: appearance(settings),
        translucent: crate::appearance::transparency(settings),
        animates: settings.bool("panelAnimation"),
        above: settings.string("candidatePlacement") == "above",
        accent: crate::system_look::accent(settings),
    }
}
