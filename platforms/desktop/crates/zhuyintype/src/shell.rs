//! The settings window: an app-drawn title bar over a sidebar and one page at a time, like the macOS app's System
//! Settings layout.  The window has no system frame; the title bar moves it and draws the window controls of Windows
//! and Linux in the app's own style.  Sub-pages slide in from the trailing side with a back button in the bar.

use std::collections::HashMap;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, TitleBar, window_border};

use crate::ui::title_bar::title_bar;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyView, App, AppContext as _, Bounds, Context, Entity, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, ease_out_quint, img, px, relative, size,
};

use crate::app::{self, Direction, Navigation, Place};
use crate::pages::{self, Page};
use crate::theme::{self, motion, size as sz, space, text, weight};
use crate::ui::{h, v};

/// The app's mark in the title bar.
const MARK: Pixels = px(18.);
const MARK_RADIUS: Pixels = px(5.);
/// How far a page slides when a sub-page is pushed or popped.
const PUSH_SHIFT: f32 = 28.;

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
            // The window's frame colour shows through: one continuous frame, also over a backdrop.
            .px(space::S12)
            .pt(space::S4)
            .pb(space::S12)
            .overflow_y_scroll()
            .children(Page::GROUPS.iter().enumerate().map(|(index, (title, pages))| {
                v().when(index > 0, |this| this.pt(space::S16))
                    .gap(space::S2)
                    .child(
                        div()
                            .px(space::S12)
                            .pb(space::S4)
                            .text_size(text::CAPTION)
                            .text_color(p.text_tertiary)
                            .child(*title),
                    )
                    .children(pages.iter().map(|&page| {
                        let active = page == selected;
                        h().id(SharedString::from(format!("sidebar-{}", page.id())))
                            .h(sz::SIDEBAR_ROW)
                            .px(space::S12)
                            .gap(space::S12)
                            .rounded(sz::R_MD)
                            .cursor_pointer()
                            .when(active, |this| this.bg(p.pressed))
                            .when(!active, |this| {
                                this.hover(|style| style.bg(p.hover)).active(|style| style.bg(p.pressed))
                            })
                            .on_click(move |_, _, cx| app::go(Place::page(page), cx))
                            .child(Icon::new(page.icon()).size(sz::SIDEBAR_ICON).text_color(if active {
                                p.text
                            } else {
                                p.text_secondary
                            }))
                            .child(
                                div()
                                    .text_size(text::BODY)
                                    .text_color(if active { p.text } else { p.text_secondary })
                                    .child(page.title()),
                            )
                    }))
            }))
    }

    /// The title bar: the app's mark over the sidebar, a drag area, the window controls.  Page titles live in the
    /// page itself.
    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        // The app's name small and quiet, as Discord's and Claude's bars carry it: the page has the large title.
        title_bar(sz::TITLE_BAR, gpui_kit::transparent_black()).child(
            h().h_full()
                .px(space::S24)
                .gap(space::S8)
                .child(img("app-icon.png").size(MARK).rounded(MARK_RADIUS))
                .child(div().text_size(text::CAPTION).text_color(p.text_secondary).child("ZhuyinType")),
        )
    }

    /// The page's heading: a back link to the parent page on a sub-page, then the large title.
    fn page_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let nav = app::nav(cx);
        let page = nav.page;
        let subpage = nav.subpage.as_ref().map(|id| pages::subpage_title(page, id).unwrap_or(id.as_ref()).to_owned());
        let title: SharedString = subpage.clone().unwrap_or_else(|| page.title().to_owned()).into();
        let shows_title = pages::shows_title(page, nav.subpage.as_deref());
        // The back link's chevron hangs into the margin so its text lines up with the title's.
        let hang = sz::ROW_PADDING_X - sz::ICON_SM - space::S2 - space::S4;
        v().when(shows_title, |this| this.pb(sz::TITLE_GAP))
            .when(subpage.is_some(), |this| {
                this.child(
                    h().id("back")
                        .mb(space::S4)
                        .ml(hang)
                        .h(sz::H_SM)
                        .pl(space::S4)
                        .pr(space::S8)
                        .gap(space::S2)
                        .rounded(sz::R_SM)
                        .cursor_pointer()
                        .text_size(text::SUBTITLE)
                        .text_color(p.text_secondary)
                        .hover(|style| style.bg(p.hover).text_color(p.text))
                        .active(|style| style.bg(p.pressed))
                        .on_click(|_, _, cx| app::back(cx))
                        .child(Icon::new(IconName::ChevronLeft).size(sz::ICON_SM))
                        .child(page.title()),
                )
            })
            .when(shows_title, |this| {
                this.child(
                    div()
                        .px(sz::ROW_PADDING_X)
                        .text_size(text::TITLE)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(p.text)
                        .child(title),
                )
            })
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.pending_appearance.take() {
            Some(forced) => theme::force(forced, window, cx),
            None => theme::sync(window, cx),
        }
        crate::platform::backdrop::apply(window, cx);
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
            .pb(sz::PAGE_PADDING_BOTTOM)
            .when_some(cx.global::<app::AppState>().service_error.clone(), |this, error| {
                this.child(
                    div()
                        .mb(space::S20)
                        .px(sz::ROW_PADDING_X)
                        .py(space::S8)
                        .rounded(sz::R_SM)
                        .border_1()
                        .border_color(p.danger)
                        .text_size(text::SUBTITLE)
                        .text_color(p.danger)
                        .child(SharedString::from(format!("輸入法服務沒有啟動，打字時不會使用 ZhuyinType：{error}"))),
                )
            })
            .child(self.page_header(cx))
            .child(view)
            .with_animation(
                SharedString::from(format!("page-{generation}")),
                Animation::new(if direction == Direction::Switch { motion::NORMAL } else { motion::PAGE })
                    .with_easing(ease_out_quint()),
                move |this, t| {
                    let shift = match direction {
                        Direction::Switch => 0.,
                        Direction::Push => PUSH_SHIFT * (1. - t),
                        Direction::Pop => -PUSH_SHIFT * (1. - t),
                    };
                    this.left(px(shift)).opacity(t)
                },
            );
        window_border().child(
            v().size_full()
                .bg(p.frame)
                .text_color(p.text)
                .font(theme::font())
                // CJK text needs room between lines.
                .line_height(relative(1.5))
                .child(self.title_bar(cx))
                .child(
                    h().flex_1().min_h(px(0.)).items_start().child(self.sidebar(cx)).child(
                        div().flex_1().min_w(px(0.)).h_full().pr(sz::SURFACE_INSET).pb(sz::SURFACE_INSET).child(
                            div()
                                .id("page")
                                .size_full()
                                // Without this the page grows to its rows' unwrapped width in a narrow window.
                                .min_w(px(0.))
                                .bg(p.page)
                                .rounded(sz::R_XL)
                                .border_1()
                                .border_color(p.surface_border)
                                // A shadow would show through a page tinted over a backdrop.
                                .when(p.material == theme::Material::Opaque, |this| this.shadow_xs())
                                .overflow_y_scroll()
                                .track_scroll(&scroll)
                                .child(content),
                        ),
                    ),
                ),
        )
    }
}

