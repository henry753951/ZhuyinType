//! 「快捷鍵」 (macOS `KeysPane`): every key grouped by feature, in the words of what it does.  Rebindable commands are
//! edited right here with the shared key field (the same as on the feature pages); optional features that are off are
//! dimmed with 「已關閉」 and lead to where they are turned on; fixed keys wait under a disclosure at the bottom.  A
//! search field at the top (the title bar's on macOS) filters every row by title, subtitle and keys.

mod demo;
mod items;

use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, WindowExt as _};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use serde_json::json;
use zt_core::settings;

use self::items::{KeyItem, KeySection, Keys};
use crate::app::{self, Navigation};
use crate::theme::{self, size, text};
use crate::ui::charts::fill_column;
use crate::ui::shortcuts::{self, key_chip, store};
use crate::ui::{self, h};

pub struct KeysPage {
    search: Entity<InputState>,
    shows_fixed: bool,
    _subscriptions: Vec<Subscription>,
}

impl KeysPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜尋按鍵或功能"));
        let subscription = cx.subscribe_in(&search, window, |_, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let query = state.read(cx).value();
                cx.global_mut::<Navigation>().query = query;
                cx.notify();
            }
        });
        demo::apply(cx);
        if let Some(query) = demo::value("keys-query") {
            search.update(cx, |state, cx| state.set_value(query.clone(), window, cx));
            cx.global_mut::<Navigation>().query = query.into();
        }
        Self { search, shows_fixed: demo::active("fixed-open"), _subscriptions: vec![subscription] }
    }

    fn search_field(&self, cx: &App) -> impl IntoElement {
        let p = theme::palette(cx);
        h().w_full().child(
            Input::new(&self.search)
                .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(p.text_tertiary))
                .cleanable(true)
                .aria_label("搜尋按鍵"),
        )
    }

    /// One row (macOS `KeyItemRow`).
    fn item_row(item: &KeyItem, keys: &store::KeyStore, cx: &App) -> AnyElement {
        let p = theme::palette(cx);
        let settings = app::settings(cx);
        let on = item.is_on(settings);
        let availability = item.availability(settings);
        let subtitle = availability.as_ref().map(|a| a.note.to_owned()).or_else(|| item.subtitle.clone());
        let anchor = format!("row-{}", item.id);
        let home = item.home();
        let row = ui::row(SharedString::from(anchor.clone()), item.title.clone()).subtitle_opt(subtitle).anchor(anchor.clone());
        if let (Keys::Command(command), true) = (&item.keys, on) {
            // Keys set right here; › goes to the feature's page.
            let command: &'static str = command;
            let group = SharedString::from(format!("keys-row-{}", item.id));
            let field = shortcuts::shortcut_field(command)
                .max_visible(2)
                .disabled(availability.is_some())
                .hover_group(group.clone());
            let go = home.clone().map(|place| {
                let name = place_name(&place);
                div()
                    .id(SharedString::from(format!("go-{}", item.id)))
                    .flex_shrink_0()
                    .h(shortcuts::chip_height())
                    .flex()
                    .items_center()
                    .pl(px(2.))
                    .cursor_pointer()
                    .tooltip(move |window, cx| Tooltip::new(format!("前往「{name}」")).build(window, cx))
                    .on_click(move |_, _, cx| app::go(place.clone(), cx))
                    .child(ui::chevron(cx))
            });
            return row
                .hover_group(group)
                .marked(keys.is_customised(command))
                .trailing(h().items_start().gap(px(8.)).child(field).children(go))
                .into_any_element();
        }
        let trailing: AnyElement = if !on {
            ui::value_text("已關閉", cx).into_any_element()
        } else {
            let chips: Vec<AnyElement> = match &item.keys {
                Keys::Fixed(caps, alternative) => std::iter::once(caps)
                    .chain((!alternative.is_empty()).then_some(alternative))
                    .map(|caps| key_chip(items::fixed_chip(caps)).into_any_element())
                    .collect(),
                Keys::Command(command) => {
                    let conflicts = keys.conflicts_of(command);
                    keys.bindings(command)
                        .into_iter()
                        .enumerate()
                        .map(|(index, binding)| {
                            key_chip(binding.label).tint(conflicts.contains_key(&index).then_some(p.danger)).into_any_element()
                        })
                        .collect()
                }
                Keys::ShiftTap => vec![key_chip(items::shift_tap(settings).unwrap_or("Shift")).into_any_element()],
            };
            h().gap(px(6.)).when(availability.is_some(), |this| this.opacity(0.45)).children(chips).into_any_element()
        };
        let row = row.dimmed(!on).trailing(trailing);
        match home {
            Some(place) => row.on_click(move |_, _, cx| app::go(place.clone(), cx)).into_any_element(),
            None => row.into_any_element(),
        }
    }

    fn reset_all(window: &mut Window, cx: &mut App) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title("要還原所有按鍵與功能嗎？")
                .description("所有按鍵回到預設值，所有功能開啟，單按左右 Shift 切換中英。")
                .show_cancel(true)
                .ok_text("全部還原")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("取消")
                .on_ok(|_, window, cx| {
                    // Every key and key feature back to its default, then both Shifts.
                    if let Err(error) = store::store(cx).reset(None) {
                        window.push_notification(error.to_string(), cx);
                    }
                    for descriptor in settings::descriptors().iter().filter(|d| d.key.starts_with("feature.")) {
                        app::reset_setting(&descriptor.key, cx);
                    }
                    app::set_setting("shiftToggle", json!("both"), cx);
                    true
                })
        });
    }
}

