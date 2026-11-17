//! Drawing the candidate window with Direct2D and DirectWrite: the same geometry (`zt_panel::layout`) and colours
//! (`zt_panel::tokens`) as the app's own window, so a list looks the same in every application.
//!
//! Text is Inter (shipped beside the DLL, `fonts\`) for Latin and digits over the system's CJK font, as in the app;
//! without the font files it is Segoe UI.  Zhuyin readings use the CJK font first, so the tone marks match the letters.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_ROUND, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_LINE_JOIN_ROUND, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory5,
    IDWriteFontCollection1, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::core::{HSTRING, w};
use windows_numerics::Vector2;
use zt_panel::keys::{self, Glyph, Piece};
use zt_panel::layout::{
    self, CONTROL_H, Frame, HintLayout, KEYCAP_HEIGHT, Measure, NoticeLayout, PanelLayout, RowLayout, SMALL_FONT,
    ToggleLayout,
};
use zt_panel::tokens::{self, Colors, Rgba};

use crate::module::module;

/// Which family a run of text is set in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Family {
    /// Inter (or Segoe UI) with the system's CJK fallback.
    Ui,
    /// The CJK font first: Zhuyin readings.
    Zhuyin,
}

/// The factories, the bundled fonts and the text formats, once per thread.
pub(crate) struct Graphics {
    pub(crate) d2d: ID2D1Factory,
    dwrite: IDWriteFactory5,
    fonts: Option<IDWriteFontCollection1>,
    round: ID2D1StrokeStyle,
    formats: RefCell<HashMap<(u32, i32, Family), IDWriteTextFormat>>,
}

thread_local! {
    static GRAPHICS: RefCell<Option<Rc<Graphics>>> = const { RefCell::new(None) };
}

/// This thread's graphics, made on first use; None when Direct2D or DirectWrite is unavailable.
pub(crate) fn graphics() -> Option<Rc<Graphics>> {
    GRAPHICS.with(|cell| {
        if cell.borrow().is_none() {
            *cell.borrow_mut() = Graphics::new().map(Rc::new);
        }
        cell.borrow().clone()
    })
}

/// The directory this DLL was loaded from.
fn module_directory() -> Option<PathBuf> {
    let mut buffer = vec![0u16; 1024];
    // SAFETY: our own module handle and a buffer of the given length.
    let length = unsafe { GetModuleFileNameW(Some(module()), &mut buffer) } as usize;
    (length > 0 && length < buffer.len())
        .then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length])).parent().map(PathBuf::from))
        .flatten()
}

