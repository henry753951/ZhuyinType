//! The settings window: an app-drawn title bar over a sidebar and one page at a time, like the macOS app's System
//! Settings layout.  The window has no system frame; the title bar moves it and draws the window controls of Windows
//! and Linux in the app's own style.  Sub-pages slide in from the trailing side with a back button in the bar.

use std::collections::HashMap;

use gpui_kit::component::{Icon, TitleBar, window_border};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyView, App, AppContext as _, Bounds, Context, Entity, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowBounds, WindowDecorations, WindowOptions, div, ease_out_quint, img, px, size,
};

use crate::app::{self, Direction, Navigation, Place};
use crate::pages::{self, Page};
use crate::theme::{self, motion, size as sz, text};
use crate::ui::{h, v};

/// The settings window's state: the page views, created on first visit and kept.
pub struct SettingsWindow {
    views: HashMap<Page, AnyView>,
    /// An appearance to force on the first frame (`--theme`).
    pending_appearance: Option<Option<bool>>,
}

impl SettingsWindow {
    pub fn new(dark: Option<bool>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { views: HashMap::new(), pending_appearance: dark.map(Some) }
    }

    fn view(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        self.views.entry(page).or_insert_with(|| pages::make_view(page, window, cx)).clone()
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let selected = app::nav(cx).page;
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
                            .when(!active, |this| {
                                this.hover(|style| style.bg(p.hover)).active(|style| style.bg(p.pressed))
                            })
                            .on_click(move |_, _, cx| app::go(Place::page(page), cx))
                            .child(
                                div()
                                    .size(sz::SIDEBAR_ICON)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .bg(page.tint())
                                    .child(Icon::new(page.icon()).size(px(14.)).text_color(gpui_kit::white())),
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
        let nav = app::nav(cx);
        let page = nav.page;
        let subpage = nav.subpage.as_ref().map(|id| pages::subpage_title(page, id).unwrap_or(id.as_ref()).to_owned());
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
                    .child(match subpage {
                        None => div()
                            .pl(sz::PAGE_PADDING_X)
                            .text_size(text::HEADLINE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(page.title())
                            .into_any_element(),
                        Some(title) => h()
                            .pl(px(14.))
                            .gap(px(6.))
                            .child(
                                h().id("back")
                                    .h(px(28.))
                                    .pl(px(4.))
                                    .pr(px(8.))
                                    .gap(px(2.))
                                    .rounded(sz::RADIUS_CONTROL)
                                    .cursor_pointer()
                                    .text_color(p.accent)
                                    .hover(|style| style.bg(p.hover))
                                    .active(|style| style.bg(p.pressed))
                                    .on_click(|_, _, cx| app::back(cx))
                                    .child(Icon::new(IconName::ChevronLeft).size(px(16.)))
                                    .child(div().text_size(text::BODY).child(page.title())),
                            )
                            .child(
                                div()
                                    .text_size(text::HEADLINE)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.text)
                                    .child(title),
                            )
                            .into_any_element(),
                    }),
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.pending_appearance.take() {
            Some(forced) => theme::force(forced, window, cx),
            None => theme::sync(window, cx),
        }
        let p = theme::palette(cx);
        let nav = app::nav(cx);
        let (page, generation, direction, scroll) = (nav.page, nav.generation, nav.direction, nav.scroll.clone());
        let view = self.view(page, window, cx);
        let content = div()
            .relative()
            .w_full()
            .max_w(sz::CONTENT_MAX)
            .mx_auto()
            .px(sz::PAGE_PADDING_X)
            .pt(sz::PAGE_PADDING_TOP)
            .pb(px(40.))
            .child(view)
            .with_animation(
                SharedString::from(format!("page-{generation}")),
                Animation::new(if direction == Direction::Switch { motion::NORMAL } else { motion::PAGE })
                    .with_easing(ease_out_quint()),
                move |this, t| {
                    let shift = match direction {
                        Direction::Switch => 0.,
                        Direction::Push => 28. * (1. - t),
                        Direction::Pop => -28. * (1. - t),
                    };
                    this.left(px(shift)).opacity(t)
                },
            );
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
                            div()
                                .id("page")
                                .flex_1()
                                .h_full()
                                .overflow_y_scroll()
                                .track_scroll(&scroll)
                                .child(content),
                        ),
                ),
        )
    }
}

/// Opens the settings window on `page`.
pub fn open(page: Page, dark: Option<bool>, cx: &mut App) -> anyhow::Result<Entity<SettingsWindow>> {
    cx.set_global(Navigation::new(page));
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
        cx.new(|cx| SettingsWindow::new(dark, window, cx))
    })?;
    Ok(view)
}
