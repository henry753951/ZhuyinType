//! App-wide state every view reads: the settings, the running service (when the engine is loaded) and the settings
//! window's navigation.  Views change settings through `set_setting`, which saves, notifies and redraws every window.

use std::time::Instant;

use gpui_kit::{App, Bounds, Global, Pixels, ScrollHandle, SharedString, Window, point, px};
use serde_json::Value;
use zt_core::service::Service;
use zt_core::settings::Settings;

use crate::pages::Page;

/// Shared state, installed once at start-up.
pub struct AppState {
    pub settings: Settings,
    /// The engine, personal data, statistics, shortcuts and the broker; None until it has started (or when it
    /// could not: then pages show their empty states and `service_error` says why).
    pub service: Option<Service>,
    pub service_error: Option<String>,
}

impl Global for AppState {}

/// The settings.
pub fn settings(cx: &App) -> &Settings {
    &cx.global::<AppState>().settings
}

/// The running service, if any.  Pages read personal data, statistics, shortcuts and the correction log through it.
pub fn service(cx: &App) -> Option<&Service> {
    cx.global::<AppState>().service.as_ref()
}

/// Sets a setting and redraws every window (the wizard and the settings window show the same values).
pub fn set_setting(key: &str, value: Value, cx: &mut App) {
    match settings(cx).set(key, value) {
        Ok(()) => cx.refresh_windows(),
        Err(error) => log::warn!("setting {key}: {error:#}"),
    }
}

/// Puts a setting back to its default.
pub fn reset_setting(key: &str, cx: &mut App) {
    if let Err(error) = settings(cx).reset(key) {
        log::warn!("resetting {key}: {error:#}");
    }
    cx.refresh_windows();
}

/// A place in the settings window: a page, optionally one of its sub-pages, optionally a row to scroll to and light
/// up (the row's anchor, or a section title for the whole section).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub page: Page,
    pub subpage: Option<SharedString>,
    pub row: Option<SharedString>,
}

impl Place {
    pub fn page(page: Page) -> Self {
        Self { page, subpage: None, row: None }
    }

    pub fn subpage(page: Page, subpage: impl Into<SharedString>) -> Self {
        Self { page, subpage: Some(subpage.into()), row: None }
    }

    pub fn row(page: Page, row: impl Into<SharedString>) -> Self {
        Self { page, subpage: None, row: Some(row.into()) }
    }
}

/// Which way the last navigation went, for the push/pop motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Another page from the sidebar: a cross-fade.
    Switch,
    /// Into a sub-page: it slides in from the trailing side.
    Push,
    /// Back out of a sub-page.
    Pop,
}

/// The settings window's navigation, read by pages to know which sub-page to draw and which row just arrived.
pub struct Navigation {
    pub page: Page,
    pub subpage: Option<SharedString>,
    /// Bumped on every navigation; animations key on it so they restart.
    pub generation: usize,
    pub direction: Direction,
    /// The row (or section) a link led to, and when: it tints once and fades.
    pub arrival: Option<(SharedString, Instant)>,
    /// The 快捷鍵 page's search text.
    pub query: SharedString,
    /// The page's scroll position; links scroll the arrival row into view.
    pub scroll: ScrollHandle,
    /// The arrival still to scroll to and to give the keyboard focus, after the next layout.
    arrive: Option<Arrive>,
}

/// What an arrival still has to do once its row is laid out (SETTINGS_IA.md §4.1).
struct Arrive {
    anchor: SharedString,
    scrolled: bool,
    focused: bool,
    /// Where each titled section of the page was laid out this frame, so a row's section heading stays in view.
    sections: Vec<Bounds<Pixels>>,
}

impl Global for Navigation {}

/// How long a link waits for its row to be drawn before it gives up scrolling to it.
const ARRIVE_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

impl Navigation {
    pub fn new(page: Page) -> Self {
        Self {
            page,
            subpage: None,
            generation: 0,
            direction: Direction::Switch,
            arrival: None,
            query: SharedString::default(),
            scroll: ScrollHandle::new(),
            arrive: None,
        }
    }

