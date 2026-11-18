//! What one GPUI scroll event asks of a vertical scroll view: a wheel notch to animate, or a precise movement to
//! follow 1:1, in pixels the way Chrome and Edge on the same system measure them.
//!
//! GPUI reports a mouse wheel as lines (`ScrollDelta::Lines`):
//!
//! - Windows: `WM_MOUSEWHEEL` → notches (`delta / WHEEL_DELTA`) × 「一次捲動的行數」 (`SPI_GETWHEELSCROLLLINES`).  Precision
//!   touchpads go through DirectManipulation and arrive as pixels.  Chromium turns a line into 100/3 px
//!   (`kScrollbarPixelsPerLine`, `ui/events/blink/web_input_event_builders_win.cc`), so a notch at the default three
//!   lines is 100 px; 「一次捲動一個畫面」 (`WHEEL_PAGESCROLL`) scrolls a page.
//! - Linux: three lines a notch (`SCROLL_LINES`), from X11 button 4/5, XInput 2 scroll valuators (a touchpad's are
//!   fractions), Wayland `axis_value120`/`axis_discrete`; Wayland touchpads arrive as pixels.  Chromium scrolls
//!   120 px a notch on Linux (`MouseWheelEvent::kWheelDelta`, `ui/events/event.cc`), 40 px a line.
//!
//! A delta of less than one notch comes from a high-resolution wheel or a touchpad that reports through the wheel
//! (X11's valuators, touchpad drivers without precision support on Windows): it already arrives in small steps, so it
//! is followed 1:1 like pixels instead of eased.  Chromium eases those on Windows too, because it cannot tell them
//! apart (crbug.com/545234); following them keeps a touchpad from feeling late.

use gpui_kit::ScrollDelta;

/// One event's movement along the vertical axis, in pixels; positive scrolls down (the scroll position grows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Whole wheel notches: animated.
    Notches(f64),
    /// Touchpad, high-resolution wheel: followed as it comes.
    Precise(f64),
}

/// How the platform measures a wheel notch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelUnits {
    /// Lines GPUI reports for one notch; `None` when a notch scrolls a page.
    pub lines_per_notch: Option<f32>,
    /// Pixels a line scrolls.
    pub px_per_line: f32,
}

/// Chromium's `kMinFractionToStepWhenPaging` (Windows and Linux overlap a page by its remaining eighth).
pub const PAGE_FRACTION: f64 = 0.875;

/// Page Up/Down and a wheel set to scroll a page move by this much of a `viewport` tall view
/// (`cc::ScrollUtils::CalculatePageStep`).
pub fn page_step(viewport: f64) -> f64 {
    (viewport * PAGE_FRACTION).trunc().max(1.0)
}

impl WheelUnits {
    /// This system's units, read now (Windows: the wheel setting can change while the app runs).
    pub fn system() -> Self {
        #[cfg(windows)]
        {
            win::units()
        }
        #[cfg(not(windows))]
        {
            Self { lines_per_notch: Some(3.0), px_per_line: 40.0 }
        }
    }

    /// The vertical movement `delta` asks for in a view `viewport` pixels tall; `None` when it has none (a horizontal
    /// or empty event).
    pub fn step(self, delta: ScrollDelta, viewport: f64) -> Option<Step> {
        let step = match delta {
            ScrollDelta::Pixels(pixels) => Step::Precise(-f64::from(f32::from(pixels.y))),
            ScrollDelta::Lines(lines) => {
                let lines = f64::from(lines.y);
                match self.lines_per_notch {
                    None => Step::Notches(-lines.signum() * page_step(viewport) * notches(lines, None)),
                    Some(per_notch) if per_notch > 0.0 => {
                        let pixels = -lines * f64::from(self.px_per_line);
                        if notches(lines, Some(per_notch)) >= 1.0 - 1e-3 {
                            Step::Notches(pixels)
                        } else {
                            Step::Precise(pixels)
                        }
                    }
                    Some(_) => return None,
                }
            }
        };
        match step {
            Step::Notches(pixels) | Step::Precise(pixels) if pixels.is_finite() && pixels != 0.0 => Some(step),
            _ => None,
        }
    }
}

