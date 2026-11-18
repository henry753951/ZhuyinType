//! 關於 › 檢查輸入法 (macOS `HealthPage`): each check with its status, one or two sentences and its fix, and under a
//! 「詳細資料」 link what was checked and found; 重新檢查 and 回報問題…, which shows the exact text before it is copied
//! or saved (nothing is sent anywhere).

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClipboardItem, Hsla, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use zt_core::health::{self, Check, Status};

use super::super::Page;
use crate::app::{self, Place};
use crate::health::{self as state, Health};
use crate::theme::{self, Palette, size, space, text, weight};
use crate::ui::actions::{ButtonKind, push_button};
use crate::ui::alert::{notice, surface};
use crate::ui::{self, Row, h, v};

/// The sub-page's id under 關於.
pub const ID: &str = "health";
pub const TITLE: &str = "檢查輸入法";

/// The row on 關於 that opens the sub-page, with the summary.
pub fn about_row(cx: &App) -> Row {
    let summary = match state::checks(cx) {
        Some(checks) => health::summary(checks),
        None => "檢查中…".to_owned(),
    };
    ui::row("drill-health", TITLE)
        .subtitle("找出無法用 ZhuyinType 打字的常見原因")
        .value(summary)
        .anchor(ID)
        .on_click(|_, _, cx| app::go(Place::subpage(Page::About, ID), cx))
}

fn status_icon(status: Status, p: &Palette) -> (IconName, Hsla) {
    match status {
        Status::Ok => (IconName::CircleCheck, p.success),
        Status::Info => (IconName::Info, p.text_secondary),
        Status::Warning => (IconName::TriangleAlert, p.accent),
        Status::Error => (IconName::CircleX, p.danger),
    }
}

/// The sub-page.
pub fn page(cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    let health = cx.try_global::<Health>();
    let running = health.is_some_and(|health| health.running);
    let fixing = health.and_then(|health| health.fixing);
    let fix_error = health.and_then(|health| health.fix_error.clone());
    let open = health.map(|health| health.open.clone()).unwrap_or_default();
    let checks = state::checks(cx).map(<[Check]>::to_vec).unwrap_or_default();
    let summary = if checks.is_empty() { "檢查中…".to_owned() } else { health::summary(&checks) };
    let header = ui::section("").first().row(
        ui::row("health-summary", summary).subtitle("每次回到這個視窗時重新檢查").trailing(
            h().gap(size::ROW_PADDING_Y)
                .child(push_button(
                    "health-rerun",
                    if running { "檢查中…" } else { "重新檢查" },
                    ButtonKind::Normal,
                    !running,
                    |_, _, cx| state::refresh(cx),
                    cx,
                ))
                .child(push_button(
                    "health-report",
                    "回報問題…",
                    ButtonKind::Normal,
                    true,
                    |_, window, cx| show_report(window, cx),
                    cx,
                )),
        ),
    );
    let mut section = ui::section("檢查結果");
    for check in checks {
        let (icon, color) = status_icon(check.status, &p);
        let mut trailing = h().gap(size::ROW_PADDING_Y);
        if let Some(fix) = check.fix.clone() {
            let id = check.id;
            let busy = fixing == Some(id);
            trailing = trailing.child(push_button(
                SharedString::from(format!("health-fix-{id}")),
                if busy { "處理中…" } else { fix.label() },
                if check.status.is_problem() { ButtonKind::Default } else { ButtonKind::Normal },
                fixing.is_none(),
                move |_, _, cx| state::apply(id, &fix, cx),
                cx,
            ));
        }
        trailing = trailing.child(
            h().gap(space::S4)
                .text_size(text::SUBTITLE)
                .text_color(color)
                .child(Icon::new(icon).size(size::ICON_SM))
                .child(check.status.label()),
        );
        let id = check.id;
        let shown = open.contains(id);
        let mut row = ui::row(SharedString::from(format!("health-{id}")), check.title.clone())
            .subtitle(check.detail.clone())
            .anchor(id)
            .trailing(trailing);
        if !check.details.is_empty() {
            row = row.link(details_link(id, shown, &p));
        }
        section = section.row(v().w_full().child(row).when(shown, |this| this.child(details(&check, &p))));
    }
    v().gap(size::SECTION_GAP)
        .child(header)
        .when_some(fix_error, |this, error| {
            this.child(
                h().gap(space::S8)
                    .px(size::ROW_PADDING_X)
                    .text_size(text::SUBTITLE)
                    .text_color(p.danger)
                    .child(Icon::new(IconName::TriangleAlert).size(size::ICON_SM))
                    .child(SharedString::from(format!("無法處理：{error}"))),
            )
        })
        .child(section)
        .into_any_element()
}

