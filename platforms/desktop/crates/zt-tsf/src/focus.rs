//! Gives the keyboard focus back after a menu of the text service's own.
//!
//! `TrackPopupMenu` needs its owner in the foreground (or the menu does not close when the user clicks elsewhere),
//! and the owner is the text service's hidden window.  Once the menu closed that hidden window kept the activation
//! and the keyboard focus, so the next keys went to it instead of the field the user was typing in.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetActiveWindow, GetFocus, SetActiveWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetForegroundWindow, IsWindow, SetForegroundWindow,
};

/// This thread's active window and focus before a menu.
pub(crate) struct Remembered {
    active: HWND,
    focus: HWND,
}

impl Remembered {
    pub(crate) fn now() -> Self {
        // SAFETY: no preconditions.
        let (active, focus) = unsafe { (GetActiveWindow(), GetFocus()) };
        let active = if active.is_invalid() && !focus.is_invalid() {
            // SAFETY: any window handle.
            unsafe { GetAncestor(focus, GA_ROOT) }
        } else {
            active
        };
        Self { active, focus }
    }

    /// Gives the activation and focus back if the menu's `owner` still has them (a command that opened another
    /// window keeps that window in front).
    pub(crate) fn restore(&self, owner: HWND) {
        if owner.is_invalid() || self.active.is_invalid() || self.active == owner {
            return;
        }
        // SAFETY: window handles are checked with IsWindow; these calls only change activation and focus.
        unsafe {
            if !IsWindow(Some(self.active)).as_bool() {
                return;
            }
            if GetForegroundWindow() == owner {
                let _ = SetForegroundWindow(self.active);
            } else if GetActiveWindow() == owner {
                let _ = SetActiveWindow(self.active);
            } else {
                return;
            }
            if !self.focus.is_invalid() && self.focus != owner && IsWindow(Some(self.focus)).as_bool() {
                let _ = SetFocus(Some(self.focus));
            }
        }
    }
}
