//! `--panel VARIANT`: one candidate window (or notice, or badge) drawn over a few lines of a document, as the macOS
//! UI renderer's `panels/` show it, for screenshots.  `--popup demo` drives the real floating windows instead.
//!
//! Variants: `vertical-standard`, `vertical-compact`, `vertical-corrections`, the same three `horizontal-…`,
//! `minimal`, `composing`, `notice-saved`, `notice-select`, `notice-invalid`, `mode-toast`, and the paging frames
//! `vertical-next-35`, `horizontal-previous-20`, `vertical-end-50` … (`next`, `previous` or `end` at a percentage).

use std::rc::Rc;

use gpui_kit::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, point, px, size,
};
use serde_json::Value;
use zt_core::presentation::{Presentation, PreviewList, Rect};
use zt_core::service::{PopupAction, PopupState, UiEvent};

use super::layout::{Extras, WindowMeasure};
use super::panel::{CandidatePanel, Pinned, mode_badge};
use super::popup;
use super::sample;
use super::style::{CurrentKeys, PanelStyle, selection_keys};
use crate::app;
use crate::theme::Palette;

struct Showcase {
    presentation: Presentation,
    preview: Option<String>,
    badge: Option<&'static str>,
    panel: CandidatePanel,
    dark: bool,
    sized: bool,
}

/// The settings a variant draws with (on the screenshot's temporary settings file).
fn configure(variant: &str, cx: &mut App) {
    let mut values: Vec<(&str, Value)> = Vec::new();
    if variant.starts_with("horizontal") {
        values.push(("candidateLayout", "horizontal".into()));
    }
    if variant.contains("compact") {
        values.push(("candidateDensity", "compact".into()));
    }
    if variant == "minimal" {
        values.extend([
            ("selectionKeys", Value::from("asdfghjkl")),
            ("candidatesPerPage", Value::from(5)),
            ("candidateFontSize", Value::from(20)),
            ("showsListSwitches", Value::from(false)),
            ("showsAnnotations", Value::from(false)),
        ]);
    }
    for (key, value) in values {
        app::set_setting(key, value, cx);
    }
}

/// A variant's content: the presentation, the composition shown in the window, the badge, and a frozen motion.
type Content = (Presentation, Option<String>, Option<&'static str>, Option<(PreviewList, Pinned)>);

fn presentation_for(variant: &str, cx: &App) -> Content {
    let settings = app::settings(cx);
    let keys = selection_keys(settings);
    let per_page = keys.len();
    let words = sample::PANEL_WORDS;
    let notice = |text: &str| Presentation { notice: Some(text.to_owned()), ..Presentation::default() };
    match variant {
        "notice-saved" => (notice("已加入自訂詞：王小明"), None, None, None),
        "notice-select" => (notice("Enter 加入自訂詞：王小明"), None, None, None),
        "notice-invalid" => (notice("自訂詞需為 1 到 6 個字"), None, None, None),
        "mode-toast" => (Presentation::default(), None, Some("中"), None),
        "composing" => (Presentation::default(), Some("王小ㄇㄧㄥ".to_owned()), None, None),
        "minimal" => {
            let list = sample::list(sample::WORDS, sample::PANEL_CORRECTIONS, true, per_page, 0);
            (list.presentation("", &keys), None, None, None)
        }
        _ => {
            let corrections = variant.ends_with("corrections");
            let list = sample::list(words, sample::PANEL_CORRECTIONS, corrections, per_page, usize::from(!corrections));
            let parts: Vec<&str> = variant.split('-').collect();
            if let [_, kind @ ("next" | "previous" | "end"), percent] = parts[..] {
                let progress = percent.parse::<f32>().unwrap_or(0.) / 100.;
                let pinned = match kind {
                    "end" => Pinned::Bounce { direction: 1, progress },
                    "next" => Pinned::Page { direction: 1, progress },
                    _ => Pinned::Page { direction: -1, progress },
                };
                return (list.presentation("", &keys), None, None, Some((list, pinned)));
            }
            (list.presentation("", &keys), None, None, None)
        }
    }
}

impl Showcase {
    fn new(variant: &str, dark: bool, window: &mut Window, cx: &mut App) -> Self {
        configure(variant, cx);
        let settings = app::settings(cx).clone();
        let style = PanelStyle::from_settings(&settings, &CurrentKeys::new(&settings, None));
        let (mut presentation, preview, badge, paging) = presentation_for(variant, cx);
        if variant == "minimal" {
            presentation.reading = None;
        }
        let mut panel = CandidatePanel::new();
        panel.hover = false;
        let keys = selection_keys(&settings);
        if let Some((mut list, pinned)) = paging {
            // The page before the motion, then the one it goes to (or the last page, for the bounce).
            match pinned {
                Pinned::Page { direction: -1, .. } => {
                    list.turn_page(1);
                }
                Pinned::Bounce { .. } => {
                    list.turn_page(1);
                }
                Pinned::Page { .. } => {}
            }
            let before = list.presentation("", &keys);
            panel.update(&before, Extras::default(), &style, 4000., true, &mut WindowMeasure(window));
            if let Pinned::Page { direction, .. } = pinned {
                list.turn_page(i64::from(direction));
            }
            presentation = list.presentation("", &keys);
            panel.update(&presentation, Extras::default(), &style, 4000., true, &mut WindowMeasure(window));
            panel.pinned = Some(pinned);
        }
        Self { presentation, preview, badge, panel, dark, sized: false }
    }
}

impl Render for Showcase {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = if self.dark { Palette::dark() } else { Palette::light() };
        let settings = app::settings(cx).clone();
        let style = PanelStyle::from_settings(&settings, &CurrentKeys::new(&settings, None));
        let (width, height) = match self.badge {
            Some(_) => (super::layout::BADGE_W, super::layout::BADGE_H),
            None if self.panel.pinned.is_some() => self.panel.layout().map_or((0., 0.), |l| (l.width, l.height)),
            None => self.panel.update(
                &self.presentation,
                Extras { preview: self.preview.as_deref() },
                &style,
                4000.,
                true,
                &mut WindowMeasure(window),
            ),
        };
        let (inset_x, inset_y) = (20., 20.);
        let total = (width + inset_x * 2., height + inset_y * 2.);
        if !self.sized {
            self.sized = true;
            window.resize(size(px(total.0), px(total.1)));
        }
        // Grey bars standing for the document's lines behind the window.
        let bar = palette.separator;
        let lines = (0..((total.1 / 18.) as usize + 1)).map(move |index| {
            let y = 4. + index as f32 * 18.;
            div()
                .absolute()
                .left(px(12.))
                .top(px(y))
                .w(px(if index % 3 == 0 { total.0 * 0.6 } else { total.0 * 0.9 }))
                .h(px(6.))
                .rounded_full()
                .bg(bar)
        });
        let content = match self.badge {
            Some(text) => mode_badge(text, &palette, true),
            None => self.panel.render(&palette, None, window),
        };
        div()
            .relative()
            .size_full()
            .bg(palette.card)
            .children(lines)
            .child(div().absolute().left(px(inset_x)).top(px(inset_y)).child(content))
    }
}