    /// Goes to `place`: the page (and its sub-page) at the top; with a row, the arrival: scroll to it, focus it, tint
    /// it once.
    pub fn go(&mut self, place: Place) {
        let direction = if place.page != self.page {
            Direction::Switch
        } else if place.subpage.is_some() && place.subpage != self.subpage {
            Direction::Push
        } else if place.subpage.is_none() && self.subpage.is_some() {
            Direction::Pop
        } else {
            self.direction
        };
        let moved = place.page != self.page || place.subpage != self.subpage;
        if place.page != self.page {
            self.query = SharedString::default();
        }
        self.page = place.page;
        self.subpage = place.subpage;
        if moved {
            self.direction = direction;
            self.generation += 1;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.arrive =
            place.row.clone().map(|anchor| Arrive { anchor, scrolled: false, focused: false, sections: Vec::new() });
        self.arrival = place.row.map(|row| (row, Instant::now()));
    }

    /// Back out of the sub-page.
    pub fn back(&mut self) {
        if self.subpage.is_some() {
            self.subpage = None;
            self.direction = Direction::Pop;
            self.generation += 1;
            self.arrival = None;
            self.arrive = None;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
    }

    /// The arrival still waiting to be scrolled to, if it has not waited too long.
    fn waiting(&mut self) -> Option<&mut Arrive> {
        let fresh = self.arrival.as_ref().is_some_and(|(_, at)| at.elapsed() < ARRIVE_WAIT);
        if !fresh {
            self.arrive = None;
        }
        self.arrive.as_mut()
    }

    /// The anchor a link just led to, while it is still to be scrolled to or focused.
    pub fn arriving(&self) -> Option<&str> {
        self.arrive.as_ref().filter(|arrive| !arrive.scrolled || !arrive.focused).map(|arrive| arrive.anchor.as_ref())
    }
}

/// What a link leads to: one row, or a whole section (its heading goes near the top).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Row,
    Section,
}

/// Called by every titled section once laid out while a link waits: where it is, so the row's heading stays in view.
pub fn section_laid_out(bounds: Bounds<Pixels>, cx: &mut App) {
    if let Some(arrive) = cx.global_mut::<Navigation>().waiting()
        && !arrive.scrolled
    {
        // Every frame lays the page out again; the newest bounds of the sections are at the end.
        if arrive.sections.len() > 512 {
            arrive.sections.clear();
        }
        arrive.sections.push(bounds);
    }
}

/// Called by a row or section with `anchor` once laid out: if a link is waiting to show it, scroll it to about the
/// top third with its section's heading in view (a section: its heading near the top).  The scroll is a jump, so
/// it is the same under reduced motion.
pub fn scroll_into_view(anchor: &str, target: Target, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let nav = cx.global_mut::<Navigation>();
    let viewport = nav.scroll.bounds();
    let offset = nav.scroll.offset();
    let max = nav.scroll.max_offset().y;
    let Some(arrive) = nav.waiting().filter(|arrive| arrive.anchor.as_ref() == anchor && !arrive.scrolled) else {
        return;
    };
    arrive.scrolled = true;
    let sections = std::mem::take(&mut arrive.sections);
    let y = arrival_offset(target, bounds, &sections, viewport, offset.y, max);
    nav.scroll.set_offset(point(offset.x, y));
    window.refresh();
}

/// The scroll offset that shows `bounds` as an arrival (see [`scroll_into_view`]), within `[-max, 0]`.
pub fn arrival_offset(
    target: Target,
    bounds: Bounds<Pixels>,
    sections: &[Bounds<Pixels>],
    viewport: Bounds<Pixels>,
    offset: Pixels,
    max: Pixels,
) -> Pixels {
    let margin = px(24.);
    let height = viewport.size.height;
    let top = match target {
        Target::Section => margin,
        Target::Row => {
            // The section the row is in: the last one laid out around it.
            let heading = sections
                .iter()
                .rev()
                .find(|section| section.top() <= bounds.top() && section.bottom() >= bounds.bottom())
                .map_or(bounds.top(), |section| section.top());
            let below_heading = bounds.top() - heading + margin;
            (height / 3.).max(below_heading).min(height - bounds.size.height - margin).max(margin)
        }
    };
    (offset - (bounds.top() - (viewport.top() + top))).min(px(0.)).max(-max)
}

/// Called by a row or section with `anchor` when drawn: if a link led to it, give `focus` the keyboard focus once
/// (the row, not a Tab stop itself, hands Tab on to its control).
pub fn focus_on_arrival(anchor: &str, focus: &gpui_kit::FocusHandle, window: &mut Window, cx: &mut App) {
    let Some(arrive) = cx.global_mut::<Navigation>().waiting().filter(|a| a.anchor.as_ref() == anchor && !a.focused)
    else {
        return;
    };
    arrive.focused = true;
    let focus = focus.clone();
    window.defer(cx, move |window, cx| window.focus(&focus, cx));
}

/// Scrolls the page just enough to show `bounds` (something the keyboard moved to), keeping a margin; the top wins
/// when it is taller than the page.
pub fn reveal(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let nav = cx.global_mut::<Navigation>();
    let viewport = nav.scroll.bounds();
    let margin = px(16.);
    let offset = nav.scroll.offset();
    let mut y = offset.y;
    if bounds.bottom() > viewport.bottom() - margin {
        y -= bounds.bottom() - (viewport.bottom() - margin);
    }
    if bounds.top() + (y - offset.y) < viewport.top() + margin {
        y = offset.y + (viewport.top() + margin - bounds.top());
    }
    let y = y.min(px(0.)).max(-nav.scroll.max_offset().y);
    if y != offset.y {
        nav.scroll.set_offset(point(offset.x, y));
        window.refresh();
    }
}

/// The navigation state.
pub fn nav(cx: &App) -> &Navigation {
    cx.global::<Navigation>()
}

/// Goes to `place` in the settings window and redraws it.
pub fn go(place: Place, cx: &mut App) {
    cx.global_mut::<Navigation>().go(place);
    cx.refresh_windows();
}

/// Back out of the current sub-page.
pub fn back(cx: &mut App) {
    cx.global_mut::<Navigation>().back();
    cx.refresh_windows();
}
