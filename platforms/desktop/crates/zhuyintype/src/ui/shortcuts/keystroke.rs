//! GPUI keystrokes as the core takes keys: a Qt key code with Qt modifier flags (`zt_core::keys`).  Used by the key
//! recorder and by practice fields (the 介紹 try-it field).

use gpui_kit::{Keystroke, Modifiers};
use zt_core::keys::{Key, code, modifier};

/// The Qt modifier flags of GPUI's modifiers (the Windows/Super key is Meta).
pub fn qt_modifiers(modifiers: &Modifiers) -> u32 {
    let mut flags = modifier::NONE;
    if modifiers.shift {
        flags |= modifier::SHIFT;
    }
    if modifiers.control {
        flags |= modifier::CONTROL;
    }
    if modifiers.alt {
        flags |= modifier::ALT;
    }
    if modifiers.platform {
        flags |= modifier::META;
    }
    flags
}

/// The Qt key code of a GPUI key name; `None` for keys the core never sees (function keys, media keys, modifiers).
pub fn qt_code(key: &str) -> Option<i32> {
    let code = match key {
        "space" => code::SPACE,
        "tab" => code::TAB,
        "enter" => code::RETURN,
        "escape" => code::ESCAPE,
        "backspace" => code::BACKSPACE,
        "delete" => code::DELETE,
        "insert" => code::INSERT,
        "left" => code::LEFT,
        "right" => code::RIGHT,
        "up" => code::UP,
        "down" => code::DOWN,
        "home" => code::HOME,
        "end" => code::END,
        "pageup" => code::PAGE_UP,
        "pagedown" => code::PAGE_DOWN,
        other => {
            let mut chars = other.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else { return None };
            if !c.is_ascii_graphic() {
                return None;
            }
            c.to_ascii_uppercase() as i32
        }
    };
    Some(code)
}

/// A keystroke as `(Qt code, Qt modifiers)`.
pub fn qt_key(keystroke: &Keystroke) -> Option<(i32, u32)> {
    if keystroke.is_modifier_key() {
        return None;
    }
    Some((qt_code(&keystroke.key)?, qt_modifiers(&keystroke.modifiers)))
}

/// A keystroke as the core's [`Key`] (practice fields).
pub fn core_key(keystroke: &Keystroke, repeat: bool) -> Option<Key> {
    if keystroke.is_modifier_key() {
        let code = match keystroke.key.as_str() {
            "shift" => code::SHIFT,
            "control" => code::CONTROL,
            "alt" => code::ALT,
            "platform" => code::META,
            _ => return None,
        };
        return Some(Key::new(code, qt_modifiers(&keystroke.modifiers)));
    }
    let (code, modifiers) = qt_key(keystroke)?;
    let text = keystroke.key_char.clone().unwrap_or_default();
    Some(Key { code, modifiers, text, repeat, ..Key::default() })
}
