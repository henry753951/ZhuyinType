//! 「關於」 page.  Mirrors the macOS page of the same name; see the module docs of `pages` for the rules.

use gpui_kit::{Context, IntoElement, Render, Window};

use super::{Page, generic};

pub struct AboutPage;

impl AboutPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AboutPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        generic::page(Page::About, cx)
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(_id: &str) -> Option<&'static str> {
    None
}
