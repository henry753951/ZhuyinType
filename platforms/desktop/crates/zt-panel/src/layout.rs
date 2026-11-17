//! Where everything in the candidate window goes, and how big the window is: a port of macOS
//! `CandidateContentView.update(list:…)` with the same numbers, so the window has the macOS sizes at every font size,
//! density and layout.  Pure geometry over measured text widths: the GPUI app measures with its text system and the
//! Windows text service with DirectWrite, and both draw what this returns.

use crate::keys::{self, Piece};
use crate::model::{PanelModel, WORD_ACTIONS};
use crate::style::{Layout, PanelStyle};
use crate::tokens;

/// A rectangle in the window's own coordinates (top left origin, logical pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Whether `(x, y)` is inside.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Measures text in the window's font (`medium`: the medium weight).
pub trait Measure {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32;
}

/// The width of a key's label as it is drawn (`keys::pieces`: its arrows are glyphs of `size × 0.95`, ⇧ is the word
/// 「Shift」).
pub fn key_width(measure: &mut dyn Measure, key: &str, size: f32) -> f32 {
    keys::pieces(key)
        .iter()
        .map(|piece| match piece {
            Piece::Glyph(_) => keys::glyph_size(size),
            Piece::Text(text) => measure.width(text, size, false),
            Piece::Gap => keys::gap(size),
        })
        .sum()
}

/// The geometry below is the macOS window's functional layout (what goes where, how big at each font size);
/// colours, radii and the small text sizes come from `tokens`.
///
/// The reading row's and the footer's labels.
pub const SMALL_FONT: f32 = tokens::LABEL_FONT;
/// The key caps of the reading row, the footer and the notices.
pub const KEYCAP_HEIGHT: f32 = tokens::KEYCAP_H;
/// The 中/英 badge.
pub const BADGE_W: f32 = tokens::BADGE;
pub const BADGE_H: f32 = tokens::BADGE;
pub const BADGE_FONT: f32 = tokens::BADGE_FONT;
/// The clickable box around a footer control or the reading row's key (the key cap sits centred in it).
pub const CONTROL_H: f32 = 22.;
/// A paging button.
const PAGE_BUTTON: f32 = 22.;
/// The page label between the paging buttons (「12/12」 fits).
const PAGE_LABEL: f32 = 36.;
/// Between a key cap and its label.
const KEYCAP_GAP: f32 = 6.;
/// Between the annotation and the candidate.
const ANNOTATION_GAP: f32 = 8.;
/// A notice row's text inset from the window's edge.
const NOTICE_X: f32 = 12.;
/// A notice row above candidates.
const NOTICE_ROW: f32 = 24.;
/// The reading row's text above 聯想詞 (next-word suggestions), which have no reading.
pub const ASSOCIATION_LABEL: &str = "聯想詞・↓ 選擇";

/// A key cap's width for `key` at the key caps' font: the label and 5 on each side, never narrower than tall.
pub fn keycap_width(measure: &mut dyn Measure, key: &str) -> f32 {
    (ceil(key_width(measure, key, tokens::KEYCAP_FONT)) + 10.).max(KEYCAP_HEIGHT)
}

/// The selection keys' column: its text size (`NUMBER_SCALE` of the candidates', at least the micro size) and its
/// fixed width, that of the widest key the column can show (「9」 for digits, 「W」 for letters), so every page and every
/// row puts its candidates at the same x.
pub fn number_column(measure: &mut dyn Measure, font_size: f32, letters: bool) -> (f32, f32) {
    let size = (font_size * tokens::NUMBER_SCALE).round().max(11.);
    (size, ceil(measure.width(if letters { "W" } else { "9" }, size, true)))
}

/// The annotations' text size (the corrections' readings): smaller than the candidates, never under the micro size.
pub fn annotation_font(font_size: f32) -> f32 {
    (font_size * 0.68).round().max(11.)
}

/// One candidate row, in the rows' area of the window.
#[derive(Clone, Debug, PartialEq)]
pub struct RowLayout {
    pub frame: Frame,
    pub key: String,
    pub text: String,
    pub annotation: Option<String>,
    pub selected: bool,
    /// The row's index on the page (what a click picks).
    pub index: usize,
    /// The number column's left edge (row coordinates); its width is `PanelLayout::number_width`.
    pub key_inset: f32,
    pub text_inset: f32,
    /// The width the candidate's own text gets, and the annotation's (both after squeezing).
    pub main_width: f32,
    pub annotation_width: f32,
}

/// 「Enter 加入自訂詞：你好」 → a ↩ key cap, the action and the phrase; 「已加入自訂詞：你好」 → a check, the action and the
/// phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct NoticeLayout {
    pub frame: Frame,
    pub font_size: f32,
    pub key: Option<String>,
    pub done: bool,
    pub label: String,
    /// 「：你好」 or "".
    pub value: String,
    pub keycap_w: f32,
    pub keycap_h: f32,
    pub lead: f32,
    pub label_w: f32,
    /// The 「復原」 link at the row's end (window coordinates), after a word action.
    pub undo: Option<Frame>,
}

/// The link that undoes a word action.
pub const UNDO_LABEL: &str = "復原";

