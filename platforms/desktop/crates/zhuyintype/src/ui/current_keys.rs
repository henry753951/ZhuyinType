//! The keys the settings pages name, read from the current settings (macOS `CurrentKeys`): the rebindable commands'
//! keys, whether their feature is on, the Shift that switches 中/英.  No page spells a configurable key itself, and a
//! key whose feature is off is not shown.
//!
//! The commands' keys come from `default_bindings` until `zt_core::shortcuts` (being ported from the Qt client) can
//! say what the user bound; which feature turns a command off comes from the `commands` lists in `settings.json`.

// Shared with the 快捷鍵 and 視窗與提示 pages, which use the parts these pages do not.
#![allow(dead_code)]

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    div, px,
};
use serde_json::Value;
use zt_core::settings;

use super::h;
use crate::app::{self, Place};
use crate::pages::Page;
use crate::theme::{self, size, text};

/// A command with a rebindable key (macOS `ShortcutCommand`, most used first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    SingleCharacterList,
    CorrectionsList,
    TypeZhuyin,
    NextPage,
    PreviousPage,
    SelectLeft,
    SelectRight,
    DeleteWord,
    CommitWithNewline,
    PreviousCharacter,
    NextCharacter,
    TogglePunctuationWidth,
    InsertSpace,
}

impl Command {
    pub const ALL: [Command; 13] = [
        Command::SingleCharacterList,
        Command::CorrectionsList,
        Command::TypeZhuyin,
        Command::NextPage,
        Command::PreviousPage,
        Command::SelectLeft,
        Command::SelectRight,
        Command::DeleteWord,
        Command::CommitWithNewline,
        Command::PreviousCharacter,
        Command::NextCharacter,
        Command::TogglePunctuationWidth,
        Command::InsertSpace,
    ];

    /// The id in settings, links and `settings.json` `commands`.
    pub fn id(self) -> &'static str {
        match self {
            Command::SingleCharacterList => "singleCharacterList",
            Command::CorrectionsList => "correctionsList",
            Command::TypeZhuyin => "typeZhuyin",
            Command::NextPage => "nextPage",
            Command::PreviousPage => "previousPage",
            Command::SelectLeft => "selectLeft",
            Command::SelectRight => "selectRight",
            Command::DeleteWord => "deleteWord",
            Command::CommitWithNewline => "commitWithNewline",
            Command::PreviousCharacter => "previousCharacter",
            Command::NextCharacter => "nextCharacter",
            Command::TogglePunctuationWidth => "togglePunctuationWidth",
            Command::InsertSpace => "insertSpace",
        }
    }

    pub fn from_id(id: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|command| command.id() == id)
    }

    /// The short name on the 快捷鍵 page and in the rows of a feature with several commands.
    pub fn label(self) -> &'static str {
        match self {
            Command::SingleCharacterList => "只列一個字",
            Command::CorrectionsList => "列出修正的字",
            Command::TypeZhuyin => "打出注音本身",
            Command::NextPage => "下一頁",
            Command::PreviousPage => "上一頁",
            Command::SelectLeft => "往左選取",
            Command::SelectRight => "往右選取",
            Command::DeleteWord => "刪掉一個詞",
            Command::CommitWithNewline => "送出並換行",
            Command::PreviousCharacter => "改前一個字",
            Command::NextCharacter => "改後一個字",
            Command::TogglePunctuationWidth => "標點全半形",
            Command::InsertSpace => "插入空白",
        }
    }

    /// The anchor of the command's 「快捷鍵」 row on its feature's page.
    pub fn keys_row(self) -> String {
        format!("keys-{}", self.id())
    }

    /// The toggle setting that turns the command off: the descriptor whose `commands` lists it.
    pub fn feature_key(self) -> Option<&'static str> {
        settings::descriptors()
            .iter()
            .find(|descriptor| {
                descriptor
                    .extra
                    .get("commands")
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.iter().any(|id| id.as_str() == Some(self.id())))
            })
            .map(|descriptor| descriptor.key.as_str())
    }
}