/// Opens the showcase window for `variant`.
pub fn open(variant: String, dark: bool, cx: &mut App) -> anyhow::Result<()> {
    let bounds = Bounds::new(point(px(40.), px(40.)), size(px(700.), px(600.)));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("zhuyintype".into()),
        ..WindowOptions::default()
    };
    cx.open_window(options, move |window, cx| {
        crate::theme::init(window.appearance(), cx);
        let showcase = Showcase::new(&variant, dark, window, cx);
        cx.new(|_| showcase)
    })?;
    Ok(())
}

/// `--popup demo`: the real floating windows at a caret, driven by an example list; clicks page and pick in it.
pub fn popup_demo(cx: &mut App) {
    let caret = Rect::new(220., 160., 2., 20.);
    let settings = app::settings(cx).clone();
    let keys = selection_keys(&settings);
    let list = Rc::new(std::cell::RefCell::new(sample::list(sample::PANEL_WORDS, sample::PANEL_CORRECTIONS, false, keys.len(), 1)));
    let show = {
        let list = list.clone();
        let keys = keys.clone();
        move |cx: &mut App| {
            let presentation = list.borrow().presentation("約了王曉明", &keys);
            popup::handle(UiEvent::Popup(Some(PopupState { presentation, caret, inline_preedit: true })), cx);
        }
    };
    let show = Rc::new(show);
    let sink_show = show.clone();
    popup::set_sink(
        Rc::new(move |action, cx| {
            let boundary = {
                let mut list = list.borrow_mut();
                match action {
                    PopupAction::TurnPage(delta) => list.turn_page(i64::from(delta)),
                    PopupAction::MoveSelection(delta) => list.move_selection(i64::from(delta)),
                    PopupAction::ToggleCorrections => {
                        list.toggle_corrections();
                        None
                    }
                    PopupAction::ToggleSingle => {
                        list.toggle_single();
                        None
                    }
                    PopupAction::Pick(_) | PopupAction::TypeReading => None,
                }
            };
            match boundary {
                Some(zt_core::presentation::PreviewIntent::Boundary(direction)) => popup::handle(UiEvent::Boundary(direction), cx),
                _ => sink_show(cx),
            }
        }),
        cx,
    );
    show(cx);
    // The badge stays 1 s: show it late enough for a screenshot taken at 2.5 s.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(1800)).await;
        cx.update(|cx| popup::handle(UiEvent::ModeToast { english: false, anchor: Rect::new(520., 160., 2., 20.) }, cx));
    })
    .detach();
}
