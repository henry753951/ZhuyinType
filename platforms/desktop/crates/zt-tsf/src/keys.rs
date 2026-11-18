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
    if key.key == 0x20 && key.modifiers & (SHIFT | CONTROL | ALT | META) == SHIFT {
        return true; // ⇧Space switches 中文標點 between full and half width; declined, the space is typed.
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

/// A key the host listed in `claims` (IPC.md §攔截的按鍵: the keys of 開啟快速設定, 符號與 Emoji, 選取的字加入自訂詞 that
/// can run in this field and mode): claimed whatever the composition, since the local rules above would give a chord
/// such as Ctrl+Alt+Q, or any key in English mode, to the application.  Only the press; a letter matches in either
/// case, and a shifted symbol as the key it is on (the claim names Ctrl+Shift+/, the key arrives as Ctrl+Shift+?).
pub(crate) fn claimed_key(key: &Key, claims: &[(i32, i32)]) -> bool {
    let chord = |modifiers: i32| modifiers & (SHIFT | CONTROL | ALT | META);
    !key.release
        && key.key != 0
        && claims.iter().any(|&(code, modifiers)| {
            chord(modifiers) == chord(key.modifiers) && unshifted(code, modifiers) == unshifted(key.key, key.modifiers)
        })
}

/// The key a code names, the way the service's bindings store it: letters upper case, and with Shift a US-layout
/// shifted symbol as its unshifted key (`zt_core::shortcuts::Binding::normalized`).
fn unshifted(code: i32, modifiers: i32) -> i32 {
    const SHIFTED: &[u8] = b"~!@#$%^&*()_+{}|:\"<>?";
    const UNSHIFTED: &[u8] = b"`1234567890-=[]\\;',./";
    if (i32::from(b'a')..=i32::from(b'z')).contains(&code) {
        return code - 32;
    }
    if modifiers & SHIFT != 0
        && let Some(index) = u8::try_from(code).ok().and_then(|c| SHIFTED.iter().position(|&s| s == c))
    {
        return i32::from(UNSHIFTED[index]);
    }
    code
}

/// What the text service knows of the host when it decides on a key: the last reply it applied, and what it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Routing {
    /// The reply's `composing`, or a composition in the document.
    pub(crate) composing: bool,
    /// 符號與 Emoji is on screen: the window the text service draws holds the picker's grid.
    pub(crate) picker: bool,
    /// 英 (Caps Lock counted only once the host has seen it).
    pub(crate) english: bool,
    /// A passive 聯想詞 list is shown (the reply's `associated`).
    pub(crate) suggesting: bool,
}

impl Routing {
    /// The host owns every key: a composition, or the picker, whose search, arrows, Tab, ⌫, Return and Esc are all
    /// its keys in either mode.  The picker counts even if a reply said otherwise (an older host, or a reply that did
    /// not reach the document): a key given to the application while the grid shows is a key the search never gets.
    fn owns_keys(self) -> bool {
        self.composing || self.picker
    }
}

/// `OnTestKeyDown`/`OnTestKeyUp`: whether the text service takes the key from the application for the host.
pub(crate) fn claims_key(key: &Key, routing: Routing, claims: &[(i32, i32)]) -> bool {
    let owned = routing.owns_keys();
    claimed_key(key, claims)
        || (routing.suggesting && !owned && enters_suggestions(key))
        || host_wants_key(key, owned, routing.english)
}

/// A key the application gets that the host is told about (`host_observes_key`); never while the host owns keys.
pub(crate) fn observes_key(key: &Key, routing: Routing) -> bool {
    host_observes_key(key, routing.owns_keys(), routing.english, routing.suggesting)
}

/// While a passive 聯想詞 list shows (the reply's `associated`): ↓ with no modifier enters it, so the text service
/// claims it although nothing is composed.  Every other key the list leaves to the application.
pub(crate) fn enters_suggestions(key: &Key) -> bool {
    !key.release && key.key == KEY_DOWN && key.modifiers & (SHIFT | CONTROL | ALT | META) == 0
}

