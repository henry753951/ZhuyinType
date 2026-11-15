//! The settings pages: the same nine pages, order, groups and titles as the macOS app
//! (`SettingsWindowController.swift`, `SettingsPane`).

use gpui_kit::assets::IconName;
use gpui_kit::{Hsla, rgb};

/// One page of the settings window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Intro,
    Analytics,
    Typing,
    Choosing,
    Words,
    Candidates,
    Keys,
    Advanced,
    About,
}

impl Page {
    /// The sidebar's sections, each under a small header.
    pub const GROUPS: &[(&str, &[Page])] = &[
        ("概覽", &[Page::Intro, Page::Analytics]),
        ("輸入", &[Page::Typing, Page::Choosing, Page::Words]),
        ("外觀", &[Page::Candidates]),
        ("一般", &[Page::Keys, Page::Advanced, Page::About]),
    ];

    pub const ALL: [Page; 9] = [
        Page::Intro,
        Page::Analytics,
        Page::Typing,
        Page::Choosing,
        Page::Words,
        Page::Candidates,
        Page::Keys,
        Page::Advanced,
        Page::About,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Intro => "介紹",
            Page::Typing => "打字",
            Page::Choosing => "選字",
            Page::Candidates => "視窗與提示",
            Page::Words => "字詞",
            Page::Analytics => "分析",
            Page::Keys => "快捷鍵",
            Page::Advanced => "進階",
            Page::About => "關於",
        }
    }

    /// The id used on the command line, in stored state and in links (`settingsLastPane` on macOS).
    pub fn id(self) -> &'static str {
        match self {
            Page::Intro => "intro",
            Page::Typing => "typing",
            Page::Choosing => "choosing",
            Page::Candidates => "candidates",
            Page::Words => "words",
            Page::Analytics => "analytics",
            Page::Keys => "keys",
            Page::Advanced => "advanced",
            Page::About => "about",
        }
    }

    pub fn from_id(id: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| page.id() == id)
    }

    /// The sidebar icon (macOS: an SF Symbol on a coloured tile; here the Lucide glyph of the same meaning).
    pub fn icon(self) -> IconName {
        match self {
            Page::Intro => IconName::Sparkles,
            Page::Typing => IconName::Keyboard,
            Page::Choosing => IconName::WandSparkles,
            Page::Candidates => IconName::PanelsTopLeft,
            Page::Words => IconName::BookOpenText,
            Page::Analytics => IconName::ChartColumn,
            Page::Keys => IconName::Command,
            Page::Advanced => IconName::SlidersHorizontal,
            Page::About => IconName::Info,
        }
    }

    /// The tile colour behind the icon (macOS's system colours).
    pub fn tint(self) -> Hsla {
        let hex = match self {
            Page::Intro => 0xff375f,
            Page::Typing => 0x5e5ce6,
            Page::Choosing => 0x30b0c7,
            Page::Candidates => 0x0a84ff,
            Page::Words => 0xff9f0a,
            Page::Analytics => 0x30d158,
            Page::Keys | Page::Advanced | Page::About => 0x8e8e93,
        };
        rgb(hex).into()
    }
}
