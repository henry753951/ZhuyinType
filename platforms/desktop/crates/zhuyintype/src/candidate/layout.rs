//! Where everything in the candidate window goes, and how big the window is: a port of macOS
//! `CandidateContentView.update(list:…)` with the same numbers, so the window has the macOS sizes at every font size,
//! density and layout.  Pure geometry over measured text widths; the panel draws what this returns.

use gpui_kit::{FontWeight, Pixels, TextRun, Window, font, px};
use zt_core::presentation::Presentation;

use super::style::{Layout, PanelStyle};
use crate::theme;

/// A rectangle in the window's own coordinates (top left origin, logical pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

/// Measures text in the window's font.
pub trait Measure {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32;
}

/// Measures with the window's text system (shaped lines are cached per frame by GPUI).
pub struct WindowMeasure<'a>(pub &'a Window);

impl Measure for WindowMeasure<'_> {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let mut run_font = font(theme::ui_font());
        run_font.weight = if medium { FontWeight::MEDIUM } else { FontWeight::NORMAL };
        let run = TextRun {
            len: text.len(),
            font: run_font,
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(self.0.text_system().layout_line(text, px(size), &[run], None).width)
    }
}

/// The width of a key's label as `panel::key_glyphs` draws it: ⇧ ↑ ↓ ← → ↩ are icons of `size × 0.95`.
pub fn key_width(measure: &mut dyn Measure, key: &str, size: f32) -> f32 {
    let icon = (size * 0.95).round();
    let mut width = 0.;
    let mut text = String::new();
    for c in key.chars() {
        if matches!(c, '⇧' | '↑' | '↓' | '←' | '→' | '↩') {
            width += icon;
        } else {
            text.push(c);
        }
    }
    width + measure.width(&text, size, false)
}

/// The reading row's font, the footer's and the page label's (macOS: 12 pt, 12 pt, 11 pt).
pub const SMALL_FONT: f32 = 12.;
pub const PAGE_FONT: f32 = 11.;
/// The window's corner radius (macOS `InputSurface.cornerRadius`).
pub const CORNER_RADIUS: f32 = 14.;
/// The key caps of the reading row and the footer.
pub const KEYCAP_HEIGHT: f32 = 19.;
pub const KEYCAP_RADIUS: f32 = 4.;
/// The 中/英 badge (macOS `ModeBadgeView.size`).
pub const BADGE_W: f32 = 52.;
pub const BADGE_H: f32 = 48.;
pub const BADGE_FONT: f32 = 23.;

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

/// The reading row above the list.
#[derive(Clone, Debug, PartialEq)]
pub struct HintLayout {
    pub frame: Frame,
    pub key: String,
    pub hint: String,
    /// Negative without a key cap: the text starts at the left edge (macOS −8.5).
    pub keycap_w: f32,
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
    pub row_radius: f32,
    pub notice: Option<NoticeLayout>,
    /// The composition, underlined, when the app cannot show it inline.
    pub preview: Option<(Frame, String, f32)>,
    pub hint: Option<HintLayout>,
    pub rows_area: Frame,
    pub rows: Vec<RowLayout>,
    pub single: Option<ToggleLayout>,
    pub corrections: Option<ToggleLayout>,
    pub pager: Option<PagerLayout>,
    /// From the window's left edge to the candidates' text (the window lines its text up with the composition).
    pub text_inset: f32,
    pub list_open: bool,
}

impl PanelLayout {
    /// The selected row's frame in the rows' area.
    pub fn selected_frame(&self) -> Option<Frame> {
        self.rows.iter().find(|row| row.selected).map(|row| row.frame)
    }
}

/// What the window shows besides the presentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras<'a> {
    /// The composition, shown in the window when the app cannot show it inline.
    pub preview: Option<&'a str>,
}

fn ceil(value: f32) -> f32 {
    value.ceil()
}

