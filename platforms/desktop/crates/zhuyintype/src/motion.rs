//! Motion beyond the page durations in `theme::motion`: how selection, disclosure, pressing and the floating windows
//! move, the easing curves, the keyboard focus ring, and the system's request for less motion.
//!
//! Two kinds of motion, as macOS treats them under Reduce Motion:
//!
//! - **Spatial** (a slide, a nudge, a rotation): skipped when the system asks for less motion; the value arrives at
//!   once.
//! - **Fades** (a selection fill appearing, the badge fading): kept, short, because they carry no movement.
//!
//! The floating windows (candidate window, 中/英 badge) read `Panel` once per update; nothing here runs per frame
//! unless something is moving, and keys never wait for motion.

use std::time::Duration;

#[cfg(target_os = "linux")]
use gpui_kit::Global;
use gpui_kit::base::{ReducedMotion, Transition, TransitionId, transition};
use gpui_kit::{App, BoxShadow, Hsla, StyleRefinement, Styled as _, Window, point, px};

use crate::theme::Palette;

/// A selected item's fill appearing (the sidebar, segments).
pub const SELECT: Duration = Duration::from_millis(140);
/// A disclosure chevron turning.
pub const DISCLOSE: Duration = Duration::from_millis(160);

/// The floating windows, beside the candidate window's own timings (`candidate::panel::motion`).  Milliseconds and
/// logical pixels.
pub mod panel {
    /// The 中/英 badge appears faster than the candidate window: it answers a key.
    pub const BADGE_APPEAR: f32 = 90.;
    pub const BADGE_SLIDE: f32 = 3.;
    /// The badge starts at this opacity rather than clear, so it reads at once.
    pub const BADGE_FROM: f32 = 0.35;
}

/// Ease-out (CoreAnimation's `easeOut`, quadratic).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t) * (1. - t)
}

/// Ease-in-out (quadratic).
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.5 { 2. * t * t } else { 1. - (-2. * t + 2.).powi(2) / 2. }
}

/// Ease-out, cubic: settles a little more softly than `ease_out` (selection, disclosure).
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

/// Whether the system (or `ZT_REDUCE_MOTION`) asks for less motion.
pub fn reduced(cx: &App) -> bool {
    cx.reduce_motion()
}

/// How the floating windows move, decided once per update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    /// The 動畫 setting: page turns, the bounce, appearing and the badge's fade.
    pub animates: bool,
    /// The system asks for less motion: slides become fades, the bounce a dim.
    pub reduced: bool,
}

impl Panel {
    pub fn new(animates: bool, cx: &App) -> Self {
        Self { animates, reduced: reduced(cx) }
    }

    /// Whether things travel (a slide or a nudge), rather than only fade.
    pub fn slides(self) -> bool {
        self.animates && !self.reduced
    }
}

/// A fade toward `on` (1) or off (0), keyed by `id` within the element being drawn.  Fades keep their timing under
/// reduced motion.  The first value is adopted at once: nothing fades in when a page first draws.
pub fn fade(id: impl Into<TransitionId>, on: bool, duration: Duration, window: &mut Window, cx: &mut App) -> f32 {
    let policy = Transition::new(duration).ease(ease_out_cubic).reduced_motion(ReducedMotion::Continue);
    transition(id, if on { 1f32 } else { 0f32 }, policy, window, cx)
}

/// A spatial value (an angle, an offset) gliding to `target`, keyed by `id`; arrives at once under reduced motion.
pub fn glide(id: impl Into<TransitionId>, target: f32, duration: Duration, window: &mut Window, cx: &mut App) -> f32 {
    let policy = Transition::new(duration).ease(ease_out_cubic).reduced_motion(ReducedMotion::Finish);
    transition(id, target, policy, window, cx)
}

/// The keyboard focus ring: two pixels of the accent around the element, drawn as a shadow so it takes no room and
/// follows the element's corners.  Shown only while the keyboard moved the focus (`focus_visible`).
///
/// GPUI fills a shadow's whole box, so the element takes `under` (the surface it sits on) as its background while the
/// ring shows, and only the two pixels outside it stay visible.  Fills drawn as children (selection, hover) still
/// show on top of it.
pub fn focus_ring(style: StyleRefinement, p: &Palette, under: Hsla) -> StyleRefinement {
    style.bg(under).shadow(ring(p.accent.opacity(if p.dark { 0.8 } else { 0.6 })))
}

fn ring(color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow { color, offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }]
}

// MARK: - The system's preference

/// What the app last read from GNOME's switch, which GPUI's own reading does not cover.
#[cfg(target_os = "linux")]
struct GnomeMotion(bool);

#[cfg(target_os = "linux")]
impl Global for GnomeMotion {}

/// Reads the system's preference for less motion where GPUI's own reading (at `gpui_kit::init`: Windows'
/// 「動畫效果」, the desktop portal's `reduced-motion`) misses it.  Call once after `gpui_kit::init`.
///
/// - `ZT_REDUCE_MOTION=1` or `0` decides (screenshots, testing).
/// - Linux: GNOME's `org.gnome.desktop.interface enable-animations`, read off the UI thread, for desktops whose portal
///   does not offer `reduced-motion` yet.
pub fn init(cx: &mut App) {
    if let Some(forced) = std::env::var("ZT_REDUCE_MOTION").ok().and_then(|value| match value.as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }) {
        cx.set_reduce_motion(forced);
        return;
    }
    refresh(cx);
}

/// Reads the preference again (the settings window or the wizard came forward): Windows posts no change GPUI
/// follows, and GNOME's switch is read only here.
pub fn refresh(cx: &mut App) {
    if std::env::var_os("ZT_REDUCE_MOTION").is_some() {
        return;
    }
    #[cfg(windows)]
    gpui_kit::base::apply_system_reduce_motion(cx);
    #[cfg(target_os = "linux")]
    {
        let reading = cx.background_executor().spawn(async { gnome_reduces_motion() });
        cx.spawn(async move |cx| {
            let Some(reduce) = reading.await else { return };
            cx.update(|cx| {
                let before = cx.try_global::<GnomeMotion>().is_some_and(|reading| reading.0);
                // GNOME's switch turned off animations, or turned them back on after this app followed it; otherwise
                // the portal's reading stands.
                if reduce || before {
                    cx.set_reduce_motion(reduce);
                }
                cx.set_global(GnomeMotion(reduce));
            });
        })
        .detach();
    }
}

/// GNOME's 「動畫」 switch: Some(true) when it is off.  None when GNOME's settings are not there.
#[cfg(target_os = "linux")]
fn gnome_reduces_motion() -> Option<bool> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "false" => Some(true),
        "true" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_start_at_zero_and_end_at_one() {
        for ease in [ease_out, ease_in_out, ease_out_cubic] {
            assert!(ease(0.).abs() < f32::EPSILON);
            assert!((ease(1.) - 1.).abs() < f32::EPSILON);
            assert!(ease(-1.).abs() < f32::EPSILON);
            assert!((ease(2.) - 1.).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn reduced_motion_keeps_fades_and_drops_slides() {
        let panel = Panel { animates: true, reduced: true };
        assert!(panel.animates && !panel.slides());
        assert!(Panel { animates: true, reduced: false }.slides());
        assert!(!Panel { animates: false, reduced: false }.slides());
    }
}
