//! 快速設定 on Windows: the menu of the 中/英 button (right click) and of the `quickSettings` shortcut (at the caret),
//! and the glyph and tooltip the button shows.  Pure, so it is tested on any system; text_service.rs shows the menu
//! with `TrackPopupMenuEx` and sends the IPC `quick` op (platforms/desktop/IPC.md).

/// The four switches as the host reports them (`quick` in a reply; `english`, `simplified` and `learningPaused` are
/// in every reply).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct QuickState {
    pub(crate) english: bool,
    pub(crate) full_width: bool,
    pub(crate) simplified: bool,
    pub(crate) learning_paused: bool,
    /// A 符號 picker exists: the menu then has 「符號…」.
    pub(crate) symbols: bool,
}

/// One item of the menu; its id is the menu command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    English = 1,
    HalfWidth,
    Simplified,
    PauseLearning,
    Settings,
    Words,
    Symbols,
    Wizard,
    Tour,
    Feedback,
    About,
}

const COMMANDS: [Command; 11] = [
    Command::English,
    Command::HalfWidth,
    Command::Simplified,
    Command::PauseLearning,
    Command::Settings,
    Command::Words,
    Command::Symbols,
    Command::Wizard,
    Command::Tour,
    Command::Feedback,
    Command::About,
];

/// What picking a command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// The `quick` op with this one field.
    Quick(&'static str, bool),
    /// The `show-settings` op, with this place or none.
    Settings(Option<&'static str>),
}

impl Command {
    pub(crate) fn id(self) -> u32 {
        self as u32
    }

    pub(crate) fn from_id(id: u32) -> Option<Self> {
        COMMANDS.into_iter().find(|command| command.id() == id)
    }

    /// Check items flip their switch from `state`.
    pub(crate) fn effect(self, state: &QuickState) -> Effect {
        match self {
            Self::English => Effect::Quick("english", !state.english),
            // Checked = half width, so picking it asks for the other width.
            Self::HalfWidth => Effect::Quick("fullWidth", !state.full_width),
            Self::Simplified => Effect::Quick("simplified", !state.simplified),
            Self::PauseLearning => Effect::Quick("learningPaused", !state.learning_paused),
            Self::Settings => Effect::Settings(None),
            Self::Words => Effect::Settings(Some("words")),
            Self::Symbols => Effect::Settings(Some("symbols")),
            Self::Wizard => Effect::Settings(Some("wizard")),
            Self::Tour => Effect::Settings(Some("tour")),
            Self::Feedback => Effect::Settings(Some("feedback")),
            Self::About => Effect::Settings(Some("about")),
        }
    }
}

/// A menu line: a command, or a separator (`None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) command: Option<Command>,
    pub(crate) label: &'static str,
    pub(crate) checked: bool,
    pub(crate) enabled: bool,
}

impl Item {
    fn command(command: Command, label: &'static str) -> Self {
        Self { command: Some(command), label, checked: false, enabled: true }
    }

    fn check(command: Command, label: &'static str, checked: bool, enabled: bool) -> Self {
        Self { command: Some(command), label, checked, enabled }
    }

    fn separator() -> Self {
        Self { command: None, label: "", checked: false, enabled: true }
    }
}

/// The menu (contract: four check items, a separator, the settings places).  `switches`: a field that takes Chinese
/// has the focus, so the `quick` op can reach the host; without one the check items are grayed.  The taskbar's menu
/// (`taskbar`) also has the input method's other places, as before 快速設定.
pub(crate) fn items(state: &QuickState, switches: bool, taskbar: bool) -> Vec<Item> {
    let mut items = vec![
        Item::check(Command::English, "英文", state.english, switches),
        Item::check(Command::HalfWidth, "半形標點", !state.full_width, switches),
        Item::check(Command::Simplified, "簡體", state.simplified, switches),
        Item::check(Command::PauseLearning, "暫停學習", state.learning_paused, switches),
        Item::separator(),
        Item::command(Command::Settings, if taskbar { "ZhuyinType 設定…" } else { "設定…" }),
        Item::command(Command::Words, "自訂詞與學到的詞…"),
    ];
    if state.symbols {
        items.push(Item::command(Command::Symbols, "符號…"));
    }
    if taskbar {
        items.extend([
            Item::separator(),
            Item::command(Command::Wizard, "設定嚮導…"),
            Item::command(Command::Tour, "新功能導覽…"),
            Item::separator(),
            Item::command(Command::Feedback, "回報問題…"),
            Item::command(Command::About, "關於 ZhuyinType"),
        ]);
    }
    items
}