/// 「詳細資料 ›」, turning down while they show.
fn details_link(id: &'static str, shown: bool, p: &Palette) -> impl IntoElement {
    let ring = *p;
    h().id(SharedString::from(format!("health-details-{id}")))
        .gap(space::S2)
        .cursor_pointer()
        .text_size(text::SUBTITLE)
        .text_color(p.accent)
        .rounded(size::R_XS)
        .tab_index(0)
        .focus_visible(move |style| crate::motion::focus_ring(style, &ring, ring.surface))
        .hover(|style| style.opacity(0.8))
        .active(|style| style.opacity(0.6))
        .on_click(move |_, _, cx| state::toggle(id, cx))
        .child("詳細資料")
        .child(Icon::new(if shown { IconName::ChevronDown } else { IconName::ChevronRight }).size(size::ICON_XS))
}

/// A check's 詳細資料: what was checked and what was found, label and value, under the row's text.
fn details(check: &Check, p: &Palette) -> impl IntoElement {
    v().w_full()
        .pl(size::ROW_PADDING_X)
        .pr(size::ROW_PADDING_X)
        .pb(size::ROW_PADDING_Y)
        .gap(space::S2)
        .text_size(text::CAPTION)
        .children(check.details.iter().map(|(label, value)| {
            h().w_full()
                .items_start()
                .gap(space::S8)
                .child(
                    div()
                        .w(DETAIL_LABEL_W)
                        .flex_shrink_0()
                        .text_color(p.text_tertiary)
                        .child(SharedString::from(label.clone())),
                )
                .child(
                    div().flex_1().min_w(px(0.)).text_color(p.text_secondary).child(SharedString::from(value.clone())),
                )
        }))
}

/// The labels' column in 詳細資料 (the longest, 「每個應用程式視窗使用不同的輸入法」, wraps).
const DETAIL_LABEL_W: gpui_kit::Pixels = px(150.);

/// 回報問題: the exact text, with 拷貝, 儲存為檔案… and 關閉.
pub(crate) fn show_report(window: &mut Window, cx: &mut App) {
    let report: SharedString = state::report(window.scale_factor(), cx).into();
    window.open_dialog(cx, move |dialog, window, cx| {
        let p = theme::palette(cx);
        let copy = report.clone();
        let save = report.clone();
        let viewport = window.viewport_size();
        // The text as tall as the window allows (the title, the note, the buttons and the margins take about 260
        // px): the report runs to a hundred lines.
        let text_h = (viewport.height - px(260.)).clamp(px(200.), size::REPORT_TEXT_H * 1.5);
        let width = size::REPORT_W.min(viewport.width - space::S16 * 2.);
        let body = v()
            .gap(space::S12)
            .p(space::S20)
            .child(div().text_size(text::HEADLINE).font_weight(weight::SEMIBOLD).text_color(p.text).child("回報問題"))
            .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(
                "以下是會拷貝或存下的全部內容，可以直接貼到 GitHub 的問題回報。不含你打的字、自訂詞或記住的字詞，也不會自動送出。",
            ))
            .child(
                div()
                    .id("health-report-text")
                    .h(text_h)
                    .overflow_y_scroll()
                    .p(space::S12)
                    .rounded(size::R_SM)
                    .border_1()
                    .border_color(p.surface_border)
                    .bg(p.card)
                    .font_family(ui::content::mono_font())
                    .text_size(text::SUBTITLE)
                    .text_color(p.text)
                    .child(report.clone()),
            )
            .child(
                h().gap(size::ROW_PADDING_Y)
                    .justify_end()
                    .child(push_button(
                        "health-copy",
                        "拷貝",
                        ButtonKind::Normal,
                        true,
                        move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string())),
                        cx,
                    ))
                    .child(push_button(
                        "health-save",
                        "儲存為檔案…",
                        ButtonKind::Normal,
                        true,
                        move |_, window, cx| save_report(save.to_string(), window, cx),
                        cx,
                    ))
                    .child(push_button(
                        "health-close",
                        "關閉",
                        ButtonKind::Default,
                        true,
                        |_, window, cx| window.close_dialog(cx),
                        cx,
                    )),
            );
        surface(dialog, cx).w(width).p(px(0.)).gap(px(0.)).close_button(false).on_ok(|_, _, _| true).child(body)
    });
}

fn save_report(report: String, window: &mut Window, cx: &mut App) {
    let home = std::env::home_dir().unwrap_or_else(|| ".".into());
    let chosen = cx.prompt_for_new_path(&home, Some("ZhuyinType 問題回報.txt"));
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(path))) = chosen.await else { return };
            if let Err(error) = std::fs::write(&path, report.as_bytes()) {
                cx.update(|window, cx| notice("無法儲存", format!("{}：{error}", path.display()), window, cx)).ok();
            }
        })
        .detach();
}
