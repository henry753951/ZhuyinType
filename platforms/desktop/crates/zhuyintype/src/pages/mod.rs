//! The settings pages: the same nine pages, order, groups and titles as the macOS app
//! (`SettingsWindowController.swift`, `SettingsPane`).
//!
//! Each page is a module with a view (`XPage`, a GPUI entity created once and kept while the window lives) and
//! `subpage_title`.  A page reads `app::nav(cx).subpage` to know which of its sub-pages to draw; it goes elsewhere
//! with `app::go(Place, cx)`.  Pages are composed only from `ui` components and `theme` tokens; the shell scrolls them
//! and draws the sub-page bar and the push/pop motion.

pub mod about;
pub mod advanced;
pub mod analytics;
pub mod candidates;
pub mod choosing;
pub mod intro;
pub mod keys;
pub mod typing;
pub mod words;

use gpui_kit::assets::IconName;
use gpui_kit::{AnyView, AppContext as _, Window};

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
}

/// The view of `page`, created when the page is first shown.
pub fn make_view(page: Page, window: &mut Window, cx: &mut gpui_kit::App) -> AnyView {
    match page {
        Page::Intro => cx.new(|cx| intro::IntroPage::new(window, cx)).into(),
        Page::Analytics => cx.new(|cx| analytics::AnalyticsPage::new(window, cx)).into(),
        Page::Typing => cx.new(|cx| typing::TypingPage::new(window, cx)).into(),
        Page::Choosing => cx.new(|cx| choosing::ChoosingPage::new(window, cx)).into(),
        Page::Words => cx.new(|cx| words::WordsPage::new(window, cx)).into(),
        Page::Candidates => cx.new(|cx| candidates::CandidatesPage::new(window, cx)).into(),
        Page::Keys => cx.new(|cx| keys::KeysPage::new(window, cx)).into(),
        Page::Advanced => cx.new(|cx| advanced::AdvancedPage::new(window, cx)).into(),
        Page::About => cx.new(|cx| about::AboutPage::new(window, cx)).into(),
    }
}

/// The title of `page`'s sub-page `id`.
pub fn subpage_title(page: Page, id: &str) -> Option<&'static str> {
    match page {
        Page::Intro => intro::subpage_title(id),
        Page::Analytics => analytics::subpage_title(id),
        Page::Typing => typing::subpage_title(id),
        Page::Choosing => choosing::subpage_title(id),
        Page::Words => words::subpage_title(id),
        Page::Candidates => candidates::subpage_title(id),
        Page::Keys => keys::subpage_title(id),
        Page::Advanced => advanced::subpage_title(id),
        Page::About => about::subpage_title(id),
    }
}

/// Whether the shell draws the page's large title above it (介紹 opens with its own header instead).
pub fn shows_title(page: Page, subpage: Option<&str>) -> bool {
    !(page == Page::Intro && subpage.is_none())
}