/// The right-click menu of a candidate, drawn inside the window under the list.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuLayout {
    /// The row of the page it acts on.
    pub row: usize,
    /// The hairline above it.
    pub rule: Frame,
    pub items: Vec<MenuItemLayout>,
    pub font_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItemLayout {
    pub frame: Frame,
    /// Index into [`WORD_ACTIONS`] (the action's raw value).
    pub action: usize,
    pub label: &'static str,
    pub enabled: bool,
}

/// A footer switch: its key cap and short name.
#[derive(Clone, Debug, PartialEq)]
pub struct ToggleLayout {
    pub frame: Frame,
    pub key: String,
    pub label: &'static str,
    pub on: bool,
    pub keycap_w: f32,
}

/// The reading row above the list: the reading on the left; at the right end the key that types it, as a key cap
/// named 「注音」 the way the footer names its switches.
#[derive(Clone, Debug, PartialEq)]
pub struct HintLayout {
    /// The whole row (it is clickable when `clickable`).
    pub frame: Frame,
    /// The reading's x in the row.
    pub text_x: f32,
    pub key: String,
    pub hint: String,
    /// 0 without a key cap.
    pub keycap_w: f32,
    /// The key's name after its cap (「注音」) and its width.
    pub key_label: &'static str,
    pub key_label_w: f32,
    /// The key group's right edge's distance from the row's right edge.
    pub keycap_right: f32,
    pub clickable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PagerLayout {
    pub label: String,
    pub label_frame: Frame,
    pub previous: Frame,
    pub next: Frame,
    pub previous_enabled: bool,
    pub next_enabled: bool,
}

/// The whole window.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelLayout {
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub compact: bool,
    pub vertical: bool,
    pub row_height: f32,
    /// The window's corner radius and its padding; the rows' radius is the difference (concentric corners).
    pub radius: f32,
    pub padding: f32,
    pub row_radius: f32,
    /// The selection keys' column: text size and fixed width.  The number is centred on the row, not set on the
    /// candidate's baseline: next to an ideograph (centred on its em box) a digit on the baseline reads as a subscript.
    pub number_font: f32,
    pub number_width: f32,
    pub notice: Option<NoticeLayout>,
    /// The composition, underlined, when the app cannot show it inline.
    pub preview: Option<(Frame, String, f32)>,
    /// The preview's characters shown through an automatic correction: `(x, width)` from the preview text's left
    /// edge, marked with a dotted underline in the accent colour.
    pub preview_marks: Vec<(f32, f32)>,
    pub hint: Option<HintLayout>,
    /// Below the list: 「你打的是 ㄊㄜˇ · ⌫ 還原」 for a span shown through an automatic correction, in the reading
    /// row's type.  The frame is the whole row; the text starts at the rows' key inset.
    pub typed: Option<(Frame, String)>,
    pub rows_area: Frame,
    pub rows: Vec<RowLayout>,
    /// The hairline above the footer (vertical, edge to edge) or before it (horizontal, 1 × 14).
    pub footer_rule: Option<Frame>,
    pub single: Option<ToggleLayout>,
    pub corrections: Option<ToggleLayout>,
    pub pager: Option<PagerLayout>,
    /// From the window's left edge to the candidates' text (the window lines its text up with the composition).
    pub text_inset: f32,
    pub list_open: bool,
    /// The word actions' tip under the list, and its text size.
    pub tip: Option<(Frame, String, f32)>,
    pub menu: Option<MenuLayout>,
    /// The part of `height` that is the menu: a list's steady size leaves it out, so the window shrinks back when
    /// the menu closes.
    pub transient_height: f32,
}

impl PanelLayout {
    /// The selected row's frame in the rows' area.
    pub fn selected_frame(&self) -> Option<Frame> {
        self.rows.iter().find(|row| row.selected).map(|row| row.frame)
    }
}

fn ceil(value: f32) -> f32 {
    value.ceil()
}

/// Lays the window out for `p` (macOS `CandidateContentView.update`), keeping at least `steady` (the largest size of
/// the list shown, so turning pages never resizes the window), never wider than `max_width`.
///
/// A horizontal list too wide for `max_width` first drops the 單字／修正 switches (a list switched on keeps its
/// switch), then shows fewer candidates of the page (always the selected one) rather than squeezing their text; the
/// keys still pick the hidden ones.  Callers with a narrow room (the setup wizard's practice stage) pass its width.
pub fn compute(
    p: &PanelModel,
    style: &PanelStyle,
    max_width: f32,
    steady: (f32, f32),
    measure: &mut dyn Measure,
) -> PanelLayout {
    let (layout, natural) = compute_fitted(p, style, max_width, steady, measure);
    if style.layout == Layout::Horizontal && style.shows_list_switches && natural > max_width {
        let style = PanelStyle { shows_list_switches: false, ..style.clone() };
        return compute_fitted(p, &style, max_width, steady, measure).0;
    }
    layout
}

