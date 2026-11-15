//! Rows shared by the pages whose settings have keys (macOS `FeatureRow`, `CommandKeyRows`, `DrillRow`,
//! `KeysOverviewLink`, `PlaceLink` in `SettingsPages.swift`).
//!
//! A feature is its row (title, description, switch) followed by one indented 「快捷鍵」 row per command, shown only
//! while the feature is on.  Turned off, the description says which keys go to the app; made unavailable by another
//! setting, the row dims and links to that setting.

use gpui_kit::{AnyElement, App, IntoElement, SharedString};
use zt_core::settings;

use super::current_keys::{Command, CurrentKeys, compact, shortcut_chip};
use super::{Row, controls, row};
use crate::app::{self, Place};
use crate::pages::Page;

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
            place: Place::row(Page::Candidates, "showsZhuyin"),
            link_title: "視窗與提示",
        }),
        "feature.correctionsList"
            if settings.string("typoTolerance") == "off"
                && !["zZh", "cCh", "sSh", "enEng", "anAng"]
                    .iter()
                    .any(|pair| settings.bool(&format!("fuzzy.{pair}"))) =>
        {
            Some(Availability {
                note: "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單",
                place: Place::row(Page::Choosing, "typo"),
                link_title: "選字",
            })
        }
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

/// Commands that cannot be turned off, under a row of their own (「送出並換行」).
pub fn command_feature_row(title: impl Into<SharedString>, subtitle: impl Into<SharedString>) -> FeatureRow {
    FeatureRow { title: title.into(), subtitle: subtitle.into(), feature: None, commands: Vec::new(), anchor: None }
}

impl FeatureRow {
    pub fn commands(mut self, commands: impl IntoIterator<Item = Command>) -> Self {
        self.commands = commands.into_iter().collect();
        self
    }

    pub fn anchor(mut self, anchor: impl Into<SharedString>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }

    /// The row and, while the feature is on, its key rows.
    pub fn rows(self, cx: &App) -> Vec<AnyElement> {
        let on = self.feature.is_none_or(|key| app::settings(cx).bool(key));
        let available = self.feature.and_then(|key| availability(key, cx));
        let subtitle: SharedString = match (&available, on) {
            (Some(availability), _) => availability.note.into(),
            (None, false) if self.feature == Some("feature.zhuyinRowClick") => "關閉時，點注音列不做事".into(),
            (None, false) => match self.commands.first() {
                Some(&command) => {
                    format!("關閉時，{} 交給 App", compact(&CurrentKeys::new(cx).assigned(command))).into()
                }
                None => self.subtitle.clone(),
            },
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
            rows.extend(command_key_rows(&self.commands, cx).into_iter().map(IntoElement::into_any_element));
        }
        rows
    }
}

/// The indented 「快捷鍵」 rows of a setting: one per command, named after the command when there are several.
pub fn command_key_rows(commands: &[Command], cx: &App) -> Vec<Row> {
    let several = commands.len() > 1;
    commands
        .iter()
        .map(|&command| {
            row(
                SharedString::from(format!("keysrow-{}", command.id())),
                if several { command.label() } else { "快捷鍵" },
            )
            .indent(true)
            .anchor(command.keys_row())
            .trailing(shortcut_chip(command, cx))
        })
        .collect()
}

/// A row for one command's keys as a setting of its own (「標點全半形」「插入空白」 on 打字): title, description and
/// the keys.
pub fn command_row(command: Command, subtitle: impl Into<SharedString>, cx: &App) -> Row {
    row(SharedString::from(format!("command-{}", command.id())), command.label())
        .subtitle(subtitle)
        .anchor(command.keys_row())
        .trailing(shortcut_chip(command, cx))
}

/// A row that opens one of the page's sub-pages: title, description, the current state and ›.
pub fn drill_row(
    page: Page,
    subpage: &'static str,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> Row {
    let title: SharedString = title.into();
    let value: SharedString = value.into();
    let row = row(SharedString::from(format!("drill-{subpage}")), title).subtitle(subtitle);
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

/// The page's one link to the overview of all keys, at its foot.
pub fn keys_overview_link(cx: &App) -> impl IntoElement {
    place_link("keys-overview", "在快捷鍵總覽中查看全部按鍵", Place::page(Page::Keys), cx)
}
