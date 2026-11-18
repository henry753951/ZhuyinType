//! 「色彩」 page (SETTINGS_IA.md §2; Windows' 「個人化 › 色彩」): a still preview of the candidate list that follows the
//! settings below, 主題 (the whole app's light or dark, 透明效果, and the candidate window's own), 強調色, and what the
//! system's accessibility settings change, read-only.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

use crate::candidate::preview::{PreviewState, UnifiedPreview};
use crate::theme::{self, size, space};
use crate::ui::{self, row, section, setting_row, v};

pub struct ColorsPage {
    preview: Entity<UnifiedPreview>,
}

impl ColorsPage {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preview = cx.new(|cx| {
            let mut preview = UnifiedPreview::new(cx);
            preview.tabs = false;
            preview.hold(PreviewState::List);
            preview
        });
        Self { preview }
    }
}

/// 「透明效果」, or why this system has no backdrop (the row then stays off and cannot be changed).
fn transparency_row(cx: &App) -> ui::Row {
    let row = setting_row("transparency", cx);
    match zt_core::appearance::backdrop_support() {
        Ok(()) => row,
        Err(reason) => row.subtitle(reason).dimmed(true).trailing(ui::value_text("關閉", cx)),
    }
}

/// The system's setting `title`, on or off (read-only).
fn system_row(id: &'static str, title: &'static str, on: bool, cx: &App) -> ui::Row {
    row(id, title).anchor(id).trailing(ui::value_text(if on { "開啟" } else { "關閉" }, cx))
}

/// Windows' own page of these settings; Linux desktops have no one place to open.
fn system_settings_button(cx: &App) -> Option<gpui_kit::AnyElement> {
    if !cfg!(windows) {
        return None;
    }
    Some(
        ui::actions::push_button(
            "open-accessibility",
            "打開系統的輔助使用設定",
            ui::actions::ButtonKind::Normal,
            true,
            |_, _, cx| cx.open_url("ms-settings:easeofaccess"),
            cx,
        )
        .into_any_element(),
    )
}

impl Render for ColorsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let high_contrast = zt_core::system_look::look(p.dark).high_contrast.is_some();
        let reduced = crate::motion::reduced(cx);
        // As on 候選字視窗: the page keeps to the room there is, so the preview cannot widen it.
        let room = (window.viewport_size().width - size::SIDEBAR).min(size::CONTENT_MAX) - size::PAGE_PADDING_X * 2.;
        v().w_full()
            .min_w_0()
            .max_w(room)
            .gap(size::SECTION_GAP)
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .p(size::ROW_PADDING_X)
                    .rounded(size::R_LG)
                    .bg(p.card)
                    .border_1()
                    .border_color(p.separator)
                    .child(self.preview.clone()),
            )
            .child(
                section("主題")
                    .row(setting_row("theme", cx))
                    .row(transparency_row(cx))
                    .row(setting_row("panelAppearance", cx)),
            )
            .child(section("").row(setting_row("accentColor", cx)))
            .child(
                section("無障礙")
                    .row(system_row("highContrast", "高對比", high_contrast, cx))
                    .row(system_row("reduceMotion", "減少動態效果", reduced, cx))
                    .footer(
                        v().gap(space::S8)
                            .child(
                                "跟隨系統的設定：高對比時候選字視窗與中英提示改用系統的顏色；減少動態效果時視窗不移動，只淡入淡出。",
                            )
                            .when_some(system_settings_button(cx), |this, button| this.child(div().child(button))),
                    ),
            )
    }
}
