//! Where the candidate window goes on screen (macOS `InputPanelPlacement`).

/// A rectangle (screen coordinates for the caret; logical or physical pixels, as long as all agree).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    #[must_use]
    pub fn right(&self) -> f64 {
        self.x + self.width
    }
}

/// The gap between the caret's line and the window, in logical pixels.
pub const GAP: f64 = 4.0;

/// Where the window goes: below the caret's line (or above it, `above`) with a gap of `gap`, its text lined up with
/// the composition (`text_inset`: the distance from the window's left edge to its text); on the other side when
/// `screen` (the visible area) has no room, and always inside it.
#[must_use]
pub fn popup_position_with_gap(
    anchor: Rect,
    width: f64,
    height: f64,
    above: bool,
    text_inset: f64,
    screen: Rect,
    gap: f64,
) -> (f64, f64) {
    let right = screen.right() - 1.0;
    let bottom = screen.bottom() - 1.0;
    let x = (anchor.x - text_inset).clamp(screen.x, screen.x.max(right - width + 1.0));
    let below = anchor.bottom() + gap;
    let over = anchor.y - height - gap;
    let fits_below = below + height <= bottom + 1.0;
    let fits_above = over >= screen.y;
    let y = if above {
        if fits_above || !fits_below { over } else { below }
    } else if fits_below || !fits_above {
        below
    } else {
        over
    };
    (x, y.clamp(screen.y, screen.y.max(bottom - height + 1.0)))
}

/// What a window is placed by: the caret, when it is in the application's `window`; otherwise the window's top-left
/// corner, where a field's text usually starts (the caret's x is no better than its y).  Some applications report a caret that is not where their
/// text is: the Start menu's search box reports where it was during the menu's opening animation, hundreds of pixels
/// below the screen, until its text first changes, so a window opened before then (符號與 Emoji on an empty search)
/// went to the screen's bottom corner.  A caret at the screen's origin (none reported) is outside most windows too.
#[must_use]
pub fn anchor_in_window(caret: Rect, window: Rect) -> Rect {
    let (cx, cy) = (caret.x + caret.width / 2.0, caret.y + caret.height / 2.0);
    let inside = (window.x..=window.right()).contains(&cx) && (window.y..=window.bottom()).contains(&cy);
    if inside || window.width <= 0.0 || window.height <= 0.0 { caret } else { Rect::new(window.x, window.y, 0.0, 0.0) }
}

/// [`popup_position_with_gap`] in logical pixels (a gap of [`GAP`]).
#[must_use]
pub fn popup_position(anchor: Rect, width: f64, height: f64, above: bool, text_inset: f64, screen: Rect) -> (f64, f64) {
    popup_position_with_gap(anchor, width, height, above, text_inset, screen, GAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1280×800 screen with the taskbar (the work area is 752 tall), physical pixels at 100 %.
    const SCREEN: Rect = Rect { x: 0.0, y: 0.0, width: 1280.0, height: 752.0 };
    /// The Start menu while searching, and its search box's caret.
    const START: Rect = Rect { x: 240.0, y: 36.0, width: 800.0, height: 716.0 };
    const SEARCH_CARET: Rect = Rect { x: 329.0, y: 72.0, width: 0.0, height: 18.0 };
    /// 符號與 Emoji's grid.
    const PICKER: (f64, f64) = (340.0, 320.0);

    #[test]
    fn a_caret_in_its_window_places_the_window() {
        assert_eq!(anchor_in_window(SEARCH_CARET, START), SEARCH_CARET);
        let (x, y) = popup_position(anchor_in_window(SEARCH_CARET, START), PICKER.0, PICKER.1, false, 12.0, SCREEN);
        assert_eq!((x, y), (317.0, 94.0));
    }

    #[test]
    fn a_caret_outside_its_window_is_replaced_by_the_window_top() {
        // What the Start menu's empty search reports until its text changes: below the screen.
        let stale = Rect::new(636.0, 884.0, 0.0, 18.0);
        let anchor = anchor_in_window(stale, START);
        assert_eq!(anchor, Rect::new(240.0, 36.0, 0.0, 0.0));
        let (x, y) = popup_position(anchor, PICKER.0, PICKER.1, false, 12.0, SCREEN);
        // At the Start menu's top left, not at the screen's bottom corner (y 432, where it went before).
        assert_eq!((x, y), (228.0, 40.0));
        let (_, before) = popup_position(stale, PICKER.0, PICKER.1, false, 12.0, SCREEN);
        assert_eq!(before, 752.0 - PICKER.1);
        // No caret at all (the screen's origin) is outside a window that is not there.
        assert_eq!(anchor_in_window(Rect::default(), START), Rect::new(START.x, START.y, 0.0, 0.0));
    }

    #[test]
    fn near_the_screen_bottom_the_window_goes_above_the_caret() {
        let notepad = Rect::new(130.0, 130.0, 950.0, 540.0);
        let caret = Rect::new(150.0, 640.0, 0.0, 15.0);
        let (_, y) = popup_position(anchor_in_window(caret, notepad), PICKER.0, PICKER.1, false, 12.0, SCREEN);
        assert_eq!(y, 640.0 - GAP - PICKER.1);
        // Asked for above, with no room above: below.
        let high = Rect::new(150.0, 140.0, 0.0, 15.0);
        let (_, y) = popup_position(anchor_in_window(high, notepad), PICKER.0, PICKER.1, true, 12.0, SCREEN);
        assert_eq!(y, 155.0 + GAP);
    }
}
