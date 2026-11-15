//! Keys as the broker takes them: Qt key codes and modifier flags (`zt_core::keys::Key`), and which keys the host
//! wants.  Pure; the Windows translation is in `keyboard.rs`.

pub(crate) const KEY_ESCAPE: i32 = 0x0100_0000;
pub(crate) const KEY_TAB: i32 = 0x0100_0001;
pub(crate) const KEY_BACKSPACE: i32 = 0x0100_0003;
pub(crate) const KEY_RETURN: i32 = 0x0100_0004;
pub(crate) const KEY_ENTER: i32 = 0x0100_0005;
pub(crate) const KEY_INSERT: i32 = 0x0100_0006;
pub(crate) const KEY_DELETE: i32 = 0x0100_0007;
pub(crate) const KEY_HOME: i32 = 0x0100_0010;
pub(crate) const KEY_END: i32 = 0x0100_0011;
pub(crate) const KEY_LEFT: i32 = 0x0100_0012;
pub(crate) const KEY_UP: i32 = 0x0100_0013;
pub(crate) const KEY_RIGHT: i32 = 0x0100_0014;
pub(crate) const KEY_DOWN: i32 = 0x0100_0015;
pub(crate) const KEY_PAGE_UP: i32 = 0x0100_0016;
pub(crate) const KEY_PAGE_DOWN: i32 = 0x0100_0017;
pub(crate) const KEY_SHIFT: i32 = 0x0100_0020;
pub(crate) const KEY_CONTROL: i32 = 0x0100_0021;
pub(crate) const KEY_ALT: i32 = 0x0100_0023;
pub(crate) const KEY_CAPS_LOCK: i32 = 0x0100_0024;
pub(crate) const KEY_NUM_LOCK: i32 = 0x0100_0025;
pub(crate) const KEY_SCROLL_LOCK: i32 = 0x0100_0026;
pub(crate) const KEY_F1: i32 = 0x0100_0030;

pub(crate) const SHIFT: i32 = 0x0200_0000;
pub(crate) const CONTROL: i32 = 0x0400_0000;
pub(crate) const ALT: i32 = 0x0800_0000;
pub(crate) const META: i32 = 0x1000_0000;
pub(crate) const KEYPAD: i32 = 0x2000_0000;

/// One key event in the broker's terms.  `key` 0 means the key has no identity the host knows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) key: i32,
    pub(crate) modifiers: i32,
    /// 0 unknown, 1 left, 2 right (Shift only).
    pub(crate) shift_side: i32,
    pub(crate) release: bool,
    pub(crate) repeat: bool,
}

/// Whether the host would act on this key, decided locally because TSF drops a key whose test said "eaten" even when
/// OnKeyDown then declines it.  While composing the host owns every key; otherwise only Shift (中/英), and in Chinese
/// mode the keys that type zhuyin or punctuation.  Ctrl/Alt with a letter, Win, editing and navigation keys go to the
/// application.
pub(crate) fn host_wants_key(key: &Key, composing: bool, english: bool) -> bool {
    if key.key == 0 {
        return false;
    }
    if composing {
        return true;
    }
    if key.key == KEY_SHIFT {
        return true; // A tap switches 中/英.
    }
    if english || key.release {
        return false;
    }
    if key.key >= KEY_ESCAPE || key.key <= 0x20 {
        return false; // Space, Return, arrows and editing keys when idle.
    }
    let chord = key.modifiers & (CONTROL | ALT | META);
    if chord == 0 {
        return true; // Zhuyin keys, digits for tones, punctuation.
    }
    if chord & META != 0 {
        return false; // Win: the system's.
    }
    let letter = (i32::from(b'A')..=i32::from(b'Z')).contains(&key.key)
        || (i32::from(b'a')..=i32::from(b'z')).contains(&key.key);
    // Ctrl or Alt with a symbol key types 中文標點 / the key's symbol; with a letter it is the app's shortcut.
    !letter && chord != (CONTROL | ALT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: i32, modifiers: i32) -> Key {
        Key { key, modifiers, ..Key::default() }
    }

    #[test]
    fn idle_chinese_takes_typing_keys_only() {
        assert!(host_wants_key(&key('J' as i32, 0), false, false));
        assert!(host_wants_key(&key('3' as i32, 0), false, false));
        assert!(!host_wants_key(&key(0x20, 0), false, false));
        assert!(!host_wants_key(&key(KEY_RETURN, 0), false, false));
        assert!(!host_wants_key(&key(KEY_LEFT, 0), false, false));
        assert!(!host_wants_key(&key('C' as i32, CONTROL), false, false));
        assert!(host_wants_key(&key(',' as i32, CONTROL), false, false));
        assert!(!host_wants_key(&key(',' as i32, CONTROL | ALT), false, false));
        assert!(!host_wants_key(&key('J' as i32, META), false, false));
    }

    #[test]
    fn shift_always_and_english_never() {
        assert!(host_wants_key(&key(KEY_SHIFT, SHIFT), false, true));
        assert!(!host_wants_key(&key('J' as i32, 0), false, true));
        let mut release = key('J' as i32, 0);
        release.release = true;
        assert!(!host_wants_key(&release, false, false));
    }

    #[test]
    fn composing_takes_everything() {
        assert!(host_wants_key(&key(KEY_RETURN, 0), true, true));
        assert!(!host_wants_key(&key(0, 0), true, false));
    }
}
