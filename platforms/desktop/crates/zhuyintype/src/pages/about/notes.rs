//! 更新內容: what changed since the version the user updated from (macOS `ReleaseNotesPane`), from the notes the app
//! carries, and the releases' web page for the rest.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, div, px};
use zt_core::release_notes::{self, RELEASES_URL};

use super::VERSION;
use crate::app;
use crate::theme::{self, size, space, text, weight};
use crate::ui::{self, h, v};

/// The sub-page.
pub fn page(cx: &App) -> AnyElement {
    let releases = release_notes::shown(app::settings(cx), VERSION);
    let mut page = v().gap(size::SECTION_GAP);
    if releases.is_empty() {
        page = page.child(ui::section("").first().row(ui::row("no-notes", format!("{VERSION} 的更新內容在發布頁面"))));
    }
    for release in releases {
        let mut section = ui::section(format!("ZhuyinType {}", release.version)).footer(release.date.clone());
        if release.sections.is_empty() {
            let name = if cfg!(windows) { "Windows" } else { "Linux" };
            section = section.row(ui::row(
                SharedString::from(format!("empty-{}", release.version)),
                format!("這一版沒有 {name} 的變更"),
            ));
        }
        for group in &release.sections {
            section = section.row(changes(group, cx));
        }
        page = page.child(section);
    }
    page.child(
        ui::section("")
            .row(ui::setting_row(release_notes::SHOW_KEY, cx))
            .row(
                ui::row("all-notes", "完整的發布說明")
                    .subtitle("每一版的變更、已知問題與準確率，在 GitHub 上")
                    .on_click(|_, _, cx| cx.open_url(RELEASES_URL)),
            )
            .row(ui::row("notes-feedback", "回報問題或建議").on_click(|_, _, cx| app::go(super::feedback_place(), cx))),
    )
    .into_any_element()
}

/// 新功能, 改進 or 修正 with its items.
fn changes(group: &release_notes::Section, cx: &App) -> impl IntoElement + use<> {
    let p = theme::palette(cx);
    v().w_full()
        .px(size::ROW_PADDING_X)
        .py(size::ROW_PADDING_Y)
        .gap(space::S8)
        .child(
            div()
                .text_size(text::SUBTITLE)
                .font_weight(weight::SEMIBOLD)
                .text_color(p.text_secondary)
                .child(SharedString::from(group.title.clone())),
        )
        .children(group.items.iter().map(|item| {
            h().w_full()
                .items_start()
                .gap(space::S8)
                .text_size(text::BODY)
                .child(div().flex_shrink_0().text_color(p.text_tertiary).child("・"))
                .child(
                    v().flex_1()
                        .min_w(px(0.))
                        .gap(space::S2)
                        .when_some(item.title.clone(), |this, title| {
                            this.child(
                                div().font_weight(weight::SEMIBOLD).text_color(p.text).child(SharedString::from(title)),
                            )
                        })
                        .child(div().text_color(p.text).child(SharedString::from(item.text.clone()))),
                )
        }))
}