/// The button's character: 英 where typing gives English (`english_shown`), else 中, or 简 with 簡體 output.
pub(crate) fn glyph(english_shown: bool, simplified: bool) -> &'static str {
    match (english_shown, simplified) {
        (true, _) => "英",
        (false, true) => "简",
        (false, false) => "中",
    }
}

/// The button's tooltip.  `available`: a field that takes Chinese has the focus.
pub(crate) fn tooltip(available: bool, english: bool, simplified: bool, paused: bool) -> String {
    let mode = if !available || english {
        "英文"
    } else if simplified {
        "簡體中文"
    } else {
        "中文"
    };
    let hint = match (available, english) {
        (false, _) => None,
        (true, true) => Some("按一下切換成中文"),
        (true, false) => Some("按一下切換成英文"),
    };
    match (paused, hint) {
        (false, None) => mode.to_owned(),
        (false, Some(hint)) => format!("{mode}（{hint}）"),
        (true, None) => format!("{mode}（暫停學習）"),
        (true, Some(hint)) => format!("{mode}（暫停學習，{hint}）"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[Item]) -> Vec<&'static str> {
        items.iter().map(|item| if item.command.is_some() { item.label } else { "-" }).collect()
    }

    #[test]
    fn quick_menu_has_the_contract_items() {
        // As a session starts: Chinese, full-width punctuation, traditional, learning.
        let state = QuickState { full_width: true, ..Default::default() };
        let items = items(&state, true, false);
        assert_eq!(labels(&items), ["英文", "半形標點", "簡體", "暫停學習", "-", "設定…", "自訂詞與學到的詞…"]);
        let checked: Vec<bool> = items.iter().take(4).map(|item| item.checked).collect();
        assert_eq!(checked, [false, false, false, false]);
    }

    #[test]
    fn checks_follow_the_state() {
        let state =
            QuickState { english: true, full_width: false, simplified: true, learning_paused: true, symbols: true };
        let items = items(&state, true, false);
        let checked: Vec<bool> = items.iter().take(4).map(|item| item.checked).collect();
        assert_eq!(checked, [true, true, true, true]);
        assert_eq!(items.last().map(|item| item.label), Some("符號…"));
    }

    #[test]
    fn taskbar_menu_keeps_the_other_places() {
        let items = items(&QuickState::default(), false, true);
        assert_eq!(
            labels(&items),
            [
                "英文",
                "半形標點",
                "簡體",
                "暫停學習",
                "-",
                "ZhuyinType 設定…",
                "自訂詞與學到的詞…",
                "-",
                "設定嚮導…",
                "新功能導覽…",
                "-",
                "回報問題…",
                "關於 ZhuyinType"
            ]
        );
        assert!(items.iter().take(4).all(|item| !item.enabled));
        assert!(items.iter().skip(4).all(|item| item.enabled));
    }

    #[test]
    fn commands_round_trip_and_flip_their_switch() {
        for command in COMMANDS {
            assert_eq!(Command::from_id(command.id()), Some(command));
        }
        assert_eq!(Command::from_id(0), None);
        assert_eq!(Command::from_id(12), None);
        let state = QuickState {
            english: false,
            full_width: true,
            simplified: false,
            learning_paused: true,
            ..Default::default()
        };
        assert_eq!(Command::English.effect(&state), Effect::Quick("english", true));
        assert_eq!(Command::HalfWidth.effect(&state), Effect::Quick("fullWidth", false));
        assert_eq!(Command::Simplified.effect(&state), Effect::Quick("simplified", true));
        assert_eq!(Command::PauseLearning.effect(&state), Effect::Quick("learningPaused", false));
        assert_eq!(Command::Settings.effect(&state), Effect::Settings(None));
        assert_eq!(Command::Words.effect(&state), Effect::Settings(Some("words")));
        assert_eq!(Command::Symbols.effect(&state), Effect::Settings(Some("symbols")));
    }

    #[test]
    fn badge_glyph_and_tooltip() {
        assert_eq!(glyph(false, false), "中");
        assert_eq!(glyph(false, true), "简");
        assert_eq!(glyph(true, true), "英");
        assert_eq!(tooltip(true, false, false, false), "中文（按一下切換成英文）");
        assert_eq!(tooltip(true, false, true, false), "簡體中文（按一下切換成英文）");
        assert_eq!(tooltip(true, true, true, false), "英文（按一下切換成中文）");
        assert_eq!(tooltip(false, false, true, false), "英文");
        assert_eq!(tooltip(true, false, false, true), "中文（暫停學習，按一下切換成英文）");
        assert_eq!(tooltip(false, false, false, true), "英文（暫停學習）");
    }
}
