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

/// [`popup_position_with_gap`] in logical pixels (a gap of [`GAP`]).
#[must_use]
pub fn popup_position(anchor: Rect, width: f64, height: f64, above: bool, text_inset: f64, screen: Rect) -> (f64, f64) {
    popup_position_with_gap(anchor, width, height, above, text_inset, screen, GAP)
}
