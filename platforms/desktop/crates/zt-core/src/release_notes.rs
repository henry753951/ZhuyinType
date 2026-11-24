//! What each version changed, carried in the app so an update can say it without the network
//! (docs/product/DESIGN.md §3.6).  `build.rs` makes the JSON from docs/releases/*.md.
//!
//! The user's side of it, as two stored values beside the settings (no descriptor, so 「回復預設設定」 keeps them):
//! the version whose notes were last announced ([`SEEN_KEY`]) and the version before that ([`PREVIOUS_KEY`]), from
//! which 「更新內容」 lists everything that is new.

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

use crate::settings::Settings;
use crate::update::newer;

/// 更新後提示更新內容.
pub const SHOW_KEY: &str = "showUpdateNotes";
/// The version last announced (or installed first), and the one announced before it.
pub const SEEN_KEY: &str = "updateNotesSeenVersion";
pub const PREVIOUS_KEY: &str = "updateNotesPreviousVersion";
/// The releases' web page.
pub const RELEASES_URL: &str = "https://github.com/henry753951/ZhuyinType/releases";

/// This system's name in the notes' JSON, and as the user reads it.
const PLATFORM: &str = if cfg!(windows) { "windows" } else { "linux" };
const PLATFORM_NAME: &str = if cfg!(windows) { "Windows" } else { "Linux" };

/// One change.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Item {
    /// The name of a new feature; other changes have none.
    pub title: Option<String>,
    pub text: String,
    /// The systems it is about; empty: every one.
    platforms: Vec<String>,
}

impl Item {
    /// In one line of at most `limit` characters: the feature's name, or the first sentence.
    fn headline(&self, limit: usize) -> String {
        let line =
            self.title.as_deref().unwrap_or_else(|| self.text.split('。').next().unwrap_or_default()).trim().to_owned();
        if line.chars().count() <= limit {
            return line;
        }
        line.chars().take(limit - 1).chain(['…']).collect()
    }
}

/// A line's length in the notification.  A Windows toast shows four lines of about 22 characters and cuts the rest.
const HEADLINE_LIMIT: usize = if cfg!(windows) { 22 } else { 44 };

/// 新功能, 改進 or 修正.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

/// One version.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub version: String,
    pub date: String,
    pub sections: Vec<Section>,
}

#[derive(Deserialize)]
struct File {
    releases: Vec<Release>,
}

/// Every release this build knows, newest first, with only what concerns `platform`.
fn parse(json: &str, platform: &str) -> Vec<Release> {
    let file: File = serde_json::from_str(json).expect("build.rs writes valid release notes");
    let mut releases = file.releases;
    for release in &mut releases {
        for section in &mut release.sections {
            section
                .items
                .retain(|item| item.platforms.is_empty() || item.platforms.iter().any(|name| name == platform));
        }
        release.sections.retain(|section| !section.items.is_empty());
    }
    releases
}

fn all() -> &'static [Release] {
    static ALL: OnceLock<Vec<Release>> = OnceLock::new();
    ALL.get_or_init(|| parse(include_str!(concat!(env!("OUT_DIR"), "/release-notes.json")), PLATFORM))
}

/// The releases after `previous` up to `current`, newest first; only `current` when `previous` is not known (or not
/// earlier).  An earlier one that changed nothing here is left out.
fn between<'a>(releases: &'a [Release], previous: Option<&str>, current: &str) -> Vec<&'a Release> {
    let previous = previous.filter(|previous| newer(current, previous));
    releases
        .iter()
        .filter(|release| match previous {
            Some(previous) => newer(&release.version, previous) && !newer(&release.version, current),
            None => release.version == current,
        })
        .filter(|release| release.version == current || !release.sections.is_empty())
        .collect()
}

/// What 「更新內容」 lists: everything since the version before the one last announced.
#[must_use]
pub fn shown(settings: &Settings, current: &str) -> Vec<&'static Release> {
    between(all(), stored(settings, PREVIOUS_KEY).as_deref(), current)
}

/// The notification's text: the version, and the main changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    pub title: String,
    pub body: String,
}

/// How many changes the notification names.
const HIGHLIGHTS: usize = 3;

