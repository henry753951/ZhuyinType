//! Keys as the core receives them: a Qt key code with Qt modifier flags (the values the broker protocol carries,
//! `IPC.md`), the text the key types, whether it repeats, and which Shift it is.  The native adapters send these
//! over the broker; the GPUI app converts its keystrokes to them.
//!
//! On Linux and Windows Control is `CONTROL` and the Windows/Super key is `META` (Qt's macOS swap does not apply).

use zhuyin_core::api::KeyInput;

/// Qt key codes (`Qt::Key`) used by the core.  Printable ASCII keys are their upper-case character (`'A'`, `'1'`).
pub mod code {
    pub const SPACE: i32 = 0x20;
    pub const QUOTE_LEFT: i32 = 0x60;
    pub const ESCAPE: i32 = 0x0100_0000;
    pub const TAB: i32 = 0x0100_0001;
    pub const BACKTAB: i32 = 0x0100_0002;
    pub const BACKSPACE: i32 = 0x0100_0003;
    pub const RETURN: i32 = 0x0100_0004;
    pub const ENTER: i32 = 0x0100_0005;
    pub const INSERT: i32 = 0x0100_0006;
    pub const DELETE: i32 = 0x0100_0007;
    pub const HOME: i32 = 0x0100_0010;
    pub const END: i32 = 0x0100_0011;
    pub const LEFT: i32 = 0x0100_0012;
    pub const UP: i32 = 0x0100_0013;
    pub const RIGHT: i32 = 0x0100_0014;
    pub const DOWN: i32 = 0x0100_0015;
    pub const PAGE_UP: i32 = 0x0100_0016;
    pub const PAGE_DOWN: i32 = 0x0100_0017;
    pub const SHIFT: i32 = 0x0100_0020;
    pub const CONTROL: i32 = 0x0100_0021;
    pub const META: i32 = 0x0100_0022;
    pub const ALT: i32 = 0x0100_0023;
    pub const CAPS_LOCK: i32 = 0x0100_0024;
    pub const NUM_LOCK: i32 = 0x0100_0025;
    pub const F1: i32 = 0x0100_0030;
    pub const SUPER_L: i32 = 0x0100_0053;
    pub const SUPER_R: i32 = 0x0100_0054;
    pub const ALT_GR: i32 = 0x0100_1103;
    pub const UNKNOWN: i32 = 0x01ff_ffff;
}

/// Qt modifier flags (`Qt::KeyboardModifier`).
pub mod modifier {
    pub const NONE: u32 = 0;
    pub const SHIFT: u32 = 0x0200_0000;
    pub const CONTROL: u32 = 0x0400_0000;
    pub const ALT: u32 = 0x0800_0000;
    /// The Windows key or Super.
    pub const META: u32 = 0x1000_0000;
    pub const KEYPAD: u32 = 0x2000_0000;
    pub const GROUP_SWITCH: u32 = 0x4000_0000;
}

/// Which Shift key went down or up (`shiftSide` in the protocol).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShiftSide {
    /// The platform did not say.
    #[default]
    Unknown,
    Left,
    Right,
}

impl ShiftSide {
    /// The protocol value: 0 unknown, 1 left, 2 right.
    #[must_use]
    pub fn from_wire(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Unknown),
            1 => Some(Self::Left),
            2 => Some(Self::Right),
            _ => None,
        }
    }

    /// The protocol value.
    #[must_use]
    pub fn wire(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Left => 1,
            Self::Right => 2,
        }
    }
}

/// One key press or release.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Key {
    /// Qt key code (`code::*`, or the upper-case ASCII character).
    pub code: i32,
    /// Qt modifier flags (`modifier::*`).
    pub modifiers: u32,
    /// The text the key types, when the platform reports it (informational; the core reads the code).
    pub text: String,
    /// An auto-repeat.
    pub repeat: bool,
    /// For `code::SHIFT`, which Shift.
    pub shift_side: ShiftSide,
}

/// The unshifted key of each shifted symbol on the US layout.
const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
const UNSHIFTED: &str = "`1234567890-=[]\\;',./";

impl Key {
    /// A key with modifiers.
    #[must_use]
    pub fn new(code: i32, modifiers: u32) -> Self {
        Self { code, modifiers, ..Self::default() }
    }

