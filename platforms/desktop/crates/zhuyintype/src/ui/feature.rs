//! Rows shared by the pages whose settings have keys (macOS `FeatureRow`, `CommandKeyRows`, `DrillRow`,
//! `KeysOverviewLink`, `PlaceLink` in `SettingsPages.swift`).
//!
//! A feature is its row (title, description, switch) followed by one indented 「快捷鍵」 row per command, shown only
//! while the feature is on; a command without a switch is one row of its own.  Either way the keys are edited right
//! there with the shared key field (`ui::shortcuts`), with 「還原預設」 under the title while they differ from the
//! defaults; the row's anchor is `keys-<command>` (`pages::links::command_place`).  Turned off, the description says
//! which keys go to the app; made unavailable by another setting, the row dims and links to that setting.

use gpui_kit::{AnyElement, App, IntoElement, SharedString};
use zt_core::settings;

use super::current_keys::{Command, CurrentKeys, compact};
use super::shortcuts::{self, restore_link, store};
use super::{Row, controls, row};
use crate::app::{self, Place};
use crate::pages::{Page, links};

/// Why a feature is not in use although its switch may be on: another setting turns it off or changes it (macOS
/// `KeyAvailability`).
pub struct Availability {
    pub note: &'static str,
    pub place: Place,
    pub link_title: &'static str,
}

/// What another page's setting does to the feature `key` (`feature.…`); None when nothing.
pub fn availability(key: &str, cx: &App) -> Option<Availability> {
    let settings = app::settings(cx);
    match key {
        "feature.zhuyinRowClick" if !settings.bool("showsZhuyinHint") => Some(Availability {
            note: "「顯示注音」已關閉，候選字視窗沒有注音列",
            place: links::shows_zhuyin_place(),
            link_title: "候選字視窗",
        }),
        "feature.correctionsList"
            if settings.string("typoTolerance") == "off"
                && settings.string("keyOrder") != "list"
                && !["zZh", "cCh", "sSh", "enEng", "anAng"]
                    .iter()
                    .any(|pair| settings.bool(&format!("fuzzy.{pair}"))) =>
        {
            Some(Availability {
                note: "「按錯鍵修正」關閉、「按鍵順序」不是只列出，也沒有開啟模糊音：這個鍵開啟一般清單",
                place: links::typo_place(),
                link_title: "修正",
            })
        }
        // An Alt chord without Ctrl never reaches a Windows text service (it is a system key): the switch would promise
        // keys that go to the app.  Ctrl with a symbol key types the Chinese punctuation instead.
        "feature.optionSymbols" if cfg!(windows) => Some(Availability {
            note: "Windows 的 Alt 組合鍵交給 App，輸入法收不到；中文標點可以用 Ctrl 加符號鍵",
            place: links::feature_place("feature.controlPunctuation"),
            link_title: "Ctrl 加符號鍵",
        }),
        _ => None,
    }
}

/// A feature row under construction.
pub struct FeatureRow {
    title: SharedString,
    subtitle: SharedString,
    feature: Option<&'static str>,
    commands: Vec<Command>,
    anchor: Option<SharedString>,
}

/// The feature `key` (a `feature.…` toggle of the registry): its title, description and anchor from `settings.json`,
/// its commands from the descriptor's `commands`.
pub fn feature_row(key: &'static str) -> FeatureRow {
    let descriptor = settings::descriptor(key);
    let commands = descriptor
        .and_then(|d| d.extra.get("commands"))
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().filter_map(|id| id.as_str().and_then(Command::from_id)).collect())
        .unwrap_or_default();
    FeatureRow {
        title: descriptor.map(|d| d.title.clone()).unwrap_or_else(|| key.to_owned()).into(),
        subtitle: descriptor.and_then(|d| d.detail.clone()).unwrap_or_default().into(),
        feature: Some(key),
        commands,
        anchor: descriptor.and_then(|d| d.anchor.clone()).map(Into::into),
    }
}

