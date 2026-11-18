//! The settings pages: the same fourteen pages, order, groups and titles as the macOS app (SETTINGS_IA.md §2;
//! `SettingsWindowController.swift`, `SettingsPane`).
//!
//! Each page is a module with a view (`XPage`, a GPUI entity created once and kept while the window lives) and
//! `subpage_title`.  A page reads `app::nav(cx).subpage` to know which of its sub-pages to draw; it goes elsewhere
//! with `app::go(Place, cx)`, and every place a link leads to is listed in `links`.  Pages are composed only from `ui`
//! components and `theme` tokens; the shell scrolls them and draws the sub-page bar and the push/pop motion.

pub mod about;
pub mod advanced;
pub mod analytics;
pub mod candidates;
pub mod choosing;
pub mod colors;
pub mod corrections;
pub mod health;
pub mod intro;
pub mod keys;
pub mod links;
pub mod mode;
pub mod punctuation;
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
    Punctuation,
    Mode,
    Choosing,
    Corrections,
    Words,
    Candidates,
    Colors,
    Keys,
    Health,
    Advanced,
    About,
}

impl Page {
    /// The sidebar's sections, each under a small header.
    pub const GROUPS: &[(&str, &[Page])] = &[
        ("概覽", &[Page::Intro, Page::Analytics]),
        ("輸入", &[Page::Typing, Page::Punctuation, Page::Mode]),
        ("選字與字詞", &[Page::Choosing, Page::Corrections, Page::Words]),
        ("外觀", &[Page::Candidates, Page::Colors]),
        ("一般", &[Page::Keys, Page::Health, Page::Advanced, Page::About]),
    ];

    pub const ALL: [Page; 14] = [
        Page::Intro,
        Page::Analytics,
        Page::Typing,
        Page::Punctuation,
        Page::Mode,
        Page::Choosing,
        Page::Corrections,
        Page::Words,
        Page::Candidates,
        Page::Colors,
        Page::Keys,
        Page::Health,
        Page::Advanced,
        Page::About,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Intro => "介紹",
            Page::Analytics => "分析",
            Page::Typing => "打字",
            Page::Punctuation => "標點與符號",
            Page::Mode => "中英切換",
            Page::Choosing => "選字",
            Page::Corrections => "修正",
            Page::Words => "字詞",
            Page::Candidates => "候選字視窗",
            Page::Colors => "色彩",
            Page::Keys => "快捷鍵",
            Page::Health => "檢查輸入法",
            Page::Advanced => "進階",
            Page::About => "關於",
        }
    }

    /// The id used on the command line, in stored state and in links (`settingsLastPane` on macOS).
    pub fn id(self) -> &'static str {
        match self {
            Page::Intro => "intro",
            Page::Analytics => "analytics",
            Page::Typing => "typing",
            Page::Punctuation => "punctuation",
            Page::Mode => "mode",
            Page::Choosing => "choosing",
            Page::Corrections => "corrections",
            Page::Words => "words",
            Page::Candidates => "candidates",
            Page::Colors => "colors",
            Page::Keys => "keys",
            Page::Health => "health",
            Page::Advanced => "advanced",
            Page::About => "about",
        }
    }

    /// The page with exactly this id (no old ids).
    pub fn from_current_id(id: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| page.id() == id)
    }

    /// The sidebar icon (macOS: an SF Symbol on a coloured tile; here the Lucide glyph of the same meaning).
    pub fn icon(self) -> IconName {
        match self {
            Page::Intro => IconName::House,
            Page::Analytics => IconName::ChartColumn,
            Page::Typing => IconName::Type,
            Page::Punctuation => IconName::Quote,
            Page::Mode => IconName::Languages,
            Page::Choosing => IconName::ListOrdered,
            Page::Corrections => IconName::WandSparkles,
            Page::Words => IconName::BookOpenText,
            Page::Candidates => IconName::PanelsTopLeft,
            Page::Colors => IconName::Palette,
            Page::Keys => IconName::Keyboard,
            Page::Health => IconName::Stethoscope,
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
        Page::Punctuation => cx.new(|cx| punctuation::PunctuationPage::new(window, cx)).into(),
        Page::Mode => cx.new(|cx| mode::ModePage::new(window, cx)).into(),
        Page::Choosing => cx.new(|cx| choosing::ChoosingPage::new(window, cx)).into(),
        Page::Corrections => cx.new(|cx| corrections::CorrectionsPage::new(window, cx)).into(),
        Page::Words => cx.new(|cx| words::WordsPage::new(window, cx)).into(),
        Page::Candidates => cx.new(|cx| candidates::CandidatesPage::new(window, cx)).into(),
        Page::Colors => cx.new(|cx| colors::ColorsPage::new(window, cx)).into(),
        Page::Keys => cx.new(|cx| keys::KeysPage::new(window, cx)).into(),
        Page::Health => cx.new(|cx| health::HealthPage::new(window, cx)).into(),
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
        Page::Corrections => corrections::subpage_title(id),
        Page::Words => words::subpage_title(id),
        Page::Punctuation | Page::Mode | Page::Candidates | Page::Colors | Page::Keys | Page::Health => None,
        Page::Advanced => advanced::subpage_title(id),
        Page::About => about::subpage_title(id),
    }
}

/// Whether the shell draws the page's large title above it (介紹 opens with its own header instead).
pub fn shows_title(page: Page, subpage: Option<&str>) -> bool {
    !(page == Page::Intro && subpage.is_none())
}