    /// The key that types `character` on the US layout: letters as their upper-case code (Shift for capitals),
    /// shifted symbols as Qt reports them (`'!'` with Shift).  `None` outside printable ASCII.
    #[must_use]
    pub fn typing(character: char) -> Option<Self> {
        let text = character.to_string();
        match character {
            ' ' => Some(Self { text, ..Self::new(code::SPACE, 0) }),
            'a'..='z' => Some(Self { text, ..Self::new(character.to_ascii_uppercase() as i32, 0) }),
            'A'..='Z' => Some(Self { text, ..Self::new(character as i32, modifier::SHIFT) }),
            c if SHIFTED.contains(c) => Some(Self { text, ..Self::new(c as i32, modifier::SHIFT) }),
            c if c.is_ascii_graphic() => Some(Self { text, ..Self::new(c as i32, 0) }),
            _ => None,
        }
    }

    /// A Shift key of `side`.
    #[must_use]
    pub fn shift(side: ShiftSide) -> Self {
        Self { shift_side: side, ..Self::new(code::SHIFT, modifier::SHIFT) }
    }

    /// What the composer receives for this key, if anything.
    #[must_use]
    pub fn input(&self) -> Option<KeyInput> {
        input(self.code, self.modifiers)
    }

    /// The key with Backtab as Shift+Tab, Enter as Return and the keypad flag dropped (how bindings compare keys).
    #[must_use]
    pub fn canonical(&self) -> (i32, u32) {
        canonical(self.code, self.modifiers)
    }

    /// Shift, Control, Alt, AltGr, Meta or Super alone.
    #[must_use]
    pub fn is_modifier(&self) -> bool {
        matches!(
            self.code,
            code::SHIFT | code::CONTROL | code::META | code::ALT | code::ALT_GR | code::SUPER_L | code::SUPER_R
        )
    }
}

/// `(code, modifiers)` with Backtab as Shift+Tab, Enter as Return and the keypad flag dropped.
#[must_use]
pub fn canonical(code: i32, modifiers: u32) -> (i32, u32) {
    let mut modifiers = modifiers & !modifier::KEYPAD;
    let mut code = code;
    if code == code::BACKTAB {
        code = code::TAB;
        modifiers |= modifier::SHIFT;
    }
    if code == code::ENTER {
        code = code::RETURN;
    }
    (code, modifiers)
}

/// What the composer receives for a Qt key (macOS `KeyMapping`, Qt `KeyInput::fromQt`): `None` for keys it never
/// handles (function keys, Meta/Super chords, unknown combinations).
#[must_use]
pub fn input(code: i32, modifiers: u32) -> Option<KeyInput> {
    if modifiers & modifier::META != 0 {
        return None;
    }
    let mods = modifiers & !modifier::KEYPAD;
    let shift = mods == modifier::SHIFT;
    let alt = mods == modifier::ALT;
    let control = mods == modifier::CONTROL;
    if mods != 0 && !shift && !alt && !control {
        return None;
    }
    let plain = mods == 0;
    Some(match code {
        code::SPACE if plain || shift => {
            if shift {
                KeyInput::ShiftSpace
            } else {
                KeyInput::Space
            }
        }
        code::RETURN | code::ENTER if plain || shift => {
            if shift {
                KeyInput::ShiftEnter
            } else {
                KeyInput::Enter
            }
        }
        code::BACKSPACE if plain || alt => {
            if alt {
                KeyInput::OptionBackspace
            } else {
                KeyInput::Backspace
            }
        }
        code::DELETE if plain => KeyInput::ForwardDelete,
        code::ESCAPE if plain => KeyInput::Escape,
        code::TAB | code::BACKTAB if plain || shift => {
            if shift || code == code::BACKTAB {
                KeyInput::ShiftTab
            } else {
                KeyInput::Tab
            }
        }
        code::LEFT if plain || shift => {
            if shift {
                KeyInput::ShiftLeft
            } else {
                KeyInput::Left
            }
        }
        code::RIGHT if plain || shift => {
            if shift {
                KeyInput::ShiftRight
            } else {
                KeyInput::Right
            }
        }
        code::UP if alt => KeyInput::PageUp,
        code::UP if plain || shift => {
            if shift {
                KeyInput::ShiftUp
            } else {
                KeyInput::Up
            }
        }
        code::DOWN if alt => KeyInput::PageDown,
        code::DOWN if plain || shift => {
            if shift {
                KeyInput::ShiftDown
            } else {
                KeyInput::Down
            }
        }
        code::HOME if plain => KeyInput::Home,
        code::END if plain => KeyInput::End,
        code::PAGE_UP if plain => KeyInput::PageUp,
        code::PAGE_DOWN if plain => KeyInput::PageDown,
        0x21..=0x7e => {
            let mut c = char::from_u32(code as u32)?.to_ascii_lowercase();
            if shift && let Some(index) = SHIFTED.find(c) {
                c = UNSHIFTED.as_bytes()[index] as char;
            }
            if (alt || control) && c.is_ascii_lowercase() {
                return None;
            }
            if shift {
                KeyInput::ShiftedChar(c)
            } else if alt {
                KeyInput::Printed(c)
            } else if control {
                KeyInput::ControlChar(c)
            } else {
                KeyInput::Char(c)
            }
        }
        _ => return None,
    })
}