impl Graphics {
    fn new() -> Option<Self> {
        // SAFETY: plain factory creation; the objects are used on this thread only.
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
            let dwrite: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
            let round = d2d
                .CreateStrokeStyle(
                    &D2D1_STROKE_STYLE_PROPERTIES {
                        startCap: D2D1_CAP_STYLE_ROUND,
                        endCap: D2D1_CAP_STYLE_ROUND,
                        lineJoin: D2D1_LINE_JOIN_ROUND,
                        ..Default::default()
                    },
                    None,
                )
                .ok()?;
            let fonts = Self::bundled_fonts(&dwrite);
            Some(Self { d2d, dwrite, fonts, round, formats: RefCell::new(HashMap::new()) })
        }
    }

    /// Inter from `fonts\` beside the DLL.
    fn bundled_fonts(dwrite: &IDWriteFactory5) -> Option<IDWriteFontCollection1> {
        let directory = module_directory()?.join("fonts");
        // SAFETY: DirectWrite calls with valid arguments.
        unsafe {
            let builder = dwrite.CreateFontSetBuilder().ok()?;
            let mut added = 0;
            for name in ["Inter-Regular.ttf", "Inter-Medium.ttf", "Inter-SemiBold.ttf"] {
                let path = HSTRING::from(directory.join(name).as_os_str());
                if let Ok(file) = dwrite.CreateFontFileReference(&path, None)
                    && builder.AddFontFile(&file).is_ok()
                {
                    added += 1;
                }
            }
            if added == 0 {
                return None;
            }
            let set = builder.CreateFontSet().ok()?;
            dwrite.CreateFontCollectionFromFontSet(&set).ok()
        }
    }

    fn format(&self, size: f32, weight: DWRITE_FONT_WEIGHT, family: Family) -> Option<IDWriteTextFormat> {
        let key = (size.to_bits(), weight.0, family);
        if let Some(format) = self.formats.borrow().get(&key) {
            return Some(format.clone());
        }
        // SAFETY: DirectWrite calls with valid arguments.
        let format = unsafe {
            let (name, collection) = match (family, &self.fonts) {
                (Family::Ui, Some(fonts)) => (w!("Inter"), Some(fonts.clone().into())),
                (Family::Ui, None) => (w!("Segoe UI"), None),
                (Family::Zhuyin, _) => (w!("Microsoft JhengHei UI"), None),
            };
            let collection: Option<windows::Win32::Graphics::DirectWrite::IDWriteFontCollection> = collection;
            let format = self
                .dwrite
                .CreateTextFormat(
                    name,
                    collection.as_ref(),
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("zh-TW"),
                )
                .ok()?;
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            if let Ok(sign) = self.dwrite.CreateEllipsisTrimmingSign(&format) {
                let trimming =
                    DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, ..Default::default() };
                let _ = format.SetTrimming(&trimming, &sign);
            }
            format
        };
        self.formats.borrow_mut().insert(key, format.clone());
        Some(format)
    }

    /// `text` laid out on one line, at most `max_width` wide (longer: ends in an ellipsis).
    pub(crate) fn layout(
        &self,
        text: &str,
        size: f32,
        medium: bool,
        family: Family,
        max_width: f32,
    ) -> Option<IDWriteTextLayout> {
        let weight = if medium { DWRITE_FONT_WEIGHT_MEDIUM } else { DWRITE_FONT_WEIGHT_NORMAL };
        let format = self.format(size, weight, family)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: a valid format and string.
        unsafe { self.dwrite.CreateTextLayout(&units, &format, max_width.max(0.), 10_000.).ok() }
    }

    /// The size of a laid-out line (width with trailing spaces, height of the line).
    pub(crate) fn extent(layout: &IDWriteTextLayout) -> (f32, f32) {
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: a valid layout.
        let _ = unsafe { layout.GetMetrics(&mut metrics) };
        (metrics.widthIncludingTrailingWhitespace, metrics.height)
    }
}

/// Text widths for the shared layout, measured with DirectWrite in the UI family.
pub(crate) struct DirectWriteMeasure<'a>(pub(crate) &'a Graphics);

impl Measure for DirectWriteMeasure<'_> {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        self.0.layout(text, size, medium, Family::Ui, 100_000.).map_or(0., |layout| Graphics::extent(&layout).0)
    }
}

fn color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn rect(f: Frame) -> D2D_RECT_F {
    D2D_RECT_F { left: f.x, top: f.y, right: f.x + f.w, bottom: f.y + f.h }
}

/// One frame of drawing on a render target, in logical pixels (the target's DPI is the screen scale).
pub(crate) struct Painter<'a> {
    pub(crate) g: &'a Graphics,
    pub(crate) target: &'a ID2D1RenderTarget,
    brush: ID2D1SolidColorBrush,
    pub(crate) c: Colors,
}

impl<'a> Painter<'a> {
    pub(crate) fn new(g: &'a Graphics, target: &'a ID2D1RenderTarget, c: Colors) -> Option<Self> {
        // SAFETY: a valid target.
        let brush = unsafe { target.CreateSolidColorBrush(&color(c.text), None) }.ok()?;
        Some(Self { g, target, brush, c })
    }

    fn paint(&self, c: Rgba) -> &ID2D1SolidColorBrush {
        // SAFETY: our brush.
        unsafe { self.brush.SetColor(&color(c)) };
        &self.brush
    }

    pub(crate) fn fill(&self, f: Frame, radius: f32, c: Rgba) {
        if c.a <= 0. || f.w <= 0. || f.h <= 0. {
            return;
        }
        let brush = self.paint(c);
        // SAFETY: valid target and brush.
        unsafe {
            if radius > 0. {
                let rounded = D2D1_ROUNDED_RECT { rect: rect(f), radiusX: radius, radiusY: radius };
                self.target.FillRoundedRectangle(&rounded, brush);
            } else {
                self.target.FillRectangle(&rect(f), brush);
            }
        }
    }

