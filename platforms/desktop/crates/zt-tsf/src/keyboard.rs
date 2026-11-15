//! Windows virtual keys to the broker's keys (`keys.rs`), from the key event and the keyboard state.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, MAPVK_VSC_TO_VK_EX, MapVirtualKeyW, ToUnicodeEx, VIRTUAL_KEY,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F24, VK_HOME,
    VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT, VK_NUMLOCK, VK_NUMPAD0, VK_PACKET,
    VK_PRIOR, VK_PROCESSKEY, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT,
    VK_TAB, VK_UP,
};

use crate::keys::*;

/// `ToUnicodeEx` flag: leave the keyboard state (dead keys) unchanged.
const NO_STATE_CHANGE: u32 = 4;

fn code(vk: WPARAM) -> u16 {
    u16::try_from(vk.0).unwrap_or(0)
}

fn down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: no preconditions.
    (unsafe { GetKeyState(i32::from(key.0)) } as u16 & 0x8000) != 0
}

pub(crate) fn caps_lock_on() -> bool {
    // SAFETY: no preconditions.
    (unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) } & 1) != 0
}

fn scan_code(flags: LPARAM) -> u32 {
    ((flags.0 as u32) >> 16) & 0xff
}

/// Keys that may reach the host at all: not Win, not keys another IME already processed, not injected text.
pub(crate) fn potential_key(vk: WPARAM) -> bool {
    let vk = code(vk);
    ![VK_LWIN.0, VK_RWIN.0, VK_PROCESSKEY.0, VK_PACKET.0].contains(&vk)
}

/// The one printable character the key types with `state`, if any.
fn character(vk: u16, flags: LPARAM, state: &[u8; 256]) -> Option<u16> {
    let mut chars = [0u16; 8];
    // SAFETY: valid buffers; the layout of this thread.
    let count = unsafe {
        ToUnicodeEx(u32::from(vk), scan_code(flags), state, &mut chars, NO_STATE_CHANGE, Some(GetKeyboardLayout(0)))
    };
    (count == 1 && chars[0] >= 32 && !(0xd800..=0xdfff).contains(&chars[0])).then_some(chars[0])
}

/// The character the key types in the current keyboard state, for a key the host declined; none with Ctrl or Alt.
pub(crate) fn typed_character(vk: WPARAM, flags: LPARAM) -> Option<u16> {
    let mut state = [0u8; 256];
    // SAFETY: a 256-byte buffer.
    unsafe { GetKeyboardState(&mut state) }.ok()?;
    if (state[usize::from(VK_CONTROL.0)] | state[usize::from(VK_MENU.0)]) & 0x80 != 0 {
        return None;
    }
    character(code(vk), flags, &state)
}

pub(crate) fn translate_key(vk: WPARAM, flags: LPARAM, release: bool) -> Key {
    let vk = code(vk);
    let mut key = Key { release, repeat: !release && (flags.0 & (1 << 30)) != 0, ..Key::default() };
    if down(VK_SHIFT) {
        key.modifiers |= SHIFT;
    }
    if down(VK_CONTROL) {
        key.modifiers |= CONTROL;
    }
    if down(VK_MENU) {
        key.modifiers |= ALT;
    }
    if down(VK_LWIN) || down(VK_RWIN) {
        key.modifiers |= META;
    }
    if (VK_NUMPAD0.0..=VK_DIVIDE.0).contains(&vk) {
        key.modifiers |= KEYPAD;
    }
    key.key = match vk {
        _ if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 => {
            let physical = if vk == VK_SHIFT.0 {
                // SAFETY: no preconditions.
                u16::try_from(unsafe { MapVirtualKeyW(scan_code(flags), MAPVK_VSC_TO_VK_EX) }).unwrap_or(0)
            } else {
                vk
            };
            key.shift_side = if physical == VK_LSHIFT.0 {
                1
            } else if physical == VK_RSHIFT.0 {
                2
            } else {
                0
            };
            KEY_SHIFT
        }
        _ if (u16::from(b'A')..=u16::from(b'Z')).contains(&vk) => i32::from(vk),
        _ if (VK_F1.0..=VK_F24.0).contains(&vk) => KEY_F1 + i32::from(vk - VK_F1.0),
        _ if vk == VK_ESCAPE.0 => KEY_ESCAPE,
        _ if vk == VK_TAB.0 => KEY_TAB,
        _ if vk == VK_BACK.0 => KEY_BACKSPACE,
        _ if vk == VK_RETURN.0 => {
            if flags.0 & (1 << 24) != 0 {
                KEY_ENTER
            } else {
                KEY_RETURN
            }
        }
        _ if vk == VK_INSERT.0 => KEY_INSERT,
        _ if vk == VK_DELETE.0 => KEY_DELETE,
        _ if vk == VK_HOME.0 => KEY_HOME,
        _ if vk == VK_END.0 => KEY_END,
        _ if vk == VK_LEFT.0 => KEY_LEFT,
        _ if vk == VK_UP.0 => KEY_UP,
        _ if vk == VK_RIGHT.0 => KEY_RIGHT,
        _ if vk == VK_DOWN.0 => KEY_DOWN,
        _ if vk == VK_PRIOR.0 => KEY_PAGE_UP,
        _ if vk == VK_NEXT.0 => KEY_PAGE_DOWN,
        _ if [VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0].contains(&vk) => KEY_CONTROL,
        _ if [VK_MENU.0, VK_LMENU.0, VK_RMENU.0].contains(&vk) => KEY_ALT,
        _ if vk == VK_CAPITAL.0 => KEY_CAPS_LOCK,
        _ if vk == VK_NUMLOCK.0 => KEY_NUM_LOCK,
        _ if vk == VK_SCROLL.0 => KEY_SCROLL_LOCK,
        _ => {
            let mut state = [0u8; 256];
            // SAFETY: a 256-byte buffer.
            if unsafe { GetKeyboardState(&mut state) }.is_err() {
                0
            } else {
                // Ctrl and Alt travel in the modifiers, not in the key's identity.
                for modifier in [VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU, VK_RMENU] {
                    state[usize::from(modifier.0)] = 0;
                }
                character(vk, flags, &state).map_or(0, i32::from)
            }
        }
    };
    key
}
