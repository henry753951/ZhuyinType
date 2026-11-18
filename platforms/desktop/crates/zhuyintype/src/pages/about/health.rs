//! 關於 › 檢查輸入法 (macOS `HealthPage`): each check with its status, one or two sentences and its fix, 重新檢查 and
//! 回報問題…, which shows the exact text before it is copied or saved (nothing is sent anywhere).

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClipboardItem, Hsla, InteractiveElement as _, IntoElement, ParentElement, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use zt_core::health::{self, Check, Status};

use super::super::Page;
use crate::app::{self, Place};
use crate::health::{self as state, Health};
use crate::scroll::SmoothScroll;
use crate::theme::{self, Palette, size, space, text};
use crate::ui::actions::{ButtonKind, push_button};
use crate::ui::alert::notice;
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
        section = section.row(
            ui::row(SharedString::from(format!("health-{}", check.id)), check.title.clone())
                .subtitle(check.detail.clone())
                .anchor(check.id)
                .trailing(trailing),
        );
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

/// 回報問題: the exact text, with 拷貝, 儲存為檔案… and 關閉.
fn show_report(window: &mut Window, cx: &mut App) {
    let report: SharedString = state::report(cx).into();
    let scroll = SmoothScroll::new(ScrollHandle::new());
    window.open_dialog(cx, move |dialog, _, cx| {
        let p = theme::palette(cx);
        let copy = report.clone();
        let save = report.clone();
        let body = v()
            .gap(space::S12)
            .p(space::S20)
            .child(div().text_size(text::BODY).text_color(p.text).child("回報問題"))
            .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(
                "以下是會拷貝或存下的全部內容：版本、系統、檢查結果與 ZhuyinType 最近的錯誤，不含你打的字或個人字詞。不會自動送出。",
            ))
            .child(
                div()
                    .id("health-report-text")
                    .h(size::REPORT_TEXT_H)
                    .overflow_y_scroll()
                    .track_scroll(scroll.handle())
                    .child(scroll.wheel())
                    .p(space::S12)
                    .rounded(size::R_SM)
                    .border_1()
                    .border_color(p.surface_border)
                    .bg(p.subtle)
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
        dialog.w(size::REPORT_W).p(px(0.)).gap(px(0.)).close_button(false).on_ok(|_, _, _| true).child(body)
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