/// The layout and the width the window would want with no limit.
fn compute_fitted(
    p: &PanelModel,
    style: &PanelStyle,
    max_width: f32,
    steady: (f32, f32),
    measure: &mut dyn Measure,
) -> (PanelLayout, f32) {
    let vertical = style.layout == Layout::Vertical;
    let fs = style.font_size;
    let compact = style.compact;
    let keys = &style.keys;
    let row_height = ceil(fs * if compact { 1.32 } else { 1.55 });
    let padding = tokens::PADDING;
    let spacing = if compact { 8. } else { tokens::HORIZONTAL_GAP };
    let hint_height = if compact { 23. } else { 26. };
    // The vertical footer: a hairline, then the controls' row (the gaps keep 4 px between the hairline and what
    // sits on either side of it, 3 when compact).
    let rule_gap = if compact { 2. } else { 3. };
    let footer_height = rule_gap + 1. + (rule_gap - 1.) + CONTROL_H;
    let list_open = !p.rows.is_empty();
    let preview = p.preview.as_deref().filter(|text| !text.is_empty());
    let has_pages = p.page_count > 1 && list_open;
    let readable = p.reading.as_deref().is_some_and(|reading| !reading.is_empty()) && list_open;
    let has_hint = readable && style.shows_zhuyin_hint;
    let association = p.associated && list_open && !has_hint;
    let typed = p.typed.as_deref().filter(|text| !text.is_empty() && list_open);
    let has_toggle = p.tolerant.is_some()
        && list_open
        && !keys.corrections.is_empty()
        && (style.shows_list_switches || p.tolerant == Some(true));
    let has_single = readable && !keys.single.is_empty() && (style.shows_list_switches || p.single);
    let has_footer = has_pages || has_toggle || has_single;
    // Between the footer's groups (switches, pager).
    let group_gap = 10.;
    // The pager only when there is another page to turn to (one page: no 「1/1」).
    let page_width = if has_pages { PAGE_BUTTON * 2. + PAGE_LABEL } else { 0. };

    let toggle = |key: &str, label: &'static str, on: bool, measure: &mut dyn Measure| {
        let keycap_w = keycap_width(measure, key);
        let preferred = keycap_w + KEYCAP_GAP + measure.width(label, SMALL_FONT, false);
        (ToggleLayout { frame: Frame::default(), key: key.to_owned(), label, on, keycap_w }, preferred)
    };
    let (mut single, single_pref) = toggle(&keys.single, "單字", p.single, measure);
    let (mut corrections, corrections_pref) = toggle(&keys.corrections, "修正", p.tolerant == Some(true), measure);
    let single_width = if has_single { ceil(single_pref) + group_gap } else { 0. };
    let toggle_width = if has_toggle { ceil(corrections_pref) + group_gap } else { 0. };
    // The vertical footer starts 4 in from the padding; the horizontal one after a hairline with a gap each side.
    let footer_lead = if vertical { 4. } else { spacing / 2. + 1. + spacing / 2. };
    let footer_width = if has_footer { footer_lead + single_width + toggle_width + page_width + 2. } else { 0. };

    // The number column, fixed for the list (letters when any key is one).
    let letters = p.rows.iter().any(|row| row.key.is_some_and(|key| !key.is_ascii_digit()));
    let (number_font, number_width) = number_column(measure, fs, letters);
    let key_inset = if compact { 6. } else { 8. };
    let text_inset = key_inset + number_width + if compact { 6. } else { tokens::NUMBER_GAP };
    let trailing_pad = if compact { 8. } else { 10. };

    // A notice on its own is a small window of its own height; above candidates it is a row of the window.
    let mut width: f32 = if preview.is_some() || list_open { if compact { 96. } else { 160. } } else { 0. };
    let mut y = padding;
    let alone = preview.is_none() && !list_open;
    let notice_height = if alone { tokens::NOTICE_H - padding * 2. } else { NOTICE_ROW };
    let mut notice = p.notice.as_deref().map(|message| parse_notice(message, (fs * 0.72).round(), measure));
    if let Some(notice) = &notice {
        width = width.max(notice.preferred + NOTICE_X * 2.);
        y += notice_height + if alone { 0. } else { 2. };
    }
    // The tip and the menu go under the list; they only widen the window here.
    let tip_font = SMALL_FONT;
    let tip = p.tip.as_deref().filter(|_| list_open).map(|text| {
        width = width.max(ceil(measure.width(text, tip_font, false)) + padding * 2. + key_inset * 2.);
        text.to_owned()
    });
    let menu_font = (fs * 0.8).round().max(12.);
    let menu_row = p.menu.filter(|row| *row < p.rows.len() && list_open);
    if menu_row.is_some() {
        let widest = WORD_ACTIONS.iter().map(|(_, title)| measure.width(title, menu_font, false)).fold(0., f32::max);
        width = width.max(ceil(widest) + padding * 2. + key_inset * 2.);
    }
    let undo_w = if p.undo { ceil(measure.width(UNDO_LABEL, (fs * 0.72).round(), true)) } else { 0. };
    if let Some(notice) = &notice
        && p.undo
    {
        width = width.max(notice.preferred + NOTICE_X * 2. + 12. + undo_w);
    }
    let preview_y = y;
    let mut preview_width = 0.;
    if let Some(text) = preview {
        preview_width = measure.width(text, fs, false);
        width = width.max(preview_width + 28.);
        y += row_height + if list_open { 5. } else { 0. };
    }
    let hint_y = y;
    let hint_key = if has_hint { keys.zhuyin.clone() } else { String::new() };
    let hint_keycap_w = if hint_key.is_empty() { 0. } else { keycap_width(measure, &hint_key) };
    let hint_keycap_right = key_inset;
    let hint_label = "注音";
    let hint_label_w = if hint_key.is_empty() { 0. } else { ceil(measure.width(hint_label, SMALL_FONT, false)) };
    if has_hint {
        // Measured in the UI font while drawn in the CJK one: a little room for the tone marks.
        let reading_w = measure.width(p.reading.as_deref().unwrap_or_default(), SMALL_FONT, false) + 4.;
        let key_room =
            if hint_key.is_empty() { 0. } else { 16. + hint_keycap_w + KEYCAP_GAP + hint_label_w + hint_keycap_right };
        width = width.max(ceil(padding * 2. + key_inset + reading_w + key_room.max(key_inset)));
        y += hint_height;
    } else if association {
        let label_w = measure.width(ASSOCIATION_LABEL, SMALL_FONT, false) + 4.;
        width = width.max(ceil(padding * 2. + key_inset * 2. + label_w));
        y += hint_height;
    }
    if let Some(text) = typed {
        let typed_w = measure.width(text, SMALL_FONT, false) + 4.;
        width = width.max(ceil(padding * 2. + key_inset * 2. + typed_w));
    }

    // The rows at their natural widths.
    struct Measured {
        text_w: f32,
        annotation_w: f32,
        preferred: f32,
    }
    let measured: Vec<Measured> = p
        .rows
        .iter()
        .map(|row| {
            let annotation = row.annotation.as_deref().filter(|_| style.shows_annotations);
            let text_w = measure.width(&row.text, fs, false);
            let annotation_w = annotation.map_or(0., |text| measure.width(text, annotation_font(fs), false));
            let preferred = ceil(
                text_inset
                    + text_w
                    + if annotation.is_some() { annotation_w + ANNOTATION_GAP } else { 0. }
                    + trailing_pad,
            );
            Measured { text_w, annotation_w, preferred }
        })
        .collect();
    // The rows shown: all of them, or (a horizontal list too wide for `max_width`) as many as fit at their natural
    // widths, from the start of the page or ending at the selected one.
    let (shown_start, shown_end) = if vertical {
        (0, p.rows.len())
    } else {
        let room = max_width - padding * 2. - footer_width;
        let fits = |range: std::ops::Range<usize>| {
            let rows = &measured[range];
            rows.iter().map(|m| m.preferred).sum::<f32>() + rows.len().saturating_sub(1) as f32 * spacing <= room
        };
        let mut end = p.rows.len();
        while end > 1 && !fits(0..end) {
            end -= 1;
        }
        let selected = p.selected.unwrap_or(0).min(p.rows.len().saturating_sub(1));
        if end < p.rows.len() && selected >= end {
            let mut start = selected + 1 - end.max(1);
            while start < selected && !fits(start..selected + 1) {
                start += 1;
            }
            (start, selected + 1)
        } else {
            (0, end)
        }
    };
    let all_natural: f32 = measured.iter().map(|m| m.preferred).sum();
    let measured = &measured[shown_start..shown_end];
    let count = measured.len();
    let gaps = count.saturating_sub(1) as f32 * spacing;
    let natural: f32 = measured.iter().map(|m| m.preferred).sum();
    let wanted = if vertical {
        width.max(measured.iter().map(|m| m.preferred).fold(0., f32::max) + padding * 2.)
    } else {
        width.max(all_natural + p.rows.len().saturating_sub(1) as f32 * spacing + padding * 2. + footer_width)
    };
    if vertical {
        width = wanted;
    } else {
        width = width.max(natural + gaps + padding * 2. + footer_width);
    }
    width = width.max(footer_width + padding * 2.);
    let wanted = ceil(wanted.max(footer_width + padding * 2.));
    width = ceil(width.max(steady.0)).min(max_width);

    let notice_frame = Frame::new(NOTICE_X, padding, (width - NOTICE_X * 2.).max(0.), notice_height);
    if let Some(notice) = &mut notice {
        notice.layout.frame = notice_frame;
        if p.undo {
            notice.layout.frame.w = (notice_frame.w - undo_w - 12.).max(0.);
            notice.layout.undo =
                Some(Frame::new(notice_frame.x + notice_frame.w - undo_w, notice_frame.y, undo_w, notice_frame.h));
        }
    }
    // A horizontal list too wide for its room gives up the gaps between candidates before it narrows them.
    let rows_room = (width - padding * 2. - if vertical { 0. } else { footer_width }).max(0.);
    let fit_spacing = if !vertical && count > 1 {
        spacing.min(((rows_room - natural) / (count - 1) as f32).max(0.))
    } else {
        spacing
    };
    let available = (rows_room - count.saturating_sub(1) as f32 * fit_spacing).max(0.);
    let scale = (available / natural.max(1.)).min(1.);
    // A vertical list with pages keeps room for a full page, so the last page is the same height.
    let slots = if vertical && has_pages { count.max(p.page_size) } else { count };
    let rows_top = y;
    let rows_area = Frame::new(
        padding,
        rows_top,
        rows_room,
        if list_open { if vertical { slots as f32 * row_height } else { row_height } } else { 0. },
    );
    let mut x = 0.;
    let mut row_y = 0.;
    let rows = p
        .rows
        .iter()
        .enumerate()
        .skip(shown_start)
        .zip(measured)
        .map(|((index, row), m)| {
            let row_width = if vertical { width - padding * 2. } else { (m.preferred * scale).floor() };
            let frame = Frame::new(x, row_y, row_width.max(0.), row_height);
            if vertical {
                row_y += row_height;
            } else {
                x += row_width + fit_spacing;
            }
            let annotation = row.annotation.clone().filter(|_| style.shows_annotations);
            // A squeezed row gives up its trailing pad before its text.
            let room = (frame.w - text_inset).max(0.);
            let extra = if annotation.is_some() { m.annotation_w + ANNOTATION_GAP } else { 0. };
            let needed = m.text_w + extra;
            let avail = (room - trailing_pad.min((room - needed).max(0.))).max(0.);
            let main_width = m.text_w.min((avail - extra).max(0.));
            let annotation_width = (avail - main_width - ANNOTATION_GAP).max(0.);
            RowLayout {
                frame,
                key: row.key.map(|key| key.to_ascii_uppercase().to_string()).unwrap_or_default(),
                text: row.text.clone(),
                annotation,
                selected: p.selected == Some(index),
                index,
                key_inset,
                text_inset,
                main_width,
                annotation_width,
            }
        })
        .collect::<Vec<_>>();
    if vertical {
        y += rows_area.h;
    }
    // The typed reading under the list (vertical: above the footer); a horizontal list's footer stays on the row.
    let typed_y = if vertical { y } else { y + row_height };
    if vertical && typed.is_some() {
        y += hint_height;
    }
    let page_y = if vertical { y + footer_height - CONTROL_H } else { y + (row_height - CONTROL_H) / 2. };
    let footer_rule = has_footer.then(|| {
        if vertical {
            Frame::new(0., y + rule_gap, width, 1.)
        } else {
            // Right before the footer, however wide a steady window left the rows' room.
            let x = width - padding - footer_width + spacing / 2.;
            Frame::new(x.round(), rows_top + ((row_height - tokens::DIVIDER_H) / 2.).round(), 1., tokens::DIVIDER_H)
        }
    });
    if !vertical && list_open {
        y += row_height;
        if typed.is_some() {
            y += hint_height;
        }
    }
    let text_inset_total = if list_open { padding + text_inset } else { NOTICE_X };

    let hint = has_hint.then(|| HintLayout {
        frame: Frame::new(padding, hint_y, width - padding * 2., hint_height - 2.),
        text_x: key_inset,
        key: hint_key.clone(),
        hint: p.reading.clone().unwrap_or_default(),
        keycap_w: hint_keycap_w,
        key_label: hint_label,
        key_label_w: hint_label_w,
        keycap_right: hint_keycap_right,
        clickable: keys.zhuyin_clickable,
    });
    // 聯想詞 take the reading row's place with a quiet label and no key.
    let hint = hint.or_else(|| {
        association.then(|| HintLayout {
            frame: Frame::new(padding, hint_y, width - padding * 2., hint_height - 2.),
            text_x: key_inset,
            key: String::new(),
            hint: ASSOCIATION_LABEL.to_owned(),
            keycap_w: 0.,
            key_label: "",
            key_label_w: 0.,
            keycap_right: hint_keycap_right,
            clickable: false,
        })
    });
    let footer_x = if vertical { padding + footer_lead } else { width - padding - footer_width + footer_lead };
    let single = (has_footer && has_single).then(|| {
        single.frame = Frame::new(footer_x, page_y, (single_width - group_gap).max(0.), CONTROL_H);
        single
    });
    let corrections = (has_footer && has_toggle).then(|| {
        corrections.frame = Frame::new(footer_x + single_width, page_y, (toggle_width - group_gap).max(0.), CONTROL_H);
        corrections
    });
    let next_x = width - padding - 2. - PAGE_BUTTON;
    let pager = has_pages.then(|| PagerLayout {
        label: if p.page_count > 0 { format!("{}/{}", p.page + 1, p.page_count) } else { String::new() },
        label_frame: Frame::new(next_x - PAGE_LABEL, page_y + 3., PAGE_LABEL, 16.),
        previous: Frame::new(next_x - PAGE_LABEL - PAGE_BUTTON, page_y, PAGE_BUTTON, PAGE_BUTTON),
        next: Frame::new(next_x, page_y, PAGE_BUTTON, PAGE_BUTTON),
        previous_enabled: p.page > 0,
        next_enabled: p.page + 1 < p.page_count,
    });
    if has_footer && vertical {
        y += footer_height;
    }
    let steady_height = if list_open { steady.1 } else { 0. };
    // The tip at the bottom of the list's steady size (which includes it), so it stays put as pages turn.
    const TIP_H: f32 = 18.;
    let tip = tip.map(|text| {
        let tip_y = y.max(steady_height - padding - TIP_H);
        y = tip_y + TIP_H;
        (Frame::new(padding + key_inset, tip_y, (width - padding * 2. - key_inset * 2.).max(0.), TIP_H), text, tip_font)
    });
    let mut height = (y + padding).max(steady_height);
    let mut below = height - padding;
    let base_height = height;
    let menu = menu_row.map(|row| {
        let rule = Frame::new(0., below + 3., width, 1.);
        below += 7.;
        let item_h = ceil(menu_font * 1.8);
        let bits = p.rows[row].actions;
        let items = WORD_ACTIONS
            .iter()
            .enumerate()
            .map(|(action, (_, label))| {
                let frame = Frame::new(padding, below + action as f32 * item_h, width - padding * 2., item_h);
                MenuItemLayout { frame, action, label, enabled: bits & (1 << action) != 0 }
            })
            .collect::<Vec<_>>();
        below += item_h * items.len() as f32;
        height = height.max(below + padding);
        MenuLayout { row, rule, items, font_size: menu_font }
    });
    let transient_height = height - base_height;
    let layout = PanelLayout {
        width,
        height,
        font_size: fs,
        compact,
        vertical,
        row_height,
        radius: tokens::RADIUS,
        padding,
        row_radius: tokens::ROW_RADIUS,
        number_font,
        number_width,
        notice: notice.map(|notice| notice.layout),
        preview: preview.map(|text| {
            (Frame::new(14., preview_y, (width - 28.).max(0.), row_height), text.to_owned(), preview_width)
        }),
        preview_marks: preview.map_or_else(Vec::new, |text| marks(text, &p.preview_corrected, fs, measure)),
        hint,
        typed: typed
            .map(|text| (Frame::new(padding, typed_y, width - padding * 2., hint_height - 2.), text.to_owned())),
        rows_area,
        rows,
        footer_rule,
        single,
        corrections,
        pager,
        text_inset: text_inset_total,
        list_open,
        tip,
        menu,
        transient_height,
    };
    (layout, wanted)
}