impl FeatureRow {
    /// The description while the feature is on, in place of the registry's (it names the current keys).
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// The row and, while the feature is on, its key rows.
    pub fn rows(self, cx: &App) -> Vec<AnyElement> {
        let on = self.feature.is_none_or(|key| app::settings(cx).bool(key));
        let available = self.feature.and_then(|key| availability(key, cx));
        let subtitle: SharedString = match (&available, on) {
            (Some(availability), _) => availability.note.into(),
            (None, false) if self.feature == Some("feature.zhuyinRowClick") => "關閉時，點注音列不做事".into(),
            (None, false) => {
                // Every key of the feature's commands; Page Up and Page Down page the list anyway.
                let keys = CurrentKeys::new(cx);
                let handed: Vec<String> = self
                    .commands
                    .iter()
                    .flat_map(|&command| keys.all(command))
                    .map(|caps| compact(&caps))
                    .filter(|key| key != "PgUp" && key != "PgDn")
                    .map(|key| super::keycap::prose(&key))
                    .collect();
                if handed.is_empty() {
                    self.subtitle.clone()
                } else {
                    format!("關閉時，{} 交給 App", handed.join("、")).into()
                }
            }
            (None, true) => self.subtitle.clone(),
        };
        let id = self.feature.map(str::to_owned).unwrap_or_else(|| self.title.to_string());
        let mut main = row(SharedString::from(format!("feature-{id}")), self.title.clone())
            .subtitle(subtitle)
            .dimmed(available.is_some());
        if let Some(key) = self.feature {
            main = main.trailing(controls::setting_switch(key, cx)).marked(app::settings(cx).is_customised(key));
        }
        if let Some(anchor) = self.anchor.clone() {
            main = main.anchor(anchor);
        }
        let unavailable = available.is_some();
        if let Some(availability) = available {
            let place = availability.place.clone();
            main = main.link(controls::text_link(
                SharedString::from(format!("feature-link-{id}")),
                format!("在「{}」設定", availability.link_title),
                move |_, _, cx| app::go(place.clone(), cx),
                cx,
            ));
        }
        let mut rows = vec![main.into_any_element()];
        if on {
            rows.extend(
                command_key_rows(&self.commands, unavailable, cx).into_iter().map(IntoElement::into_any_element),
            );
        }
        rows
    }
}

/// The indented 「快捷鍵」 rows of a feature: one per command (named after the command when there are several), each
/// with the command's key field.  `unavailable`: another setting keeps the feature from working, so the keys dim.
pub fn command_key_rows(commands: &[Command], unavailable: bool, cx: &App) -> Vec<Row> {
    let several = commands.len() > 1;
    commands
        .iter()
        .map(|&command| {
            keys_row(command, if several { command.label() } else { "快捷鍵" }, unavailable, cx).indent(true)
        })
        .collect()
}

/// A command without a switch as one row: its title and description, and its key field (「切換標點全半形」, 「不要再推薦」).
pub fn command_row(
    command: Command,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &App,
) -> Row {
    keys_row(command, title, false, cx).subtitle(subtitle)
}

/// The row that holds `command`'s key field, anchored `keys-<command>`.
fn keys_row(command: Command, title: impl Into<SharedString>, unavailable: bool, cx: &App) -> Row {
    let id = command.id();
    let group = SharedString::from(format!("keys-row-{id}"));
    let field = shortcuts::shortcut_field(id).without_restore().disabled(unavailable).hover_group(group.clone());
    let mut row = row(SharedString::from(format!("keysrow-{id}")), title)
        .anchor(command.keys_row())
        .hover_group(group)
        .marked(store::store(cx).is_customised(id))
        .trailing(field);
    if let Some(link) = restore_link(id, cx).filter(|_| !unavailable) {
        row = row.link(link);
    }
    row
}

/// A row that opens one of the page's sub-pages: title, description, the current state and ›.  Its anchor is the
/// sub-page's id.
pub fn drill_row(
    page: Page,
    subpage: &'static str,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> Row {
    let title: SharedString = title.into();
    let value: SharedString = value.into();
    let row = row(SharedString::from(format!("drill-{subpage}")), title).subtitle(subtitle).anchor(subpage);
    let row = if value.is_empty() { row } else { row.value(value) };
    row.on_click(move |_, _, cx| app::go(Place::subpage(page, subpage), cx))
}

/// A link to a place in the settings window.
pub fn place_link(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    place: Place,
    cx: &App,
) -> impl IntoElement {
    let id: SharedString = id.into();
    controls::text_link(id, title, move |_, _, cx| app::go(place.clone(), cx), cx)
}

/// The page's one link to the overview of all keys, at its foot: to the overview's section named like `page`.
pub fn keys_overview_link(page: Page, cx: &App) -> impl IntoElement {
    place_link("keys-overview", "在快捷鍵總覽中查看全部按鍵", links::keys_section_place(page), cx)
}
