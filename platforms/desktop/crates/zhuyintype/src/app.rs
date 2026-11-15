//! App-wide state every view reads: the settings, the running service (when the engine is loaded) and the settings
//! window's navigation.  Views change settings through `set_setting`, which saves, notifies and redraws every window.

use std::time::Instant;

use gpui_kit::{App, Bounds, Global, Pixels, ScrollHandle, SharedString, Window, point, px};
use serde_json::Value;
use zt_core::settings::Settings;

use crate::pages::Page;

/// Shared state, installed once at start-up.
pub struct AppState {
    pub settings: Settings,
}

impl Global for AppState {}

/// The settings.
pub fn settings(cx: &App) -> &Settings {
    &cx.global::<AppState>().settings
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

    pub fn in_subpage(mut self, subpage: impl Into<SharedString>) -> Self {
        self.subpage = Some(subpage.into());
        self
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
    /// The row still to scroll to after the next layout.
    pub pending_scroll: Option<SharedString>,
}

impl Global for Navigation {}

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
            pending_scroll: None,
        }
    }

    /// Goes to `place`.
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
        self.pending_scroll = place.row.clone();
        self.arrival = place.row.map(|row| (row, Instant::now()));
    }

    /// Back out of the sub-page.
    pub fn back(&mut self) {
        if self.subpage.is_some() {
            self.subpage = None;
            self.direction = Direction::Pop;
            self.generation += 1;
            self.arrival = None;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
    }
}

/// Called by a row or section with `anchor` once laid out: if a link is waiting to show it, scroll it near the top.
pub fn scroll_into_view(anchor: &str, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let nav = cx.global_mut::<Navigation>();
    if nav.pending_scroll.as_deref() != Some(anchor) {
        return;
    }
    nav.pending_scroll = None;
    let viewport = nav.scroll.bounds();
    let offset = nav.scroll.offset();
    let target = offset.y - (bounds.origin.y - viewport.origin.y - px(56.));
    let max = nav.scroll.max_offset().y;
    nav.scroll.set_offset(point(offset.x, target.min(px(0.)).max(-max)));
    window.refresh();
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
