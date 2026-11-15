//! `--popover ID`: one ⓘ card drawn on its own in a small window, for screenshots of the cards and their demos (the
//! macOS app renders `InfoCard` on its own for the same reason).

use gpui_kit::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    WindowBounds, WindowOptions, div, px, size,
};

use crate::pages::{choosing, typing};
use crate::theme::{self, size as sz};
use crate::ui;
use crate::ui::demo::InfoContent;

type Card = (&'static str, InfoContent);

/// The card `id` of any page that has cards.
fn card(id: &str) -> Option<Card> {
    typing::info(id).or_else(|| choosing::info(id))
}

pub struct CardWindow {
    card: Option<Card>,
    id: SharedString,
    pending_appearance: Option<Option<bool>>,
}

impl Render for CardWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.pending_appearance.take() {
            Some(forced) => theme::force(forced, window, cx),
            None => theme::sync(window, cx),
        }
        let p = theme::palette(cx);
        let content = match self.card {
            Some((title, content)) => ui::info_card(title.into(), content(window, cx), cx),
            None => div().p(px(20.)).child(format!("沒有 {} 這張說明卡", self.id)).into_any_element(),
        };
        div().size_full().bg(p.window).text_color(p.text).font(theme::font()).p(px(16.)).child(
            div().rounded(sz::RADIUS_CARD).bg(p.card).border_1().border_color(p.separator).shadow_lg().child(content),
        )
    }
}

/// Opens the card `id` in a window of `window_size` (default 420×820).
pub fn open(id: &str, dark: Option<bool>, window_size: Option<(f32, f32)>, cx: &mut App) -> anyhow::Result<()> {
    let (width, height) = window_size.unwrap_or((420., 820.));
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
    let card = card(id);
    let id: SharedString = id.to_owned().into();
    gpui_kit::open_window(options, cx, move |window, cx| {
        theme::init(window.appearance(), cx);
        cx.new(|_| CardWindow { card, id, pending_appearance: dark.map(Some) })
    })?;
    Ok(())
}