/// `(x, width)` of each UTF-16 range of `text` (ranges that do not fit it are left out).
fn marks(text: &str, ranges: &[std::ops::Range<usize>], font_size: f32, measure: &mut dyn Measure) -> Vec<(f32, f32)> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let prefix = |end: usize| String::from_utf16(&units[..end]).ok();
    ranges
        .iter()
        .filter(|range| range.start < range.end && range.end <= units.len())
        .filter_map(|range| {
            let (before, through) = (prefix(range.start)?, prefix(range.end)?);
            let x = measure.width(&before, font_size, false);
            Some((x, measure.width(&through, font_size, false) - x))
        })
        .collect()
}

struct ParsedNotice {
    layout: NoticeLayout,
    preferred: f32,
}

fn parse_notice(message: &str, font_size: f32, measure: &mut dyn Measure) -> ParsedNotice {
    let (key, rest) = match message.strip_prefix("Enter ") {
        Some(rest) => (Some("↩".to_owned()), rest),
        None => (None, message),
    };
    let done = rest.starts_with('已');
    let (label, value) = match rest.split_once('：') {
        Some((label, value)) => (label.to_owned(), value.to_owned()),
        None => (rest.to_owned(), String::new()),
    };
    let value = if value.is_empty() { value } else { format!("：{value}") };
    let keycap_h = KEYCAP_HEIGHT;
    let keycap_w = key.as_deref().map_or(0., |key| keycap_width(measure, key));
    let lead = if key.is_some() {
        keycap_w + KEYCAP_GAP + 2.
    } else if done {
        tokens::NOTICE_ICON + KEYCAP_GAP
    } else {
        0.
    };
    let label_w = measure.width(&label, font_size, false);
    let preferred = ceil(lead + label_w + measure.width(&value, font_size, false));
    ParsedNotice {
        layout: NoticeLayout {
            frame: Frame::default(),
            font_size,
            key,
            done,
            label,
            value,
            keycap_w,
            keycap_h,
            lead,
            label_w,
            undo: None,
        },
        preferred,
    }
}

