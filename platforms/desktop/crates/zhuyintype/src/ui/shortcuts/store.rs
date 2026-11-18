//! The keys of the rebindable commands, for every view that shows or changes them (the 快捷鍵 page, the 快捷鍵 rows of
//! the feature pages, the 介紹 demo).
//!
//! There is one table of commands and one set of rules, `zt_core::shortcuts`: with the service running the views use
//! its [`Shortcuts`]; without it (the engine not loaded yet) a [`Shortcuts`] on the same settings file, so the page
//! stays usable and both agree on the file.

use std::collections::BTreeMap;

use anyhow::Result;
use gpui_kit::App;
use zt_core::shortcuts::{Binding, BindingLabel, Command, Conflict, Shortcuts};

use crate::app;

/// Every command, in the order of macOS `ShortcutCommand.allCases` (most used first).
#[cfg(test)]
pub fn commands() -> &'static [Command] {
    Shortcuts::commands()
}

/// The command `id`.
pub fn command(id: &str) -> Option<&'static Command> {
    Shortcuts::command(id)
}

/// The keycaps of a binding: modifiers in the order Ctrl Alt ⇧ Win/Super, then the key.
pub fn keycaps(binding: Binding) -> Vec<String> {
    binding.keycaps()
}

/// The modifiers held so far, as the recorder shows them (「Ctrl+Alt+」, 「⇧」).
pub fn modifier_label(modifiers: u32) -> String {
    Shortcuts::modifier_label(modifiers)
}

/// The keys of every command, from the service or, until it runs, from the settings file.
pub struct KeyStore(Shortcuts);

/// The store the views use now.
pub fn store(cx: &App) -> KeyStore {
    KeyStore(match app::service(cx) {
        Some(service) => service.shortcuts().clone(),
        None => Shortcuts::new(app::settings(cx).clone()),
    })
}

impl KeyStore {
    /// The keys of `command` now.
    pub fn bindings(&self, command: &str) -> Vec<BindingLabel> {
        self.0.bindings(command)
    }

    /// Sets binding `index` of `command` (past the end adds one).
    ///
    /// # Errors
    /// Why not, in the page's words.
    pub fn record(&self, command: &str, key: i32, modifiers: u32, index: usize) -> Result<()> {
        self.0.record(command, key, modifiers, Some(index))
    }

    /// Removes one key; a required command keeps its last one.
    ///
    /// # Errors
    /// Why not, in the page's words.
    pub fn remove(&self, command: &str, index: usize) -> Result<()> {
        self.0.remove(command, index)
    }

    /// The command's defaults back (those no other command uses); `None`: every command's.
    ///
    /// # Errors
    /// When every default is taken (naming who has it), or saving fails.
    pub fn reset(&self, command: Option<&str>) -> Result<()> {
        self.0.reset(command)
    }

    pub fn is_customised(&self, command: &str) -> bool {
        self.0.is_customised(command)
    }

    pub fn any_customised(&self) -> bool {
        self.0.any_customised()
    }

    /// Bindings of `command` another enabled command also has: index → the other command.
    pub fn conflicts_of(&self, command: &str) -> BTreeMap<usize, &'static str> {
        self.0.conflicts_of(command)
    }

    /// Every conflicting pair once.
    pub fn conflicts(&self) -> Vec<Conflict> {
        self.0.conflicts()
    }

    /// The command's first key as a chip, or "" when it has none or its feature is off.
    pub fn active_label(&self, command: &str) -> String {
        self.0.active_label(command)
    }

    /// Why the stored keys could not be read or saved, if so (not a refused change: the field shows that one where
    /// it happened).
    pub fn error(&self) -> Option<String> {
        self.0.storage_error()
    }
}

/// The short name of `command` (「列出修正的字」).
pub fn command_label(id: &str) -> &'static str {
    command(id).map_or("", |command| command.label)
}

/// The command's default keys, read aloud and joined by 「或」.
pub fn defaults_spoken(id: &str) -> String {
    command(id)
        .map(|command| command.defaults.iter().map(|b| b.spoken()).collect::<Vec<_>>().join(" 或 "))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use zt_core::keys::{code, modifier};
    use zt_core::settings::Settings;

    use super::*;

    fn b(key: i32, modifiers: u32) -> Binding {
        Binding { key, modifiers }
    }

    #[test]
    fn labels_use_the_platform_words() {
        assert_eq!(b(code::DOWN, modifier::CONTROL).label(), "Ctrl+↓");
        assert_eq!(b(code::TAB, modifier::SHIFT).label(), "⇧Tab");
        assert_eq!(b(code::RETURN, modifier::CONTROL).label(), "Ctrl+Enter");
        assert_eq!(keycaps(b(i32::from(b'U'), modifier::CONTROL | modifier::SHIFT)), ["Ctrl", "⇧", "U"]);
        assert_eq!(b(code::PAGE_DOWN, 0).spoken(), "Page Down");
        assert_eq!(modifier_label(modifier::CONTROL | modifier::ALT), "Ctrl+Alt+");
        assert_eq!(defaults_spoken("nextPage"), "Tab 或 Page Down 或 Ctrl+↓");
    }

    #[test]
    fn without_the_service_the_rules_are_the_same() {
        let store = KeyStore(Shortcuts::new(Settings::open(None)));
        assert_eq!(store.bindings("nextPage").len(), 3);
        assert!(store.record("singleCharacterList", code::SPACE, 0, 0).is_err());
        let error = store.record("singleCharacterList", code::UP, modifier::SHIFT, 0).unwrap_err().to_string();
        assert_eq!(error, "已用於「列出修正的字」");
        store.record("singleCharacterList", i32::from(b'J'), modifier::CONTROL | modifier::ALT, 1).unwrap();
        assert_eq!(store.bindings("singleCharacterList").len(), 2);
        assert!(store.is_customised("singleCharacterList"));
        store.reset(Some("singleCharacterList")).unwrap();
        assert!(!store.any_customised());
        assert!(store.remove("commitWithNewline", 0).is_err());
    }
}