    /// A frame `width` wide just inside `f` (a hairline where the system draws no border; 2 in high contrast).
    pub(crate) fn outline(&self, f: Frame, radius: f32, c: Rgba, width: f32) {
        let brush = self.paint(c);
        let h = width / 2.;
        let inset = D2D_RECT_F { left: f.x + h, top: f.y + h, right: f.x + f.w - h, bottom: f.y + f.h - h };
        // Wider than a hairline: the corner follows the frame's middle.
        let radius = (radius - (h - 0.5)).max(0.);
        let rounded = D2D1_ROUNDED_RECT { rect: inset, radiusX: radius, radiusY: radius };
        // SAFETY: valid target and brush.
        unsafe { self.target.DrawRoundedRectangle(&rounded, brush, width, None) };
    }

    fn line(&self, points: &[(f32, f32)], width: f32, c: Rgba) {
        let brush = self.paint(c);
        for pair in points.windows(2) {
            // SAFETY: valid target, brush and stroke style.
            unsafe {
                self.target.DrawLine(
                    Vector2 { X: pair[0].0, Y: pair[0].1 },
                    Vector2 { X: pair[1].0, Y: pair[1].1 },
                    brush,
                    width,
                    &self.g.round,
                );
            }
        }
    }

    /// `text` in `box_`, vertically centred, from its left edge (`align` 0) or right-aligned (`align` 1) or centred
    /// (0.5); clipped to the box's width with an ellipsis.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text(&self, text: &str, size: f32, medium: bool, family: Family, box_: Frame, align: f32, c: Rgba) {
        if text.is_empty() || c.a <= 0. {
            return;
        }
        let Some(layout) = self.g.layout(text, size, medium, family, box_.w.max(1.)) else { return };
        let (width, height) = Graphics::extent(&layout);
        let x = box_.x + ((box_.w - width) * align).max(0.);
        let y = box_.y + (box_.h - height) / 2.;
        let brush = self.paint(c);
        // SAFETY: valid target, layout and brush.
        unsafe {
            self.target.DrawTextLayout(
                Vector2 { X: x.round(), Y: y.round() },
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }

    /// A line glyph `size` square at `(x, cy)` (left edge, vertical centre).
    fn glyph(&self, glyph: Glyph, x: f32, cy: f32, size: f32, c: Rgba) {
        let (l, r, t, b) = (x + size * 0.18, x + size * 0.82, cy - size * 0.32, cy + size * 0.32);
        let cx = x + size / 2.;
        let head = size * 0.26;
        let w = (size * 0.1).max(1.1);
        match glyph {
            Glyph::ArrowDown => {
                self.line(&[(cx, t), (cx, b)], w, c);
                self.line(&[(cx - head, b - head), (cx, b), (cx + head, b - head)], w, c);
            }
            Glyph::ArrowUp => {
                self.line(&[(cx, b), (cx, t)], w, c);
                self.line(&[(cx - head, t + head), (cx, t), (cx + head, t + head)], w, c);
            }
            Glyph::ArrowLeft => {
                self.line(&[(r, cy), (l, cy)], w, c);
                self.line(&[(l + head, cy - head), (l, cy), (l + head, cy + head)], w, c);
            }
            Glyph::ArrowRight => {
                self.line(&[(l, cy), (r, cy)], w, c);
                self.line(&[(r - head, cy - head), (r, cy), (r - head, cy + head)], w, c);
            }
            Glyph::Return => {
                let y = cy + size * 0.12;
                self.line(&[(r, t), (r, y), (l, y)], w, c);
                self.line(&[(l + head, y - head), (l, y), (l + head, y + head)], w, c);
            }
            Glyph::Backspace => {
                let tip = l - size * 0.05;
                self.line(
                    &[
                        (r, t + size * 0.08),
                        (l + head, t + size * 0.08),
                        (tip, cy),
                        (l + head, b - size * 0.08),
                        (r, b - size * 0.08),
                        (r, t + size * 0.08),
                    ],
                    w,
                    c,
                );
                let m = (l + head + r) / 2.;
                let d = size * 0.12;
                self.line(&[(m - d, cy - d), (m + d, cy + d)], w, c);
                self.line(&[(m - d, cy + d), (m + d, cy - d)], w, c);
            }
        }
    }

    /// A chevron (`direction` -1 left, 1 right) `size` square centred at `(cx, cy)`.
    fn chevron(&self, direction: f32, cx: f32, cy: f32, size: f32, c: Rgba) {
        let dx = size * 0.14 * direction;
        let dy = size * 0.27;
        self.line(&[(cx - dx, cy - dy), (cx + dx, cy), (cx - dx, cy + dy)], (size * 0.1).max(1.2), c);
    }

    /// A key cap `width` wide, centred vertically in a box `box_h` tall at `(x, y)`.
    fn keycap(&self, key: &str, x: f32, y: f32, width: f32, box_h: f32, on: bool) {
        let top = y + ((box_h - KEYCAP_HEIGHT) / 2.).max(0.).round();
        let cap = Frame::new(x, top, width, KEYCAP_HEIGHT);
        self.fill(cap, tokens::KEYCAP_RADIUS, if on { self.c.keycap_on } else { self.c.keycap });
        let ink = if on {
            self.c.accent
        } else if keys::is_speck(key) {
            self.c.text
        } else {
            self.c.text_secondary
        };
        let cy = top + KEYCAP_HEIGHT / 2.;
        if keys::is_speck(key) {
            let size = tokens::KEYCAP_FONT + keys::SPECK_GROWTH;
            let drop = keys::speck_drop(key, size);
            self.text(key, size, true, Family::Ui, Frame::new(x, cap.y + drop, width, KEYCAP_HEIGHT), 0.5, ink);
            return;
        }
        let font = tokens::KEYCAP_FONT;
        let pieces = keys::pieces(key);
        let widths: Vec<f32> = pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => DirectWriteMeasure(self.g).width(text, font, true),
                Piece::Glyph(_) => keys::glyph_size(font),
                Piece::Gap => keys::gap(font),
            })
            .collect();
        let total: f32 = widths.iter().sum();
        let mut at = x + (width - total) / 2.;
        for (piece, w) in pieces.iter().zip(widths) {
            match piece {
                Piece::Text(text) => {
                    self.text(text, font, true, Family::Ui, Frame::new(at, cap.y, w + 1., KEYCAP_HEIGHT), 0., ink)
                }
                Piece::Glyph(glyph) => self.glyph(*glyph, at, cy, keys::glyph_size(font), ink),
                Piece::Gap => {}
            }
            at += w;
        }
    }
}