/// Text widths estimated from the font size (CJK one em, Latin a little over half): enough for heights, which do
/// not depend on widths, where no window is at hand to measure with.
pub struct Estimate;

impl Measure for Estimate {
    fn width(&mut self, text: &str, size: f32, _: bool) -> f32 {
        text.chars().map(|c| if c.is_ascii() { size * 0.58 } else { size }).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PanelModel, Row};
    use crate::style::PanelStyle;

    /// Every character as wide as the font is tall (CJK), Latin half that.
    struct Fixed;

    impl Measure for Fixed {
        fn width(&mut self, text: &str, size: f32, _: bool) -> f32 {
            text.chars().map(|c| if c.is_ascii() { size / 2. } else { size }).sum()
        }
    }

    const WORDS: &[&str] = &[
        "曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "洺", "螟蛉", "明明",
        "名命",
    ];

    /// Page `page` of the example list, `page_size` a page, the second row highlighted on the first page.
    fn page(page_size: usize, page: usize, keys: &str) -> PanelModel {
        let keys: Vec<char> = keys.chars().collect();
        let start = page * page_size;
        let rows = WORDS[start..(start + page_size).min(WORDS.len())]
            .iter()
            .enumerate()
            .map(|(row, text)| Row { key: keys.get(row).copied(), text: (*text).to_owned(), ..Row::default() })
            .collect();
        PanelModel {
            rows,
            count: WORDS.len(),
            page,
            page_count: WORDS.len().div_ceil(page_size),
            page_size,
            selected: Some(1),
            reading: Some("ㄒㄧㄠˇ ㄇㄧㄥˊ".to_owned()),
            tolerant: Some(false),
            ..PanelModel::default()
        }
    }

