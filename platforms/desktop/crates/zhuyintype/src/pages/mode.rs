//! 「中英切換」 page (SETTINGS_IA.md §2; macOS `ModePane`): the keys that switch (單按 Shift and which Shift, Caps Lock),
//! how typing starts and the 中／英 badge, and the 快速設定 key with links to the other switches it flips.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window};

use super::{Page, links};
use crate::theme::{size, space};
use crate::ui::current_keys::{Command, CurrentKeys};
use crate::ui::feature::{command_row, feature_row, keys_overview_link, place_link};
use crate::ui::{h, section, setting_row, v};

pub struct ModePage;

impl ModePage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ModePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shift_switch = crate::app::settings(cx).bool("shiftSwitch");
        let toast = match CurrentKeys::new(cx).mode_key() {
            Some(key) => format!("按 {key} 切換時，在游標旁顯示「中」或「英」"),
            None => "切換時在游標旁顯示「中」或「英」".to_owned(),
        };
        v().gap(size::SECTION_GAP)
            .child(
                section("切換鍵")
                    .row(setting_row("shiftSwitch", cx))
                    .when(shift_switch, |this| {
                        this.row(setting_row("shiftToggle", cx).indent(true).anchor("shiftSide"))
                    })
                    .rows(feature_row("feature.capsLockEnglish").rows(cx)),
            )
            .child(
                section("預設與提示")
                    .row(setting_row("startInEnglish", cx))
                    .row(setting_row("showModeToast", cx).subtitle(toast)),
            )
            .child(
                section("快速設定")
                    .row(command_row(
                        Command::QuickSettings,
                        "開啟快速設定",
                        "在游標旁切換英文、半形標點、簡體與暫停學習；預設沒有按鍵",
                        cx,
                    ))
                    .footer(
                        v().gap(space::S8)
                            .child(h().flex_wrap().gap(space::S12).child("也可以在這裡設定：").children(
                                links::quick_settings_links().into_iter().map(|(title, place)| {
                                    place_link(SharedString::from(format!("quick-link-{title}")), title, place, cx)
                                }),
                            ))
                            .child(keys_overview_link(Page::Mode, cx)),
                    ),
            )
    }
}