/// The key in zhuyin-cli batch syntax (`s`, `<S-1>`, `<enter>`); `None` for the few keys it cannot write (`<`, `>`
/// alone, and `>` with a modifier).
#[must_use]
pub fn label(input: KeyInput) -> Option<String> {
    let text = |s: &str| Some(s.to_owned());
    match input {
        KeyInput::Char(' ') => text("<space>"),
        KeyInput::Char('<' | '>') => None,
        KeyInput::Char(c) => Some(c.to_string()),
        KeyInput::ShiftedChar('>') | KeyInput::Printed('>') | KeyInput::ControlChar('>') => None,
        KeyInput::ShiftedChar(c) => Some(format!("<S-{c}>")),
        KeyInput::Printed(c) => Some(format!("<O-{c}>")),
        KeyInput::ControlChar(c) => Some(format!("<C-{c}>")),
        KeyInput::Space => text("<space>"),
        KeyInput::ShiftSpace => text("<sspace>"),
        KeyInput::TogglePunctuationWidth => text("<punct-width>"),
        KeyInput::Enter => text("<enter>"),
        KeyInput::ShiftEnter => text("<senter>"),
        KeyInput::Backspace => text("<bs>"),
        KeyInput::OptionBackspace => text("<obs>"),
        KeyInput::ForwardDelete => text("<del>"),
        KeyInput::Escape => text("<esc>"),
        KeyInput::Tab => text("<tab>"),
        KeyInput::ShiftTab => text("<stab>"),
        KeyInput::Left => text("<left>"),
        KeyInput::Right => text("<right>"),
        KeyInput::Up => text("<up>"),
        KeyInput::Down => text("<down>"),
        KeyInput::Home => text("<home>"),
        KeyInput::End => text("<end>"),
        KeyInput::PageUp => text("<pgup>"),
        KeyInput::PageDown => text("<pgdown>"),
        KeyInput::ShiftLeft => text("<sleft>"),
        KeyInput::ShiftRight => text("<sright>"),
        KeyInput::ShiftDown => text("<sdown>"),
        KeyInput::ShiftUp => text("<sup>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_like_the_qt_client() {
        assert_eq!(input('S' as i32, 0), Some(KeyInput::Char('s')));
        assert_eq!(input('!' as i32, modifier::SHIFT), Some(KeyInput::ShiftedChar('1')));
        assert_eq!(input('<' as i32, modifier::SHIFT), Some(KeyInput::ShiftedChar(',')));
        assert_eq!(input(',' as i32, modifier::CONTROL), Some(KeyInput::ControlChar(',')));
        assert_eq!(input('A' as i32, modifier::CONTROL), None);
        assert_eq!(input('A' as i32, modifier::META), None);
        assert_eq!(input('A' as i32, modifier::CONTROL | modifier::ALT), None);
        assert_eq!(input(code::BACKTAB, 0), Some(KeyInput::ShiftTab));
        assert_eq!(input(code::DOWN, modifier::ALT), Some(KeyInput::PageDown));
        assert_eq!(input(code::ENTER, modifier::KEYPAD), Some(KeyInput::Enter));
        assert_eq!(input(code::F1, 0), None);
        assert_eq!(canonical(code::ENTER, modifier::SHIFT | modifier::KEYPAD), (code::RETURN, modifier::SHIFT));
        assert_eq!(Key::typing('a').unwrap().input(), Some(KeyInput::Char('a')));
        assert_eq!(Key::typing('?').unwrap().input(), Some(KeyInput::ShiftedChar('/')));
        assert_eq!(label(KeyInput::ShiftedChar('1')).as_deref(), Some("<S-1>"));
        assert_eq!(label(KeyInput::Char('>')), None);
    }
}