fn announcement(releases: &[&Release], current: &str) -> Announcement {
    let title = format!("ZhuyinType 已更新到 {current}");
    // New features first, then improvements, then fixes; the newest release's first within each.
    let mut lines: Vec<String> = Vec::new();
    let mut total = 0;
    for kind in ["新功能", "改進", "修正"] {
        for item in
            releases.iter().flat_map(|release| &release.sections).filter(|s| s.title == kind).flat_map(|s| &s.items)
        {
            total += 1;
            if lines.len() < HIGHLIGHTS {
                // A fix's first sentence says what went wrong.
                let label = if kind == "修正" { "修正：" } else { "" };
                lines.push(format!("{label}{}", item.headline(HEADLINE_LIMIT - label.chars().count())));
            }
        }
    }
    if total == 0 {
        return Announcement { title, body: format!("這一版沒有 {PLATFORM_NAME} 的變更。") };
    }
    if total > lines.len() {
        let rest = total - lines.len();
        let versions = releases.iter().filter(|release| !release.sections.is_empty()).count();
        lines.push(if versions > 1 {
            format!("{versions} 個版本，另有 {rest} 項")
        } else {
            format!("另有 {rest} 項")
        });
    }
    Announcement { title, body: lines.join("\n") }
}

fn stored(settings: &Settings, key: &str) -> Option<String> {
    settings.get(key).as_str().map(str::to_owned)
}

/// What to do when the app starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// Announced already.
    Nothing,
    /// A first install, an older version put back, or the announcement turned off: remember the version, say nothing.
    Remember,
    /// Announce; `previous` is the version updated from, if known.
    Announce { previous: Option<String> },
}

/// `seen`: the version last announced.  `used_before`: there are signs of earlier use, so without `seen` this is an
/// update from a version that did not keep it rather than a new install.
fn start(seen: Option<&str>, current: &str, used_before: bool, wanted: bool) -> Start {
    match seen {
        Some(seen) if seen == current => Start::Nothing,
        Some(seen) if !newer(current, seen) => Start::Remember,
        None if !used_before => Start::Remember,
        _ if !wanted => Start::Remember,
        seen => Start::Announce { previous: seen.map(str::to_owned) },
    }
}

/// Reads what was announced and remembers a version that needs no announcement.  Call once when the app that serves
/// the input method starts, before anything else stores settings.  The announcement, if there is one to make.
pub fn on_start(settings: &Settings, current: &str, used_before: bool) -> Option<Announcement> {
    let seen = stored(settings, SEEN_KEY);
    match start(seen.as_deref(), current, used_before, settings.bool(SHOW_KEY)) {
        Start::Nothing => None,
        Start::Remember => {
            remember(settings, current);
            None
        }
        Start::Announce { previous } => Some(announcement(&between(all(), previous.as_deref(), current), current)),
    }
}