/// Opens the settings window on `page`.
pub fn open(
    page: Page,
    dark: Option<bool>,
    window_size: Option<(f32, f32)>,
    cx: &mut App,
) -> anyhow::Result<Entity<SettingsWindow>> {
    if !cx.has_global::<Navigation>() {
        cx.set_global(Navigation::new(page));
    } else {
        cx.global_mut::<Navigation>().go(Place::page(page));
    }
    let (width, height) = window_size.map_or((sz::WINDOW_W, sz::WINDOW_H), |(w, h)| (px(w), px(h)));
    let bounds = Bounds::centered(None, size(width, height), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(sz::WINDOW_MIN_W, sz::WINDOW_MIN_H)),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("zhuyintype".into()),
        ..TitleBar::window_options()
    };
    let (handle, view) = gpui_kit::open_window(options, cx, |window, cx| {
        if !cx.has_global::<theme::Tokens>() {
            theme::init(window.appearance(), cx);
        }
        #[cfg(windows)]
        crate::platform::windows::native_frame(window);
        cx.new(|cx| SettingsWindow::new(dark, window, cx))
    })?;
    cx.set_global(OpenSettings(Some(handle)));
    Ok(view)
}

/// The settings window while it is open.
struct OpenSettings(Option<gpui_kit::AnyWindowHandle>);

impl gpui_kit::Global for OpenSettings {}

/// Brings the settings window to the front, opening it if it is closed (a second launch, the taskbar menu).
pub fn show(cx: &mut App) {
    let existing = cx.try_global::<OpenSettings>().and_then(|open| open.0);
    if let Some(handle) = existing
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        return;
    }
    let page = cx.try_global::<Navigation>().map_or(Page::Intro, |nav| nav.page);
    if let Err(error) = open(page, None, None, cx) {
        log::error!("opening the settings window: {error:#}");
        return;
    }
    // Started in the background at login, the first time the settings appear is the first use.
    crate::wizard::show_on_first_use(cx);
}

/// Whether the settings window is open.
pub fn is_open(cx: &mut App) -> bool {
    cx.try_global::<OpenSettings>()
        .and_then(|open| open.0)
        .is_some_and(|handle| handle.update(cx, |_, _, _| ()).is_ok())
}

/// Brings the settings window to the front if it is open (the setup wizard closing); never opens it.
pub fn bring_forward(cx: &mut App) {
    if let Some(handle) = cx.try_global::<OpenSettings>().and_then(|open| open.0) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}