/// Keys the host does not take when idle but should see, the way macOS sends it every key: ⌫ after a commit
/// (送出後刪掉重打), Space and Return (they extend 前文).  The application still gets them; the text service only tells
/// the host.  While a passive 聯想詞 list shows (`suggesting`), every key pressed but a lone modifier: the host closes
/// the list, as it does when it gets the key itself.
pub(crate) fn host_observes_key(key: &Key, composing: bool, english: bool, suggesting: bool) -> bool {
    const MODIFIERS: [i32; 6] = [KEY_SHIFT, KEY_CONTROL, KEY_ALT, KEY_CAPS_LOCK, KEY_NUM_LOCK, KEY_SCROLL_LOCK];
    if suggesting && !composing && !key.release && key.key != 0 && !MODIFIERS.contains(&key.key) {
        return true;
    }
    !composing
        && !english
        && !key.release
        && key.modifiers & (CONTROL | ALT | META) == 0
        && [KEY_BACKSPACE, KEY_RETURN, KEY_ENTER, 0x20].contains(&key.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: i32, modifiers: i32) -> Key {
        Key { key, modifiers, ..Key::default() }
    }

    #[test]
    fn claimed_keys_are_taken_in_any_state() {
        let claims = [(i32::from(b'/'), CONTROL | SHIFT), ('Q' as i32, CONTROL | ALT), ('J' as i32, ALT)];
        assert!(claimed_key(&key('?' as i32, CONTROL | SHIFT), &claims), "the shifted symbol Windows reports");
        assert!(claimed_key(&key('/' as i32, CONTROL | SHIFT), &claims));
        assert!(!claimed_key(&key('/' as i32, CONTROL), &claims));
        assert!(!claimed_key(&key('?' as i32, SHIFT), &claims));
        assert!(claimed_key(&key('Q' as i32, CONTROL | ALT), &claims));
        assert!(claimed_key(&key('q' as i32, CONTROL | ALT | KEYPAD), &claims), "either case, keypad aside");
        assert!(claimed_key(&key('J' as i32, ALT), &claims));
        assert!(!claimed_key(&key(',' as i32, CONTROL), &claims));
        assert!(!claimed_key(&key('.' as i32, CONTROL | ALT), &claims));
        assert!(!claimed_key(&key('W' as i32, CONTROL | ALT), &claims));
        assert!(!claimed_key(&key('J' as i32, ALT | SHIFT), &claims));
        let mut release = key('?' as i32, CONTROL | SHIFT);
        release.release = true;
        assert!(!claimed_key(&release, &claims));
        assert!(!claimed_key(&key('Q' as i32, CONTROL | ALT), &[]));
        // Ctrl+Delete is no command's: with nothing composed it reaches the application.
        assert!(!claimed_key(&key(KEY_DELETE, CONTROL), &claims));
        assert!(!host_wants_key(&key(KEY_DELETE, CONTROL), false, false));
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
    fn shift_space_is_claimed_when_idle_in_chinese() {
        assert!(host_wants_key(&key(0x20, SHIFT), false, false));
        assert!(!host_wants_key(&key(0x20, SHIFT), false, true));
        assert!(!host_wants_key(&key(0x20, SHIFT | CONTROL), false, false));
    }

    #[test]
    fn idle_editing_keys_are_observed_not_claimed() {
        for code in [KEY_BACKSPACE, KEY_RETURN, 0x20] {
            assert!(!host_wants_key(&key(code, 0), false, false));
            assert!(host_observes_key(&key(code, 0), false, false, false));
        }
        assert!(!host_observes_key(&key(KEY_BACKSPACE, 0), true, false, false));
        assert!(!host_observes_key(&key(KEY_BACKSPACE, 0), false, true, false));
        assert!(!host_observes_key(&key(KEY_BACKSPACE, CONTROL), false, false, false));
        assert!(!host_observes_key(&key(KEY_LEFT, 0), false, false, false));
    }

    #[test]
    fn a_passive_suggestion_list_takes_only_down() {
        // ↓ enters the list; the keys the application keeps (Return, ←, Ctrl+A, Esc, Tab) are not claimed, so a Win32
        // field does not lose them, but the host hears of them and closes the list.
        assert!(enters_suggestions(&key(KEY_DOWN, 0)));
        assert!(enters_suggestions(&key(KEY_DOWN, KEYPAD)));
        assert!(!enters_suggestions(&key(KEY_DOWN, SHIFT)));
        assert!(!enters_suggestions(&Key { release: true, ..key(KEY_DOWN, 0) }));
        for (code, modifiers) in
            [(KEY_RETURN, 0), (KEY_LEFT, 0), ('A' as i32, CONTROL), (KEY_ESCAPE, 0), (KEY_TAB, 0), (KEY_DELETE, 0)]
        {
            let pressed = key(code, modifiers);
            assert!(!enters_suggestions(&pressed) && !host_wants_key(&pressed, false, false));
            assert!(host_observes_key(&pressed, false, false, true));
            assert!(!host_observes_key(&pressed, false, false, false) || code == KEY_RETURN);
        }
        // A lone modifier (the start of ⇧1 or Ctrl+C) and a release leave the list alone.
        assert!(!host_observes_key(&key(KEY_CONTROL, CONTROL), false, false, true));
        assert!(!host_observes_key(&Key { release: true, ..key(KEY_LEFT, 0) }, false, false, true));
        // Once entered the list is a composition again: every key is claimed, none observed.
        assert!(!host_observes_key(&key(KEY_LEFT, 0), true, false, true));
    }

    /// Every key the picker acts on: its search (letters, Zhuyin keys on digits and symbols, Space), 1–9, the arrows,
    /// Page Up/Down, Tab, ⌫, Return, Esc, and the releases.
    fn picker_keys() -> Vec<Key> {
        let mut keys: Vec<Key> = ['V', 'U', 'L', 'H', 'E', 'A', 'R', 'T']
            .into_iter()
            .map(|c| key(c as i32, 0))
            .chain(b"1490-;,/".iter().copied().map(|c| key(i32::from(c), 0)))
            .chain(
                [
                    0x20,
                    KEY_LEFT,
                    KEY_RIGHT,
                    KEY_UP,
                    KEY_DOWN,
                    KEY_PAGE_UP,
                    KEY_PAGE_DOWN,
                    KEY_TAB,
                    KEY_BACKSPACE,
                    KEY_RETURN,
                    KEY_ENTER,
                    KEY_ESCAPE,
                ]
                .into_iter()
                .map(|code| key(code, 0)),
            )
            .collect();
        keys.push(key(KEY_TAB, SHIFT));
        keys.push(key('V' as i32, SHIFT));
        let releases: Vec<Key> = keys.iter().map(|k| Key { release: true, ..*k }).collect();
        keys.extend(releases);
        keys
    }

    #[test]
    fn the_picker_takes_every_key_in_either_mode() {
        let claims = [(i32::from(b'/'), CONTROL | SHIFT)];
        for english in [false, true] {
            // As the host replies while the picker is open (`composing`), and with the grid on screen alone.
            for (composing, picker) in [(true, true), (true, false), (false, true)] {
                let routing = Routing { composing, picker, english, suggesting: false };
                for pressed in picker_keys() {
                    assert!(claims_key(&pressed, routing, &claims), "{pressed:?} english={english} {routing:?}");
                    assert!(!observes_key(&pressed, routing), "{pressed:?} is the picker's, not observed");
                }
                // The key that opened it closes it.
                assert!(claims_key(&key('?' as i32, CONTROL | SHIFT), routing, &claims));
            }
        }
    }

    #[test]
    fn a_picker_shown_over_a_stale_suggestion_list_still_takes_its_keys() {
        let routing = Routing { composing: false, picker: true, english: false, suggesting: true };
        for pressed in [key(KEY_ESCAPE, 0), key(KEY_RETURN, 0), key('V' as i32, 0), key(KEY_LEFT, 0)] {
            assert!(claims_key(&pressed, routing, &[]));
            assert!(!observes_key(&pressed, routing));
        }
    }

    #[test]
    fn without_the_picker_idle_keys_stay_the_applications() {
        // The rules the picker overrides still hold once it closes.
        let english = Routing { english: true, ..Routing::default() };
        let chinese = Routing::default();
        assert!(!claims_key(&key('V' as i32, 0), english, &[]));
        assert!(claims_key(&key('V' as i32, 0), chinese, &[]));
        for code in [KEY_ESCAPE, KEY_RETURN, KEY_LEFT, KEY_TAB, KEY_BACKSPACE] {
            assert!(!claims_key(&key(code, 0), chinese, &[]), "{code:#x}");
            assert!(!claims_key(&key(code, 0), english, &[]), "{code:#x}");
        }
        assert!(observes_key(&key(KEY_BACKSPACE, 0), chinese));
    }

    #[test]
    fn composing_takes_everything() {
        assert!(host_wants_key(&key(KEY_RETURN, 0), true, true));
        assert!(!host_wants_key(&key(0, 0), true, false));
    }
}