/// The title of the page (or sub-page) a place leads to.
fn place_name(place: &app::Place) -> String {
    place
        .subpage
        .as_deref()
        .and_then(|id| super::subpage_title(place.page, id))
        .unwrap_or(place.page.title())
        .to_owned()
}

impl Render for KeysPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        // A link from another page clears the search; keep the field in step.
        let query = app::nav(cx).query.clone();
        if query.is_empty() && !self.search.read(cx).value().is_empty() {
            self.search.update(cx, |state, cx| state.set_value("", window, cx));
        }
        let keys = store::store(cx);
        let settings = app::settings(cx).clone();
        let mut sections: Vec<(KeySection, Vec<KeyItem>)> = Vec::new();
        let mut fixed: Vec<KeyItem> = Vec::new();
        for section in KeySection::ALL {
            let rows: Vec<KeyItem> =
                items::items(section, &settings).into_iter().filter(|item| item.matches(&query, &keys, &settings)).collect();
            let (fixed_rows, rows): (Vec<_>, Vec<_>) = rows.into_iter().partition(KeyItem::is_fixed);
            fixed.extend(fixed_rows);
            if !rows.is_empty() {
                sections.push((section, rows));
            }
        }
        let conflicts = keys.conflicts();
        let changed = keys.any_customised()
            || settings::descriptors().iter().any(|d| d.key.starts_with("feature.") && !settings.bool(&d.key))
            || settings.string("shiftToggle") != "both";
        let open = self.shows_fixed || !query.is_empty();
        let entity = cx.entity();

        fill_column()
            .gap(size::SECTION_GAP)
            .child(self.search_field(cx))
            .when_some(keys.error(), |this, error| {
                this.child(
                    h().gap(px(8.))
                        .px(size::ROW_PADDING_X)
                        .text_size(text::SUBTITLE)
                        .text_color(p.danger)
                        .child(Icon::new(IconName::TriangleAlert).size(px(13.)))
                        .child(error),
                )
            })
            .when(!conflicts.is_empty(), |this| {
                this.child(ui::section("按鍵衝突").rows(conflicts.iter().enumerate().map(|(index, conflict)| {
                    let place = store::command_place(conflict.command);
                    ui::row(
                        SharedString::from(format!("conflict-{index}")),
                        format!("「{}」與「{}」", store::command_label(conflict.command), store::command_label(conflict.other)),
                    )
                    .subtitle("用同一個按鍵；同一個按鍵只會執行其中一個功能")
                    .trailing(key_chip(conflict.label.clone()).tint(Some(p.danger)))
                    .on_click(move |_, _, cx| app::go(place.clone(), cx))
                })))
            })
            .when(sections.is_empty() && fixed.is_empty(), |this| {
                this.child(
                    ui::section("").row(
                        div()
                            .w_full()
                            .min_h(px(60.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(p.text_secondary)
                            .child(format!("沒有符合「{query}」的項目")),
                    ),
                )
            })
            .children(sections.iter().map(|(section, rows)| {
                ui::section(section.title()).rows(rows.iter().map(|item| Self::item_row(item, &keys, cx)))
            }))
            .when(!fixed.is_empty(), |this| {
                let toggle = entity.clone();
                // The whole row toggles; its chevron turns down while open.
                let disclosure = div()
                    .id("fixed-keys-toggle")
                    .w_full()
                    .cursor_pointer()
                    .hover(|style| style.bg(p.hover))
                    .active(|style| style.bg(p.pressed))
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |page, cx| {
                            page.shows_fixed = !page.shows_fixed;
                            cx.notify();
                        });
                    })
                    .child(
                        ui::row("fixed-keys", "固定的按鍵")
                            .subtitle("打注音、聲調、送出與移動游標的按鍵，無法更改")
                            .trailing(
                                Icon::new(IconName::ChevronRight)
                                    .size(px(13.))
                                    .text_color(p.text_tertiary)
                                    .rotate(gpui_kit::radians(if open { std::f32::consts::FRAC_PI_2 } else { 0. })),
                            ),
                    );
                let mut section = ui::section("").row(disclosure);
                if open {
                    section = section.rows(fixed.iter().map(|item| Self::item_row(item, &keys, cx)));
                }
                this.child(
                    section.footer(
                        h().w_full()
                            .justify_between()
                            .gap(px(12.))
                            .child("點一下任何一列，到功能的頁面設定按鍵。")
                            .child(
                                div()
                                    .id("reset-all-keys")
                                    .flex_shrink_0()
                                    .text_color(if changed { p.accent } else { p.text_tertiary })
                                    .when(changed, |this| {
                                        this.cursor_pointer()
                                            .hover(|style| style.opacity(0.8))
                                            .on_click(|_, window, cx| Self::reset_all(window, cx))
                                    })
                                    .child("全部還原…"),
                            ),
                    ),
                )
            })
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(_id: &str) -> Option<&'static str> {
    None
}