/// Lays the window out for `p` (macOS `CandidateContentView.update`), keeping at least `steady` (the largest size of
/// the list shown, so turning pages never resizes the window).
pub fn compute(
    p: &Presentation,
    extras: Extras<'_>,
    style: &PanelStyle,
    max_width: f32,
    steady: (f32, f32),
    measure: &mut dyn Measure,
) -> PanelLayout {
    let vertical = style.layout == Layout::Vertical;
    let fs = style.font_size;
    let compact = style.compact;
    let keys = &style.keys;
    let row_height = ceil(fs * if compact { 1.32 } else { 1.55 });
    let padding = if compact { 4. } else { 6. };
    let spacing = if compact { 4. } else { 10. };
    let hint_height = if compact { 23. } else { 26. };
    let footer_height = if compact { 23. } else { 25. };
    let list_open = !p.rows.is_empty();
    let preview = extras.preview.filter(|text| !text.is_empty());
    let has_pages = p.page_count > 1 && list_open;
    let readable = p.reading.as_deref().is_some_and(|reading| !reading.is_empty()) && list_open;
    let has_hint = readable && style.shows_zhuyin_hint;
    let has_toggle = p.tolerant.is_some()
        && list_open
        && !keys.corrections.is_empty()
        && (style.shows_list_switches || p.tolerant == Some(true));
    let has_single = readable && !keys.single.is_empty() && (style.shows_list_switches || p.single);
    let has_footer = has_pages || has_toggle || has_single;
    let page_width = if has_footer { 90. } else { 0. };

    let toggle = |key: &str, label: &'static str, on: bool, measure: &mut dyn Measure| {
        let keycap_w = (ceil(key_width(measure, key, SMALL_FONT)) + 9.).max(19.);
        let preferred = keycap_w + 8. + measure.width(label, SMALL_FONT, false);
        (ToggleLayout { frame: Frame::default(), key: key.to_owned(), label, on, keycap_w }, preferred)
    };
    let (mut single, single_pref) = toggle(&keys.single, "單字", p.single, measure);
    let (mut corrections, corrections_pref) = toggle(&keys.corrections, "修正", p.tolerant == Some(true), measure);
    let single_width = if has_single { ceil(single_pref) + 8. } else { 0. };
    let toggle_width = if has_toggle { ceil(corrections_pref) + 8. } else { 0. };
    let footer_width = page_width + single_width + toggle_width;

    // A notice on its own is a compact pill; above candidates it is a row of the window.
    let mut width: f32 = if preview.is_some() || list_open { if compact { 96. } else { 160. } } else { 0. };
    let mut y = padding;
    let notice_height = ceil(fs * 1.25);
    let mut notice = p.notice.as_deref().map(|message| parse_notice(message, (fs * 0.72).round(), measure));
    if let Some(notice) = &notice {
        width = width.max(notice.preferred + 28.);
        y += notice_height + if preview.is_some() || list_open { 3. } else { 0. };
    }
    let preview_y = y;
    let mut preview_width = 0.;
    if let Some(text) = preview {
        preview_width = measure.width(text, fs, false);
        width = width.max(preview_width + 28.);
        y += row_height + if list_open { 5. } else { 0. };
    }
    let hint_y = y;
    if has_hint {
        y += hint_height;
    }

    // The rows at their natural widths.
    let key_inset = if compact { 6. } else { 8. };
    let text_inset = key_inset + ceil(fs * 0.68) + if compact { 5. } else { 8. };
    let trailing_pad = if compact { 7. } else { 10. };
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
            let annotation_w = annotation.map_or(0., |text| measure.width(text, fs * 0.68, false));
            let preferred =
                ceil(text_inset + text_w + if annotation.is_some() { annotation_w + 10. } else { 0. } + trailing_pad);
            Measured { text_w, annotation_w, preferred }
        })
        .collect();
    let count = p.rows.len();
    let gaps = count.saturating_sub(1) as f32 * spacing;
    let natural: f32 = measured.iter().map(|m| m.preferred).sum();
    if vertical {
        width = width.max(measured.iter().map(|m| m.preferred).fold(0., f32::max) + padding * 2.);
    } else {
        width = width.max(natural + gaps + padding * 2. + footer_width);
    }
    width = width.max(footer_width + padding * 2.);
    width = ceil(width.max(steady.0)).min(max_width);

    let notice_frame = Frame::new(14., padding, (width - 28.).max(0.), notice_height);
    if let Some(notice) = &mut notice {
        notice.layout.frame = notice_frame;
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
        (width - padding * 2. - if vertical { 0. } else { footer_width }).max(0.),
        if list_open { if vertical { slots as f32 * row_height } else { row_height } } else { 0. },
    );
    let mut x = 0.;
    let mut row_y = 0.;
    let rows = p
        .rows
        .iter()
        .zip(&measured)
        .enumerate()
        .map(|(index, (row, m))| {
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
            let extra = if annotation.is_some() { m.annotation_w + 10. } else { 0. };
            let needed = m.text_w + extra;
            let avail = (room - trailing_pad.min((room - needed).max(0.))).max(0.);
            let main_width = m.text_w.min((avail - extra).max(0.));
            let annotation_width = (avail - main_width - 10.).max(0.);
            RowLayout {
                frame,
                key: row.key.map(String::from).unwrap_or_default(),
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
    let page_y = if vertical { y + if compact { 1. } else { 3. } } else { y + (row_height - 22.) / 2. };
    if !vertical && list_open {
        y += row_height;
    }
    let text_inset_total = if list_open { padding + text_inset } else { 14. };

    let hint = has_hint.then(|| {
        let hint_text = p.reading.clone().unwrap_or_default();
        let key = keys.zhuyin.clone();
        let keycap_w =
            if key.is_empty() { -8.5 } else { (ceil(key_width(measure, &key, SMALL_FONT)) + 9.).max(19.) };
        let preferred = keycap_w + 9. + measure.width(&hint_text, SMALL_FONT, false);
        HintLayout {
            frame: Frame::new(
                padding + 4.,
                hint_y + if compact { 1. } else { 2. },
                ceil(preferred).min(width - padding * 2. - 8.),
                22.,
            ),
            key,
            hint: hint_text,
            keycap_w,
            clickable: keys.zhuyin_clickable,
        }
    });
    let footer_x = if vertical { padding + 4. } else { width - footer_width };
    let single = (has_footer && has_single).then(|| {
        single.frame = Frame::new(footer_x, page_y, (single_width - 8.).max(0.), 22.);
        single
    });
    let corrections = (has_footer && has_toggle).then(|| {
        corrections.frame = Frame::new(footer_x + single_width, page_y, (toggle_width - 8.).max(0.), 22.);
        corrections
    });
    let pager = has_footer.then(|| PagerLayout {
        label: if p.page_count > 0 { format!("{} / {}", p.page + 1, p.page_count) } else { String::new() },
        label_frame: Frame::new(width - 65., page_y + 3., 40., 16.),
        previous: Frame::new(width - 89., page_y, 22., 22.),
        next: Frame::new(width - 25., page_y, 22., 22.),
        previous_enabled: p.page > 0,
        next_enabled: p.page + 1 < p.page_count,
    });
    if has_footer && vertical {
        y += footer_height;
    }
    let height = (y + padding).max(if list_open { steady.1 } else { 0. });
    PanelLayout {
        width,
        height,
        font_size: fs,
        compact,
        vertical,
        row_height,
        row_radius: if compact { 6. } else { 8. },
        notice: notice.map(|notice| notice.layout),
        preview: preview.map(|text| (Frame::new(14., preview_y, (width - 28.).max(0.), row_height), text.to_owned(), preview_width)),
        hint,
        rows_area,
        rows,
        single,
        corrections,
        pager,
        text_inset: text_inset_total,
        list_open,
    }
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
    let keycap_h = ceil(font_size * 1.45);
    let keycap_w = (ceil(key.as_deref().map_or(0., |key| key_width(measure, key, font_size))) + 9.).max(keycap_h);
    let lead = if key.is_some() {
        keycap_w + 7.
    } else if done {
        font_size + 6.
    } else {
        0.
    };
    let label_w = measure.width(&label, font_size, false);
    let preferred = ceil(lead + label_w + measure.width(&value, font_size, true));
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
        },
        preferred,
    }
}

/// Logical pixels.
pub fn px_of(value: f32) -> Pixels {
    px(value)
}