/// What moves while the window turns a page (`panel_window` keeps the clock).
#[derive(Clone, Debug, Default)]
pub(crate) struct Motion<'a> {
    /// The incoming rows' offset along the list and opacity.
    pub(crate) incoming: (f32, f32, f32),
    /// The outgoing page's rows, offset and opacity.
    pub(crate) outgoing: Option<(&'a [RowLayout], f32, f32, f32)>,
    /// The highlight's offset (the bounce at the list's end).
    pub(crate) highlight: (f32, f32),
    /// The old page label and how far the new one has faded in.
    pub(crate) old_label: Option<&'a str>,
    pub(crate) label_fade: f32,
    /// The row under the pointer.
    pub(crate) hover: Option<usize>,
    pub(crate) hover_control: Option<Control>,
}

/// A clickable part of the window besides the rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Single,
    Corrections,
    Previous,
    Next,
    Zhuyin,
    /// The notice's 「復原」.
    Undo,
}

/// Draws the window's content for `layout` (the surface itself is drawn by the caller).
pub(crate) fn content(p: &Painter<'_>, layout: &PanelLayout, motion: &Motion<'_>) {
    let fs = layout.font_size;
    let c = p.c;
    if let Some(notice) = &layout.notice {
        notice_element(p, notice);
        if let Some(frame) = notice.undo {
            let ink = if motion.hover_control == Some(Control::Undo) { c.text } else { c.text_secondary };
            p.text(layout::UNDO_LABEL, notice.font_size, true, Family::Ui, frame, 1., ink);
        }
    }
    if let Some((frame, text, size)) = &layout.tip {
        p.text(text, *size, false, Family::Ui, *frame, 0., c.text_tertiary);
    }
    if let Some((frame, text, text_w)) = &layout.preview {
        p.text(text, fs, false, Family::Ui, *frame, 0., c.text);
        let line_y = frame.y + frame.h / 2. + fs * 0.65 - 1.;
        p.fill(Frame::new(frame.x, line_y, text_w.min(frame.w), 1.), 0., c.text);
    }
    if let Some(hint) = &layout.hint {
        hint_element(p, hint, layout.row_radius, motion.hover_control == Some(Control::Zhuyin));
    }
    if layout.list_open {
        let area = layout.rows_area;
        // SAFETY: a valid target; popped below.
        unsafe { p.target.PushAxisAlignedClip(&rect(area), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
        if let Some(selected) = layout.selected_frame() {
            let f = Frame::new(
                area.x + selected.x + motion.highlight.0,
                area.y + selected.y + 1. + motion.highlight.1,
                selected.w,
                selected.h - 2.,
            );
            p.fill(f, layout.row_radius, c.selection);
            if let Some(outline) = c.selection_outline {
                p.outline(f, layout.row_radius, outline, c.stroke);
            }
        }
        // The outgoing page's rows slide over the new highlight: drawn as rows that are not highlighted.
        if let Some((rows, x, y, alpha)) = motion.outgoing {
            rows_element(p, layout, rows, (x, y, alpha), None, false);
        }
        rows_element(p, layout, &layout.rows, motion.incoming, motion.hover, true);
        // SAFETY: pushed above.
        unsafe { p.target.PopAxisAlignedClip() };
    }
    if let Some(rule) = layout.footer_rule {
        p.fill(rule, 0., c.hairline);
    }
    for (toggle, control) in [(&layout.single, Control::Single), (&layout.corrections, Control::Corrections)] {
        if let Some(toggle) = toggle {
            toggle_element(p, toggle, motion.hover_control == Some(control));
        }
    }
    if let Some(pager) = &layout.pager {
        let label = |text: &str, alpha: f32| {
            p.text(text, SMALL_FONT, false, Family::Ui, pager.label_frame, 0.5, c.text_secondary.fade(alpha));
        };
        if motion.label_fade < 1.
            && let Some(old) = motion.old_label
        {
            label(old, 1. - motion.label_fade);
        }
        label(&pager.label, motion.label_fade);
        for (frame, enabled, direction, control) in [
            (pager.previous, pager.previous_enabled, -1., Control::Previous),
            (pager.next, pager.next_enabled, 1., Control::Next),
        ] {
            let hovered = enabled && motion.hover_control == Some(control);
            if hovered {
                p.fill(frame, tokens::KEYCAP_RADIUS, c.hover);
            }
            let ink = if hovered {
                c.text
            } else if enabled {
                c.text_tertiary
            } else {
                c.text_tertiary.fade(0.3)
            };
            p.chevron(direction, frame.x + frame.w / 2., frame.y + frame.h / 2., tokens::CHEVRON, ink);
        }
    }
}

fn rows_element(
    p: &Painter<'_>,
    layout: &PanelLayout,
    rows: &[RowLayout],
    (dx, dy, alpha): (f32, f32, f32),
    hover: Option<usize>,
    highlighted: bool,
) {
    let c = p.c;
    let fs = layout.font_size;
    let area = layout.rows_area;
    let small = layout::annotation_font(fs);
    for row in rows {
        let frame =
            Frame::new(area.x + row.frame.x + dx, area.y + row.frame.y + 1. + dy, row.frame.w, row.frame.h - 2.);
        let selected = row.selected && highlighted;
        if hover == Some(row.index) && !row.selected {
            p.fill(frame, layout.row_radius, c.hover.fade(alpha));
        }
        let key_ink = if selected { c.selection_secondary } else { c.text_tertiary };
        p.text(
            &row.key,
            layout.number_font,
            true,
            Family::Ui,
            Frame::new(frame.x + row.key_inset, frame.y, layout.number_width, frame.h),
            1.,
            key_ink.fade(alpha),
        );
        p.text(
            &row.text,
            fs,
            false,
            Family::Ui,
            Frame::new(frame.x + row.text_inset, frame.y, row.main_width + 1., frame.h),
            0.,
            if selected { c.selection_text } else { c.text }.fade(alpha),
        );
        if let Some(annotation) = &row.annotation {
            let ink = if selected { c.selection_secondary } else { c.text_tertiary };
            p.text(
                annotation,
                small,
                false,
                Family::Zhuyin,
                Frame::new(frame.x + row.text_inset + row.main_width + 8., frame.y, row.annotation_width + 1., frame.h),
                0.,
                ink.fade(alpha),
            );
        }
    }
}

fn hint_element(p: &Painter<'_>, hint: &HintLayout, radius: f32, hovered: bool) {
    let c = p.c;
    let f = hint.frame;
    if hovered && hint.clickable {
        p.fill(f, radius, c.hover);
    }
    let group_w = hint.keycap_w + 6. + hint.key_label_w;
    let key_room = if hint.keycap_w > 0. { group_w + hint.keycap_right + 12. } else { 0. };
    p.text(
        &hint.hint,
        SMALL_FONT,
        false,
        Family::Zhuyin,
        Frame::new(f.x + hint.text_x, f.y, (f.w - hint.text_x - key_room).max(0.), f.h),
        0.,
        c.text_secondary,
    );
    if hint.keycap_w > 0. {
        let x = f.x + f.w - hint.keycap_right - group_w;
        p.keycap(&hint.key, x, f.y, hint.keycap_w, f.h, false);
        p.text(
            hint.key_label,
            SMALL_FONT,
            false,
            Family::Ui,
            Frame::new(x + hint.keycap_w + 6., f.y, hint.key_label_w + 2., f.h),
            0.,
            c.text_tertiary,
        );
    }
}

fn toggle_element(p: &Painter<'_>, toggle: &ToggleLayout, hovered: bool) {
    let c = p.c;
    let f = toggle.frame;
    p.keycap(&toggle.key, f.x, f.y, toggle.keycap_w, CONTROL_H, toggle.on);
    let ink = if toggle.on {
        c.accent
    } else if hovered {
        c.text
    } else {
        c.text_secondary
    };
    p.text(
        toggle.label,
        SMALL_FONT,
        false,
        Family::Ui,
        Frame::new(f.x + toggle.keycap_w + 6., f.y, (f.w - toggle.keycap_w - 6.).max(0.) + 2., CONTROL_H),
        0.,
        ink,
    );
}

fn notice_element(p: &Painter<'_>, notice: &NoticeLayout) {
    let c = p.c;
    let f = notice.frame;
    if let Some(key) = &notice.key {
        p.keycap(key, f.x, f.y, notice.keycap_w, f.h, false);
    } else if notice.done {
        let size = tokens::NOTICE_ICON;
        let (x, cy) = (f.x, f.y + f.h / 2.);
        p.line(
            &[(x + size * 0.18, cy), (x + size * 0.42, cy + size * 0.24), (x + size * 0.84, cy - size * 0.22)],
            1.6,
            c.success,
        );
    }
    let label_w = notice.label_w.ceil() + 1.;
    let x = f.x + notice.lead;
    p.text(&notice.label, notice.font_size, false, Family::Ui, Frame::new(x, f.y, label_w, f.h), 0., c.text_secondary);
    p.text(
        &notice.value,
        notice.font_size,
        false,
        Family::Ui,
        Frame::new(x + label_w, f.y, (f.w - notice.lead - label_w).max(0.), f.h),
        0.,
        c.text,
    );
}

/// The 中/英 badge's content in a window `side` square.
pub(crate) fn badge(p: &Painter<'_>, text: &str, side: f32) {
    p.text(text, tokens::BADGE_FONT, true, Family::Ui, Frame::new(0., 0., side, side), 0.5, p.c.text);
}

/// Where a click at `(x, y)` lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Row(usize),
    Control(Control),
}

pub(crate) fn hit(layout: &PanelLayout, x: f32, y: f32) -> Option<Hit> {
    if let Some(undo) = layout.notice.as_ref().and_then(|notice| notice.undo)
        && undo.contains(x, y)
    {
        return Some(Hit::Control(Control::Undo));
    }
    if layout.list_open && layout.rows_area.contains(x, y) {
        let (rx, ry) = (x - layout.rows_area.x, y - layout.rows_area.y);
        return layout.rows.iter().find(|row| row.frame.contains(rx, ry)).map(|row| Hit::Row(row.index));
    }
    if let Some(toggle) = &layout.single
        && toggle.frame.contains(x, y)
    {
        return Some(Hit::Control(Control::Single));
    }
    if let Some(toggle) = &layout.corrections
        && toggle.frame.contains(x, y)
    {
        return Some(Hit::Control(Control::Corrections));
    }
    if let Some(pager) = &layout.pager {
        if pager.previous_enabled && pager.previous.contains(x, y) {
            return Some(Hit::Control(Control::Previous));
        }
        if pager.next_enabled && pager.next.contains(x, y) {
            return Some(Hit::Control(Control::Next));
        }
    }
    if let Some(hint) = &layout.hint
        && hint.clickable
        && hint.frame.contains(x, y)
    {
        return Some(Hit::Control(Control::Zhuyin));
    }
    None
}