/// The keys bound to `command`, each as its keycaps (["Alt", "⌫"]).  Defaults of the Linux and Windows client (the Qt
/// registry, `platforms/qt/src/Shortcuts.cpp`, plus the macOS commands it lacked).
// Switch to `zt_core::shortcuts` once it is ported: this is the one place the pages learn a command's keys.
fn bindings(command: Command, _cx: &App) -> Vec<Vec<&'static str>> {
    match command {
        Command::SingleCharacterList => vec![vec!["⇧", "↓"]],
        Command::CorrectionsList => vec![vec!["⇧", "↑"]],
        Command::TypeZhuyin => vec![vec!["`"]],
        Command::NextPage => vec![vec!["Tab"], vec!["PgDn"], vec!["Alt", "↓"]],
        Command::PreviousPage => vec![vec!["⇧", "Tab"], vec!["PgUp"], vec!["Alt", "↑"]],
        Command::SelectLeft => vec![vec!["⇧", "←"]],
        Command::SelectRight => vec![vec!["⇧", "→"]],
        Command::DeleteWord => vec![vec!["Alt", "⌫"]],
        Command::CommitWithNewline => vec![vec!["⇧", "Return"]],
        Command::PreviousCharacter => vec![vec!["Alt", "←"]],
        Command::NextCharacter => vec![vec!["Alt", "→"]],
        Command::TogglePunctuationWidth => vec![vec!["⇧", "空白"]],
        Command::InsertSpace => vec![vec!["Alt", "空白"]],
    }
}

/// Keycaps in one piece, the way Linux and Windows write a shortcut: "Alt+⌫", "⇧Tab", "Ctrl+⇧U".
pub fn compact(caps: &[impl AsRef<str>]) -> String {
    let Some((last, modifiers)) = caps.split_last() else { return String::new() };
    let mut text = String::new();
    for cap in modifiers {
        text.push_str(cap.as_ref());
        if cap.as_ref() != "⇧" {
            text.push('+');
        }
    }
    text.push_str(last.as_ref());
    text
}

/// What the pages need to know about keys right now.
pub struct CurrentKeys<'a> {
    cx: &'a App,
}

impl<'a> CurrentKeys<'a> {
    pub fn new(cx: &'a App) -> Self {
        Self { cx }
    }

    /// Whether the command's feature is on (a command without one always is).
    pub fn is_on(&self, command: Command) -> bool {
        command.feature_key().is_none_or(|key| app::settings(self.cx).bool(key))
    }

    /// Every key of the command, whether or not it is on.
    pub fn all(&self, command: Command) -> Vec<Vec<SharedString>> {
        bindings(command, self.cx).into_iter().map(|caps| caps.into_iter().map(SharedString::from).collect()).collect()
    }

    /// The command's first key as keycaps, whether or not it is on.
    pub fn assigned(&self, command: Command) -> Vec<SharedString> {
        self.all(command).into_iter().next().unwrap_or_default()
    }

    /// The command's first key as keycaps, or None when it is off or has no key.
    pub fn caps(&self, command: Command) -> Option<Vec<SharedString>> {
        if !self.is_on(command) {
            return None;
        }
        self.all(command).into_iter().next()
    }

    /// The command's first key in one piece ("⇧↑"), or None when it is off or has no key.
    pub fn text(&self, command: Command) -> Option<String> {
        self.caps(command).map(|caps| compact(&caps))
    }

    /// The Shift that switches 中/英, or None when Shift does not switch.
    pub fn shift_tap(&self) -> Option<&'static str> {
        let settings = app::settings(self.cx);
        if !settings.bool("shiftSwitch") {
            return None;
        }
        Some(match settings.string("shiftToggle").as_str() {
            "left" => "左 Shift",
            "right" => "右 Shift",
            _ => "Shift",
        })
    }

    /// The key that shows the 中/英 badge: the Shift tap, or Caps Lock; None when neither is on.
    pub fn mode_key(&self) -> Option<&'static str> {
        self.shift_tap()
            .or_else(|| app::settings(self.cx).bool("feature.capsLockEnglish").then_some("Caps Lock"))
    }
}

/// The keys of a command at the end of its row, as one chip per key ("⇧Return").  Until keys can be recorded here, a
/// click goes to the command's row on the 快捷鍵 page, where they are changed.
pub fn shortcut_chip(command: Command, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let keys = CurrentKeys::new(cx).all(command);
    h().id(SharedString::from(format!("chip-{}", command.id())))
        .gap(px(4.))
        .cursor_pointer()
        .on_click(move |_, _, cx| app::go(Place::row(Page::Keys, command.keys_row()), cx))
        .when(keys.is_empty(), |this| {
            this.child(div().text_size(text::SUBTITLE).text_color(p.text_tertiary).child("沒有按鍵"))
        })
        .children(keys.into_iter().map(move |caps| {
            div()
                .h(px(22.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(size::RADIUS_CONTROL)
                .bg(p.pressed)
                .border_1()
                .border_color(p.separator)
                .text_size(text::SUBTITLE)
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_secondary)
                .hover(|style| style.text_color(p.text))
                .child(compact(&caps))
        }))
}
