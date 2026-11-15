//! The settings window: an app-drawn title bar over a sidebar and one page at a time, like the macOS app's System
//! Settings layout.  The window has no system frame; the title bar moves it, and draws the window controls of
//! Windows and Linux in the app's own style.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{TitleBar, window_border};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, img, px, size,
};
use zt_core::settings::{self, Kind, Settings};

use crate::pages::Page;
use crate::theme::{self, size as sz, text};
use crate::ui::{self, h, v};

/// The settings window's state.
pub struct SettingsWindow {
    page: Page,
    settings: Settings,
    /// An appearance to force on the first frame (`--theme`).
    pending_appearance: Option<Option<bool>>,
}

impl SettingsWindow {
    pub fn new(page: Page, settings: Settings, dark: Option<bool>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { page, settings, pending_appearance: dark.map(Some) }
    }

    pub fn select(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page {
            self.page = page;
            cx.notify();
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let selected = self.page;
        v().id("sidebar")
            .w(sz::SIDEBAR)
            .h_full()
            .flex_shrink_0()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.separator)
            .px(px(10.))
            .pb(px(12.))
            .overflow_y_scroll()
            .children(Page::GROUPS.iter().map(|(title, pages)| {
                v().pt(px(14.))
                    .gap(px(1.))
                    .child(
                        div()
                            .px(px(10.))
                            .pb(px(4.))
                            .text_size(text::CAPTION)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text_tertiary)
                            .child(*title),
                    )
                    .children(pages.iter().map(|&page| {
                        let active = page == selected;
                        h().id(SharedString::from(format!("sidebar-{}", page.id())))
                            .h(sz::SIDEBAR_ROW)
                            .px(px(8.))
                            .gap(px(10.))
                            .rounded(sz::RADIUS_CONTROL)
                            .cursor_pointer()
                            .when(active, |this| this.bg(p.accent))
                            .when(!active, |this| this.hover(|style| style.bg(p.hover)).active(|style| style.bg(p.pressed)))
                            .on_click(cx.listener(move |this, _, _, cx| this.select(page, cx)))
                            .child(
                                div()
                                    .size(sz::SIDEBAR_ICON)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .bg(page.tint())
                                    .child(
                                        gpui_kit::component::Icon::new(page.icon())
                                            .size(px(14.))
                                            .text_color(gpui_kit::white()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(text::BODY)
                                    .text_color(if active { p.on_accent } else { p.text })
                                    .child(page.title()),
                            )
                    }))
            }))
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        TitleBar::new()
            .h(sz::TITLE_BAR)
            .pl(px(0.))
            .bg(p.window)
            .border_b_1()
            .border_color(p.separator)
            .child(
                h().h_full()
                    .child(
                        h().w(sz::SIDEBAR)
                            .h_full()
                            .flex_shrink_0()
                            .px(px(16.))
                            .gap(px(9.))
                            .bg(p.sidebar)
                            .border_r_1()
                            .border_color(p.separator)
                            .child(img("app-icon.png").size(px(20.)).rounded(px(5.)))
                            .child(
                                div()
                                    .text_size(text::BODY)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.text)
                                    .child("ZhuyinType"),
                            ),
                    )
                    .child(
                        div()
                            .pl(sz::PAGE_PADDING_X)
                            .text_size(text::HEADLINE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(self.page.title()),
                    ),
            )
    }

    /// A page drawn from the settings registry until its own module replaces it.
    fn registry_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let page_key = match self.page {
            Page::Candidates => "appearance",
            other => other.id(),
        };
        let mut groups: Vec<(&str, Vec<&settings::Descriptor>)> = Vec::new();
        for descriptor in settings::descriptors().iter().filter(|d| d.page == page_key) {
            match groups.iter_mut().find(|(group, _)| *group == descriptor.group) {
                Some((_, list)) => list.push(descriptor),
                None => groups.push((&descriptor.group, vec![descriptor])),
            }
        }
        if groups.is_empty() {
            return div()
                .pt(px(80.))
                .w_full()
                .flex()
                .justify_center()
                .text_color(p.text_tertiary)
                .child(format!("「{}」頁面", self.page.title()))
                .into_any_element();
        }
        v().gap(sz::SECTION_GAP)
            .children(groups.into_iter().map(|(group, list)| {
                ui::section(group.to_owned()).rows(list.into_iter().map(|descriptor| {
                    let value = self.settings.get(&descriptor.key);
                    let key = descriptor.key.clone();
                    let settings = self.settings.clone();
                    let trailing: AnyElement = match descriptor.kind {
                        Kind::Toggle => gpui_kit::component::switch::Switch::new(SharedString::from(key.clone()))
                            .checked(value.as_bool().unwrap_or(false))
                            .on_click(move |checked, window, _cx| {
                                let _ = settings.set(&key, serde_json::Value::Bool(*checked));
                                window.refresh();
                            })
                            .into_any_element(),
                        _ => div()
                            .text_size(text::BODY)
                            .text_color(p.text_secondary)
                            .child(descriptor.label(&value).map(str::to_owned).unwrap_or_else(|| value.to_string()))
                            .into_any_element(),
                    };
                    ui::row(SharedString::from(descriptor.key.clone()), descriptor.title.clone())
                        .subtitle_opt(descriptor.subtitle(&value).map(str::to_owned))
                        .trailing(trailing)
                }))
            }))
            .into_any_element()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.pending_appearance.take() {
            Some(forced) => theme::force(forced, window, cx),
            None => theme::sync(window, cx),
        }
        let p = theme::palette(cx);
        let page = self.registry_page(cx);
        window_border().child(
            v().size_full()
                .bg(p.window)
                .text_color(p.text)
                .font_family(theme::ui_font())
                .child(self.title_bar(cx))
                .child(
                    h().flex_1()
                        .min_h(px(0.))
                        .items_start()
                        .child(self.sidebar(cx))
                        .child(
                            div().id("page").flex_1().h_full().overflow_y_scroll().child(
                                div()
                                    .w_full()
                                    .max_w(sz::CONTENT_MAX)
                                    .mx_auto()
                                    .px(sz::PAGE_PADDING_X)
                                    .pt(sz::PAGE_PADDING_TOP)
                                    .pb(px(40.))
                                    .child(page),
                            ),
                        ),
                ),
        )
    }
}

/// Opens the settings window on `page`.
pub fn open(page: Page, settings: Settings, dark: Option<bool>, cx: &mut App) -> anyhow::Result<Entity<SettingsWindow>> {
    let bounds = Bounds::centered(None, size(sz::WINDOW_W, sz::WINDOW_H), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(sz::WINDOW_MIN_W, sz::WINDOW_MIN_H)),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("zhuyintype".into()),
        ..TitleBar::window_options()
    };
    let (_, view) = gpui_kit::open_window(options, cx, |window, cx| {
        theme::init(window.appearance(), cx);
        cx.new(|cx| SettingsWindow::new(page, settings, dark, window, cx))
    })?;
    Ok(view)
}
