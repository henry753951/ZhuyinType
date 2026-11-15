//! The rebindable commands of the 快捷鍵 page (macOS `Shared/Shortcuts.swift` and `Preferences.bindings`, Qt
//! `Shortcuts`): each command has up to five keys; only the commands the user changed are stored (under the
//! `shortcuts` key of the settings file), the others follow their defaults minus any default the user gave to another
//! command.  The feature switches (`feature.*`) come from the same settings.
//!
//! Platform words: ⌃ is Ctrl, ⌥ is Alt, ⌘ is Win (Windows) or Super (Linux) and stays reserved; fn ↑ ↓ ← → ⌫ are PgUp,
//! PgDn, Home, End and Delete.  Ctrl with a letter is the apps' (Ctrl+C, Ctrl+V …).

use std::collections::BTreeMap;

use anyhow::Result;
use zhuyin_core::api::KeyInput;

use crate::settings::Settings;

/// One key with modifiers (Qt key code and modifier flags, normalised: no keypad flag, letters upper case, shifted
/// symbols as their unshifted key with Shift).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Binding {
    pub key: i32,
    pub modifiers: u32,
}

/// A binding with the text the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingLabel {
    pub binding: Binding,
    /// One chip: 「⇧↓」, 「Alt+↓」, 「PgDn」.
    pub label: String,
    /// Read aloud and in tooltips: 「Shift+Tab」, 「Page Down」.
    pub spoken: String,
}

/// What a command does when its key is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The composer receives this input.
    Input(KeyInput),
    /// 改前一個字／改後一個字: the list moves to the neighbouring character (-1 or 1).
    Neighbour(i32),
    /// 標點全半形.
    TogglePunctuationWidth,
}

/// One rebindable command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// `singleCharacterList`, `nextPage`, …
    pub id: &'static str,
    /// The short name on the 快捷鍵 page and in conflict messages.
    pub label: &'static str,
    /// The row's longer title.
    pub title: &'static str,
    /// The settings page whose 快捷鍵 row shows it.
    pub page: &'static str,
    /// The feature switch that turns it off, if any.
    pub feature: Option<&'static str>,
    /// Keeps at least one key.
    pub required: bool,
    /// The default keys.
    pub defaults: Vec<Binding>,
    pub action: Action,
}

impl Command {
    /// The row id links scroll to (`keys-<id>`).
    #[must_use]
    pub fn row(&self) -> String {
        format!("keys-{}", self.id)
    }
}

/// Two commands with the same key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub command: &'static str,
    pub other: &'static str,
    /// The shared key's chip.
    pub label: String,
}

/// The most keys a command has.
pub const MAX_BINDINGS: usize = 5;

/// The keys of every command.  Cheap to clone; clones share the settings store.
#[derive(Clone)]
pub struct Shortcuts {
    settings: Settings,
}

impl Shortcuts {
    /// The keys stored in `settings` (unreadable stored keys: the defaults, and `error` says so; the stored value
    /// is kept until the user changes a key).
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Every command, most used first (macOS `ShortcutCommand.allCases`).
    #[must_use]
    pub fn commands() -> &'static [Command] {
        todo!()
    }

    /// The command `id`.
    #[must_use]
    pub fn command(_id: &str) -> Option<&'static Command> {
        todo!()
    }

    /// The keys of `command` now.
    #[must_use]
    pub fn bindings(&self, _command: &str) -> Vec<BindingLabel> {
        todo!()
    }

    /// Every command's keys now.
    #[must_use]
    pub fn all(&self) -> BTreeMap<&'static str, Vec<BindingLabel>> {
        todo!()
    }

    /// The command a key runs, if any (an enabled command wins over one whose feature is off).
    #[must_use]
    pub fn resolve(&self, _key: i32, _modifiers: u32) -> Option<&'static Command> {
        todo!()
    }

    /// Sets binding `index` of `command` (`None` or past the end adds one).
    ///
    /// # Errors
    /// Why not, in the page's words (reserved key, used by another command, too many keys, cannot save).
    pub fn record(&self, _command: &str, _key: i32, _modifiers: u32, _index: Option<usize>) -> Result<()> {
        todo!()
    }

    /// Removes one key, default or not; a required command keeps its last one.
    ///
    /// # Errors
    /// When there is no such key, the command is required and this is its last key, or saving fails.
    pub fn remove(&self, _command: &str, _index: usize) -> Result<()> {
        todo!()
    }

    /// Restores the command's defaults that no other command uses now; with `None`, every command's (全部還原).
    ///
    /// # Errors
    /// When every default is taken (naming the command that has it), or saving fails.
    pub fn reset(&self, _command: Option<&str>) -> Result<()> {
        todo!()
    }

    /// The user changed this command's keys.
    #[must_use]
    pub fn is_customised(&self, _command: &str) -> bool {
        todo!()
    }

    /// The user changed any command's keys.
    #[must_use]
    pub fn any_customised(&self) -> bool {
        todo!()
    }

    /// The command can be used (its feature is on, or it has none).
    #[must_use]
    pub fn is_enabled(&self, _command: &str) -> bool {
        todo!()
    }

    /// Bindings of `command` that another enabled command also has: binding index → the other command.
    #[must_use]
    pub fn conflicts_of(&self, _command: &str) -> BTreeMap<usize, &'static str> {
        todo!()
    }

    /// Every conflicting pair once, for the 按鍵衝突 section.
    #[must_use]
    pub fn conflicts(&self) -> Vec<Conflict> {
        todo!()
    }

    /// The command's first key as a chip (「⇧↓」), or "" when it has none or its feature is off.
    #[must_use]
    pub fn active_label(&self, _command: &str) -> String {
        todo!()
    }

    /// The modifiers held so far, as the recorder shows them (「Ctrl+Alt+」).
    #[must_use]
    pub fn modifier_label(_modifiers: u32) -> String {
        todo!()
    }

    /// Why the stored keys could not be read, if they could not.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        todo!()
    }
}
