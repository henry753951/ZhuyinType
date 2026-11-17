//! `--panel VARIANT`: one candidate window (or notice, or badge) drawn over a few lines of a document, as the macOS
//! UI renderer's `panels/` show it, for screenshots.  `--popup demo` drives the real floating windows instead.
//!
//! Variants: `vertical-standard`, `vertical-compact`, `vertical-corrections`, the same three `horizontal-…`, `minimal`,
//! `associated` (聯想詞 after a commit), `composing`, `corrected` (a window on a corrected word: the typed reading under
//! the list, the composition dotted under it), `notice-saved`, `notice-select`, `notice-invalid`, `mode-toast`,
//! `quick-settings` (the 快速設定 panel), and the paging frames `vertical-next-35`, `horizontal-previous-20`,
//! `vertical-end-50` … (`next`, `previous` or `end` at a percentage), and the word actions: `word-menu` (a row's
//! right-click menu and the tip) and `word-undo` (the notice with 復原).

use std::rc::Rc;

use gpui_kit::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, point, px, size,
};
use serde_json::Value;
use zt_core::presentation::{CandidateRow, Presentation, PreviewList, Rect};
use zt_core::service::{PopupAction, PopupState, UiEvent};

use super::layout::{Extras, WindowMeasure};
use super::panel::{CandidatePanel, Pinned, mode_badge};
use super::popup;
use super::sample;
use super::style::{self, CurrentKeys, selection_keys};
use crate::app;
use crate::theme::Palette;

struct Showcase {
    presentation: Presentation,
    preview: Option<String>,
    badge: Option<&'static str>,
    panel: CandidatePanel,
    dark: bool,
    sized: bool,
    menu: Option<usize>,
    /// `quick-settings`: the 快速設定 panel instead of a list.
    quick: bool,
}

/// The settings every variant sets (so one variant's choices never leak into the next: screenshots share one
/// settings file).
const SHAPED: &[&str] = &[
    "candidateLayout",
    "candidateDensity",
    "selectionKeys",
    "candidatesPerPage",
    "candidateFontSize",
    "showsListSwitches",
    "showsAnnotations",
];

/// The settings a variant draws with (on the screenshot's temporary settings file).
fn configure(variant: &str, cx: &mut App) {
    for key in SHAPED {
        app::reset_setting(key, cx);
    }
    let mut values: Vec<(&str, Value)> =
        vec![("candidateLayout", if variant.starts_with("horizontal") { "horizontal" } else { "vertical" }.into())];
    values.push(("candidateDensity", if variant.contains("compact") { "compact" } else { "standard" }.into()));
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
        "corrected" => {
            // ㄊㄜˇ typed, 特 (ㄊㄜˋ) shown through the tone correction; the window open on it.
            let mut list = PreviewList::default();
            let entries = ["特", "忒", "慝", "忑"]
                .map(|text| zhuyin_core::api::Candidate { text: text.to_owned(), annotation: None });
            list.present(entries.to_vec(), Some("ㄊㄜˋ".to_owned()), 0);
            let composition = "我特別想你";
            let presentation = Presentation {
                corrected: std::iter::once(1..2).collect(),
                typed_reading: Some("ㄊㄜˇ".to_owned()),
                ..list.presentation(composition, &keys)
            }
            .with_correction_marks(true, true);
            (presentation, Some(composition.to_owned()), None, None)
        }
        "associated" => {
            let rows: Vec<CandidateRow> = ["你", "大家", "你們", "您", "各位"]
                .iter()
                .enumerate()
                .map(|(index, text)| CandidateRow {
                    index,
                    text: (*text).to_owned(),
                    annotation: None,
                    key: keys.get(index).copied(),
                })
                .collect();
            let p = Presentation {
                count: rows.len(),
                rows,
                page_count: 1,
                page_size: per_page,
                associated: true,
                ..Presentation::default()
            };
            (p, None, None, None)
        }
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
        let style = style::style_from_settings(&settings, &CurrentKeys::new(&settings, None));
        let (mut presentation, preview, badge, paging) = presentation_for(variant, cx);
        if variant == "minimal" {
            presentation.reading = None;
        }
        let mut menu = None;
        if variant == "word-menu" {
            presentation.row_actions = vec![0b101; presentation.rows.len()];
            presentation.row_actions[1] = 0b101;
            presentation.tip = Some("Ctrl+Del 不要再推薦・右鍵看更多".to_owned());
            menu = Some(1);
        } else if variant == "word-undo" {
            presentation.notice = Some("不再推薦「小明」".to_owned());
            presentation.undo = true;
        }
        let mut panel = CandidatePanel::new();
        panel.reduced = crate::motion::reduced(cx);
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
        Self { presentation, preview, badge, panel, dark, sized: false, menu, quick: variant == "quick-settings" }
    }
}

impl Render for Showcase {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = if self.dark { Palette::dark() } else { Palette::light() };
        let settings = app::settings(cx).clone();
        let style = style::style_from_settings(&settings, &CurrentKeys::new(&settings, None));
        let (width, height) = match self.badge {
            _ if self.quick => (super::quick::WIDTH, super::quick::height()),
            Some(_) => (super::layout::BADGE_W, super::layout::BADGE_H),
            None if self.panel.pinned.is_some() => self.panel.layout().map_or((0., 0.), |l| (l.width, l.height)),
            None => self.panel.update(
                &self.presentation,
                Extras { preview: self.preview.as_deref(), menu: self.menu },
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
        // An example: 簡體 output, learning paused.
        let switches = zt_core::service::QuickSwitches {
            english: false,
            full_width: true,
            simplified: true,
            learning_paused: true,
        };
        let content = match self.badge {
            _ if self.quick => super::quick::quick_panel(switches, &palette, super::panel::Chrome::Floating, None),
            Some(text) => mode_badge(text, &palette, true),
            None => self.panel.render(&palette, None, window),
        };
        div()
            .relative()
            .size_full()
            .font(crate::theme::font())
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
    let list = Rc::new(std::cell::RefCell::new(sample::list(
        sample::PANEL_WORDS,
        sample::PANEL_CORRECTIONS,
        false,
        keys.len(),
        1,
    )));
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
                    PopupAction::Pick(_)
                    | PopupAction::TypeReading
                    | PopupAction::Word(..)
                    | PopupAction::UndoWordAction => None,
                }
            };
            match boundary {
                Some(zt_core::presentation::PreviewIntent::Boundary(direction)) => {
                    popup::handle(UiEvent::Boundary(direction), cx)
                }
                _ => sink_show(cx),
            }
        }),
        cx,
    );
    show(cx);
    // The badge stays 1 s: show it late enough for a screenshot taken at 2.5 s.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(1800)).await;
        cx.update(|cx| popup::handle(UiEvent::Toast { text: "中", anchor: Rect::new(520., 160., 2., 20.) }, cx));
    })
    .detach();
}