    fn list(page_size: usize) -> PanelModel {
        page(page_size, 0, "123456789")
    }

    #[test]
    fn vertical_window_has_the_macos_geometry() {
        let style = PanelStyle::default();
        let layout = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        // Rows 28 tall (18 × 1.55), padding 4, the reading row 26, the footer 28 (hairline and controls).
        assert_eq!(layout.row_height, 28.);
        assert_eq!(layout.rows.len(), 9);
        assert_eq!(layout.rows_area.y, 4. + 26.);
        assert_eq!(layout.height, 4. + 26. + 9. * 28. + 28. + 4.);
        // The text starts at 4 + 8 + the number column (「9」 at 11 px: 6) + 8.
        assert_eq!(layout.text_inset, 4. + 8. + 6. + 8.);
        assert!(layout.pager.as_ref().is_some_and(|pager| pager.label == "1/2" && !pager.previous_enabled));
        assert_eq!(layout.selected_frame().map(|frame| frame.y), Some(28.));
    }

    #[test]
    fn last_page_keeps_the_full_page_height_and_steady_width() {
        let style = PanelStyle::default();
        let first = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        let last = compute(&page(9, 1, "123456789"), &style, 4000., (first.width, first.height), &mut Fixed);
        assert_eq!(last.rows.len(), 8);
        assert_eq!((last.width, last.height), (first.width, first.height));
    }

