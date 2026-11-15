//! A page drawn straight from the settings registry: its groups as sections, each setting as a row.  Pages start
//! from this and replace it with their macOS layout.

use gpui_kit::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, div, px};
use zt_core::settings;

use super::Page;
use crate::theme::{self, size};
use crate::ui::{self, v};

/// The registry's page key for `page`.
pub fn registry_key(page: Page) -> &'static str {
    match page {
        Page::Candidates => "appearance",
        other => other.id(),
    }
}

/// Every registry setting on `page`, grouped.
pub fn page(page: Page, cx: &App) -> AnyElement {
    let key = registry_key(page);
    let mut groups: Vec<(&'static str, Vec<&'static settings::Descriptor>)> = Vec::new();
    for descriptor in settings::descriptors().iter().filter(|d| d.page == key) {
        match groups.iter_mut().find(|(group, _)| *group == descriptor.group) {
            Some((_, list)) => list.push(descriptor),
            None => groups.push((descriptor.group.as_str(), vec![descriptor])),
        }
    }
    if groups.is_empty() {
        return div()
            .pt(px(80.))
            .w_full()
            .flex()
            .justify_center()
            .text_color(theme::palette(cx).text_tertiary)
            .child(SharedString::from(format!("「{}」頁面", page.title())))
            .into_any_element();
    }
    v().gap(size::SECTION_GAP)
        .children(groups.into_iter().map(|(group, list)| {
            ui::section(group).rows(list.into_iter().map(|descriptor| ui::setting_row(&descriptor.key, cx)))
        }))
        .into_any_element()
}