/// How many notches `lines` is (at least one when a notch is a page: GPUI reports `WHEEL_PAGESCROLL` as a huge
/// number of lines).
fn notches(lines: f64, per_notch: Option<f32>) -> f64 {
    match per_notch {
        Some(per_notch) => (lines / f64::from(per_notch)).abs(),
        None => 1.0,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // One SystemParametersInfoW read into a local.
mod win {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };

    use super::WheelUnits;

    /// `WHEEL_PAGESCROLL`: the wheel scrolls a page a notch.
    const WHEEL_PAGESCROLL: u32 = u32::MAX;
    /// Chromium's `kScrollbarPixelsPerLine`.
    const PX_PER_LINE: f32 = 100.0 / 3.0;

    pub(super) fn units() -> WheelUnits {
        let mut lines: u32 = 3;
        // SAFETY: SPI_GETWHEELSCROLLLINES writes one UINT into `lines`, which outlives the call.
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some(std::ptr::from_mut(&mut lines).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        // Chromium keeps its default of three lines when the read fails.
        let lines = if read.is_ok() { lines } else { 3 };
        let lines_per_notch = if lines == WHEEL_PAGESCROLL { None } else { Some(lines as f32) };
        WheelUnits { lines_per_notch, px_per_line: PX_PER_LINE }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{point, px};

    use super::*;

    const WINDOWS: WheelUnits = WheelUnits { lines_per_notch: Some(3.0), px_per_line: 100.0 / 3.0 };
    const LINUX: WheelUnits = WheelUnits { lines_per_notch: Some(3.0), px_per_line: 40.0 };

    fn lines(y: f32) -> ScrollDelta {
        ScrollDelta::Lines(point(0.0, y))
    }

    #[test]
    fn a_notch_is_what_chromium_scrolls() {
        // GPUI: notch down = -3 lines.
        let Some(Step::Notches(windows)) = WINDOWS.step(lines(-3.0), 600.0) else { panic!() };
        assert!((windows - 100.0).abs() < 1e-4);
        assert_eq!(LINUX.step(lines(3.0), 600.0), Some(Step::Notches(-120.0)));
        assert_eq!(LINUX.step(lines(-6.0), 600.0), Some(Step::Notches(240.0)));
    }

    #[test]
    fn less_than_a_notch_is_followed() {
        // A high-resolution wheel's 30/120 of a notch, an X11 touchpad's fraction.
        let Some(Step::Precise(quarter)) = WINDOWS.step(lines(-0.75), 600.0) else { panic!() };
        assert!((quarter - 25.0).abs() < 1e-4);
        assert!(matches!(LINUX.step(lines(0.37), 600.0), Some(Step::Precise(_))));
    }

    #[test]
    fn pixels_pass_through() {
        assert_eq!(LINUX.step(ScrollDelta::Pixels(point(px(0.), px(-7.5))), 600.0), Some(Step::Precise(7.5)));
    }

    #[test]
    fn the_wheel_setting_decides_what_a_notch_is() {
        let one_line = WheelUnits { lines_per_notch: Some(1.0), ..WINDOWS };
        assert!(matches!(one_line.step(lines(-1.0), 600.0), Some(Step::Notches(_))));
        let page = WheelUnits { lines_per_notch: None, ..WINDOWS };
        assert_eq!(page.step(lines(-4_294_967_295.0), 600.0), Some(Step::Notches(525.0)));
        assert_eq!(page.step(lines(4_294_967_295.0), 600.0), Some(Step::Notches(-525.0)));
        let off = WheelUnits { lines_per_notch: Some(0.0), ..WINDOWS };
        assert_eq!(off.step(lines(0.0), 600.0), None);
    }

    #[test]
    fn nothing_vertical_is_nothing() {
        assert_eq!(WINDOWS.step(ScrollDelta::Lines(point(3.0, 0.0)), 600.0), None);
        assert_eq!(WINDOWS.step(lines(f32::NAN), 600.0), None);
    }

    #[test]
    fn page_steps_keep_an_eighth() {
        assert_eq!(page_step(600.0), 525.0);
        assert_eq!(page_step(0.5), 1.0);
    }
}