    #[test]
    fn typed_reading_sits_under_the_list_and_preview_marks_the_corrections() {
        let style = PanelStyle::default();
        let plain = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        let p = PanelModel {
            typed: Some("你打的是 ㄊㄜˇ · ⌫ 還原".into()),
            preview: Some("你特別".into()),
            preview_corrected: std::iter::once(1..2).collect(),
            ..list(9)
        };
        let layout = compute(&p, &style, 4000., (0., 0.), &mut Fixed);
        let (frame, text) = layout.typed.clone().expect("a typed-reading row");
        assert_eq!(text, "你打的是 ㄊㄜˇ · ⌫ 還原");
        assert_eq!(frame.y, layout.rows_area.y + layout.rows_area.h);
        assert!(layout.footer_rule.is_some_and(|rule| rule.y > frame.y));
        let (_, _, preview_w) = layout.preview.clone().expect("a preview");
        assert_eq!(layout.height, plain.height + 26. + layout.row_height + 5.);
        let [(x, w)] = layout.preview_marks[..] else { panic!("one mark") };
        assert!(x > 0. && w > 0. && x + w < preview_w);
        // Ranges outside the text are left out; no typed reading without a list.
        let p = PanelModel {
            preview: Some("你".into()),
            preview_corrected: std::iter::once(0..3).collect(),
            typed: Some("x".into()),
            ..PanelModel::default()
        };
        let layout = compute(&p, &style, 4000., (0., 0.), &mut Fixed);
        assert!(layout.preview_marks.is_empty() && layout.typed.is_none());
    }

    #[test]
    fn notice_alone_is_a_small_window() {
        let p = PanelModel { notice: Some("Enter 加入自訂詞：王小明".into()), ..PanelModel::default() };
        let layout = compute(&p, &PanelStyle::default(), 4000., (0., 0.), &mut Fixed);
        let notice = layout.notice.expect("a notice");
        assert_eq!(notice.key.as_deref(), Some("↩"));
        assert_eq!(notice.label, "加入自訂詞");
        assert_eq!(notice.value, "：王小明");
        assert_eq!(layout.height, tokens::NOTICE_H);
        assert_eq!(notice.keycap_h, tokens::KEYCAP_H);
        assert!(layout.rows.is_empty() && layout.pager.is_none() && layout.footer_rule.is_none());
    }

    #[test]
    fn corners_are_concentric() {
        for compact in [false, true] {
            for layout_kind in [Layout::Vertical, Layout::Horizontal] {
                let style = PanelStyle { compact, layout: layout_kind, ..PanelStyle::default() };
                let layout = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
                // The selection sits `padding` in from the window's edge, so its radius is the window's minus that.
                assert_eq!(layout.radius, 10.);
                assert_eq!(layout.padding, 4.);
                assert_eq!(layout.row_radius, layout.radius - layout.padding);
                assert_eq!(layout.rows_area.x, layout.padding);
                if layout_kind == Layout::Vertical {
                    assert_eq!(layout.rows_area.x + layout.rows_area.w, layout.width - layout.padding);
                }
            }
        }
    }

    #[test]
    fn number_column_has_a_fixed_width() {
        // Digits: the width of 「9」; letters: the width of 「W」, at round(fs × 0.62), never under 11.
        assert_eq!(number_column(&mut Fixed, 18., false), (11., 6.));
        assert_eq!(number_column(&mut Fixed, 13., false), (11., 6.));
        assert_eq!(number_column(&mut Fixed, 28., true), (17., 9.));
        let style = PanelStyle::default();
        let digits = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        assert!(digits.rows.iter().all(|row| row.text_inset == digits.rows[0].text_inset));
        assert_eq!(digits.rows[0].text_inset - digits.rows[0].key_inset, digits.number_width + tokens::NUMBER_GAP);
        // Letter keys are shown as capitals and take the wider column.
        let letters = page(5, 0, "asdfg");
        let letters = compute(&letters, &style, 4000., (0., 0.), &mut Fixed);
        assert_eq!(letters.rows[0].key, "A");
        assert!(letters.number_width >= digits.number_width);
    }