/// `current` was announced (or needs no announcement).
pub fn remember(settings: &Settings, current: &str) {
    let seen = stored(settings, SEEN_KEY);
    if seen.as_deref() == Some(current) {
        return;
    }
    // From a version that did not keep it, or an older one put back: only `current`'s notes.
    let previous = seen.filter(|seen| newer(current, seen)).map_or(Value::Null, Value::from);
    for (key, value) in [(PREVIOUS_KEY, previous), (SEEN_KEY, Value::from(current))] {
        if let Err(error) = settings.set(key, value) {
            log::warn!("{key}: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str = r#"{"releases": [
      {"version": "0.8.0", "date": "d", "sections": [
        {"title": "新功能", "items": [{"title": "甲", "text": "說明。", "platforms": []}]},
        {"title": "修正", "items": [{"title": null, "text": "丙壞了。現在好了。", "platforms": ["windows"]},
                                    {"title": null, "text": "丁壞了。", "platforms": ["macos"]}]}]},
      {"version": "0.7.10", "date": "d", "sections": [
        {"title": "改進", "items": [{"title": null, "text": "乙更快。", "platforms": []},
                                    {"title": null, "text": "戊更快。", "platforms": ["linux", "windows"]}]}]},
      {"version": "0.7.9", "date": "d", "sections": []}]}"#;

    fn versions(releases: &[&Release]) -> Vec<String> {
        releases.iter().map(|release| release.version.clone()).collect()
    }

    #[test]
    fn this_version_has_notes() {
        let release = all().iter().find(|release| release.version == env!("CARGO_PKG_VERSION"));
        assert!(
            release.is_some_and(|release| !release.date.is_empty()),
            "docs/releases/v{}.md",
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn only_this_systems_changes_are_kept() {
        let windows = parse(NOTES, "windows");
        assert_eq!(windows[0].sections.len(), 2);
        assert_eq!(windows[0].sections[1].items.len(), 1);
        let linux = parse(NOTES, "linux");
        assert_eq!(linux[0].sections.len(), 1);
        assert_eq!(linux[1].sections[0].items.len(), 2);
    }

    #[test]
    fn an_update_over_several_versions_lists_each() {
        let notes = parse(NOTES, "windows");
        assert_eq!(versions(&between(&notes, Some("0.7.9"), "0.8.0")), ["0.8.0", "0.7.10"]);
        assert_eq!(versions(&between(&notes, Some("0.7.10"), "0.8.0")), ["0.8.0"]);
        // 0.7.9 changed nothing here.
        assert_eq!(versions(&between(&notes, Some("0.7.8"), "0.8.0")), ["0.8.0", "0.7.10"]);
        assert_eq!(versions(&between(&notes, Some("0.7.8"), "0.7.9")), ["0.7.9"]);
        assert_eq!(versions(&between(&notes, None, "0.8.0")), ["0.8.0"]);
        // A version this build has no notes for, or one put back.
        assert_eq!(versions(&between(&notes, Some("0.8.0"), "0.7.10")), ["0.7.10"]);
        assert!(between(&notes, None, "0.9.9").is_empty());
    }

    #[test]
    fn the_announcement_names_the_main_changes() {
        let notes = parse(NOTES, "windows");
        let one = announcement(&between(&notes, Some("0.7.10"), "0.8.0"), "0.8.0");
        assert_eq!(one.title, "ZhuyinType 已更新到 0.8.0");
        assert_eq!(one.body, "甲\n修正：丙壞了");
        let two = announcement(&between(&notes, Some("0.7.9"), "0.8.0"), "0.8.0");
        assert_eq!(two.body, "甲\n乙更快\n戊更快\n2 個版本，另有 1 項");
        let none = announcement(&[], "0.8.0");
        assert!(none.body.contains("沒有"));
    }

    #[test]
    fn a_headline_is_short() {
        let item = Item { title: None, text: "長".repeat(80), platforms: Vec::new() };
        assert_eq!(item.headline(22).chars().count(), 22);
        assert!(item.headline(22).ends_with('…'));
    }

    #[test]
    fn a_new_install_says_nothing_and_an_update_once() {
        assert_eq!(start(None, "0.8.0", false, true), Start::Remember);
        assert_eq!(start(None, "0.8.0", true, true), Start::Announce { previous: None });
        assert_eq!(start(Some("0.7.9"), "0.8.0", true, true), Start::Announce { previous: Some("0.7.9".into()) });
        assert_eq!(start(Some("0.8.0"), "0.8.0", true, true), Start::Nothing);
        assert_eq!(start(Some("0.7.9"), "0.8.0", true, false), Start::Remember);
        assert_eq!(start(Some("0.9.0"), "0.8.0", true, true), Start::Remember);
    }

    #[test]
    fn announcing_remembers_where_the_update_came_from() {
        let dir = std::env::temp_dir().join(format!("zt-notes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);
        let settings = Settings::open(Some(path));
        assert_eq!(on_start(&settings, "0.7.5", false), None);
        assert_eq!(stored(&settings, SEEN_KEY).as_deref(), Some("0.7.5"));
        // Not shown yet (the user never typed): asked again at the next start, from the same version.
        assert!(on_start(&settings, "0.8.0", true).is_some());
        assert!(on_start(&settings, "0.8.0", true).is_some());
        remember(&settings, "0.8.0");
        assert_eq!(on_start(&settings, "0.8.0", true), None);
        assert_eq!(stored(&settings, PREVIOUS_KEY).as_deref(), Some("0.7.5"));
        // 「回復預設設定」 does not bring the announcement back.
        settings.reset_all().unwrap();
        assert_eq!(on_start(&settings, "0.8.0", true), None);
        // Put back: only its own notes.
        assert_eq!(on_start(&settings, "0.7.5", true), None);
        assert_eq!(stored(&settings, PREVIOUS_KEY), None);
    }
}