    #[test]
    fn footer_hairlines() {
        let vertical = compute(&list(9), &PanelStyle::default(), 4000., (0., 0.), &mut Fixed);
        let rule = vertical.footer_rule.expect("a hairline above the footer");
        // Edge to edge, between the rows and the footer's controls.
        assert_eq!((rule.x, rule.w, rule.h), (0., vertical.width, 1.));
        assert!(rule.y > vertical.rows_area.y + vertical.rows_area.h);
        assert!(vertical.pager.as_ref().is_some_and(|pager| pager.next.y >= rule.y + 1.));

        let style = PanelStyle { layout: Layout::Horizontal, ..PanelStyle::default() };
        let horizontal = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        let rule = horizontal.footer_rule.expect("a hairline before the footer");
        let last = horizontal.rows.last().expect("rows").frame;
        // 1 × 14, centred on the row, between the last candidate and the first footer control.
        assert_eq!((rule.w, rule.h), (1., 14.));
        assert!(rule.x > horizontal.rows_area.x + last.x + last.w);
        let first_control = horizontal.single.as_ref().map_or(f32::MAX, |single| single.frame.x);
        assert!(rule.x + 1. < first_control);
        // Candidates 14 apart.
        assert_eq!(horizontal.rows[1].frame.x - (horizontal.rows[0].frame.x + horizontal.rows[0].frame.w), 14.);
    }

    #[test]
    fn reading_row_puts_the_key_at_its_end() {
        let layout = compute(&list(9), &PanelStyle::default(), 4000., (0., 0.), &mut Fixed);
        let hint = layout.hint.expect("a reading row");
        assert_eq!(hint.key, "`");
        assert_eq!(hint.frame.x, layout.padding);
        assert_eq!(hint.frame.w, layout.width - layout.padding * 2.);
        assert_eq!(hint.text_x, layout.rows[0].key_inset);
        assert_eq!(hint.key_label, "注音");
        // The key group ends as far from the right edge as the reading starts from the left.
        assert_eq!(hint.keycap_right, hint.text_x);
        let mut style = PanelStyle::default();
        style.keys.zhuyin.clear();
        let layout = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        assert_eq!(layout.hint.expect("a reading row").keycap_w, 0.);
    }

    #[test]
    fn associated_list_is_labelled() {
        let mut p = list(9);
        p.reading = None;
        p.associated = true;
        let layout = compute(&p, &PanelStyle::default(), 4000., (0., 0.), &mut Fixed);
        let hint = layout.hint.expect("the 聯想詞 label");
        assert_eq!((hint.hint.as_str(), hint.keycap_w, hint.clickable), (ASSOCIATION_LABEL, 0., false));
        assert_eq!(layout.rows_area.y, hint.frame.y + hint.frame.h + 2.);
        p.associated = false;
        assert!(compute(&p, &PanelStyle::default(), 4000., (0., 0.), &mut Fixed).hint.is_none());
    }

    #[test]
    fn narrow_horizontal_list_drops_switches_then_candidates() {
        let style = PanelStyle { layout: Layout::Horizontal, ..PanelStyle::default() };
        let full = compute(&list(9), &style, 4000., (0., 0.), &mut Fixed);
        assert!(full.single.is_some() && full.rows.len() == 9);
        // A little narrower: the switches go, every candidate stays at its natural width.
        let switches = full.single.as_ref().map_or(0., |single| single.frame.w)
            + full.corrections.as_ref().map_or(0., |toggle| toggle.frame.w);
        let narrower = compute(&list(9), &style, full.width - 10., (0., 0.), &mut Fixed);
        assert!(narrower.single.is_none() && narrower.corrections.is_none());
        assert!(switches > 10. && narrower.rows.len() == 9 && narrower.width <= full.width - 10.);
        // Much narrower: fewer candidates, never squeezed, the selected one (the second) among them.
        let narrow = compute(&list(9), &style, 300., (0., 0.), &mut Fixed);
        assert!(narrow.width <= 300.);
        assert!(narrow.rows.len() < 9 && narrow.rows.iter().any(|row| row.selected));
        assert!(narrow.rows.iter().all(|row| row.main_width >= Fixed.width(&row.text, 18., false)));
        // The rows keep their page indices (what a click picks).
        assert_eq!(narrow.rows[0].index, 0);
    }

    #[test]
    fn word_actions_tip_menu_and_undo() {
        let style = PanelStyle::default();
        let plain = compute(&list(5), &style, 4000., (0., 0.), &mut Fixed);
        let mut p = list(5);
        p.tip = Some("Ctrl+Del 不要再推薦・右鍵看更多".into());
        let tipped = compute(&p, &style, 4000., (0., 0.), &mut Fixed);
        let (frame, ..) = tipped.tip.clone().expect("a tip under the list");
        assert!(frame.y >= plain.height - plain.padding - 0.5, "under everything else");
        assert_eq!(tipped.transient_height, 0., "the tip is part of the list's size");
        p.rows[1].actions = 0b011;
        p.menu = Some(1);
        let menu = compute(&p, &style, 4000., (tipped.width, tipped.height), &mut Fixed);
        let items = &menu.menu.as_ref().expect("the menu").items;
        assert_eq!(items.iter().map(|item| item.enabled).collect::<Vec<_>>(), [true, true, false]);
        assert!(menu.transient_height > 0.);
        assert!((menu.height - menu.transient_height - tipped.height).abs() < 0.5, "shrinks back when it closes");
        let mut notice = list(5);
        notice.notice = Some("不再推薦「小明」".into());
        notice.undo = true;
        let undo = compute(&notice, &style, 4000., (0., 0.), &mut Fixed).notice.expect("a notice");
        let link = undo.undo.expect("復原");
        assert!(link.x >= undo.frame.x + undo.frame.w, "after the notice's text");
    }
}
