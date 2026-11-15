//! 授權與資料來源: `resources/sources.json` filtered to this platform (macOS `SourcesPane`, Qt `LegalDocuments`), and
//! the licence texts it names.  The texts the repository holds are built into the binary; files made at packaging
//! time (`Rust-crates.txt`) are read from a `Licenses` folder installed next to the program, and model notices from
//! the engine's data folder, when they are there.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The licence texts built in, by the name `sources.json` uses.
const EMBEDDED: &[(&str, &str)] = &[
    ("AGPL-3.0.txt", include_str!("../../../../../../../LICENSE")),
    ("NOTICE.txt", include_str!("../../../../../../../NOTICE")),
    ("Apache-2.0.txt", include_str!("../../../../../../macos/Resources/Licenses/Apache-2.0.txt")),
    ("CC-BY-4.0.txt", include_str!("../../../../../../macos/Resources/Licenses/CC-BY-4.0.txt")),
    ("CC-BY-SA-4.0.txt", include_str!("../../../../../../macos/Resources/Licenses/CC-BY-SA-4.0.txt")),
    ("CC0-1.0.txt", include_str!("../../../../../../macos/Resources/Licenses/CC0-1.0.txt")),
    ("LEXICON.txt", include_str!("../../../../../../macos/Resources/Licenses/LEXICON.txt")),
    ("McBopomofo-MIT.txt", include_str!("../../../../../../macos/Resources/Licenses/McBopomofo-MIT.txt")),
    ("OGDL-1.0.txt", include_str!("../../../../../../macos/Resources/Licenses/OGDL-1.0.txt")),
    ("Reranker-NOTICE.md", include_str!("../../../../../../macos/Resources/Licenses/Reranker-NOTICE.md")),
    ("Unicode-3.0.txt", include_str!("../../../../../../macos/Resources/Licenses/Unicode-3.0.txt")),
    ("libtabe-COPYING.txt", include_str!("../../../../../../macos/Resources/Licenses/libtabe-COPYING.txt")),
    ("wordfreq-NOTICE.txt", include_str!("../../../../../../macos/Resources/Licenses/wordfreq-NOTICE.txt")),
    ("nlohmann-json.txt", include_str!("../../../../../../linux/LICENSES/nlohmann-json.txt")),
];

const SOURCES: &str = include_str!("../../../../../../../resources/sources.json");

/// The largest text the viewer opens (macOS: 8 MB).
const MAX_TEXT: u64 = 8 << 20;

#[derive(Debug, Deserialize)]
struct SourceList {
    version: u32,
    groups: Vec<GroupRecord>,
    entries: Vec<EntryRecord>,
}

#[derive(Debug, Deserialize)]
struct GroupRecord {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryRecord {
    id: String,
    group: String,
    name: String,
    #[serde(rename = "use")]
    usage: String,
    distribution: String,
    license: String,
    copyright: Option<String>,
    url: Option<String>,
    version: Option<String>,
    changes: Option<String>,
    #[serde(default)]
    license_files: Vec<String>,
    #[serde(default)]
    notices: Vec<String>,
    platforms: Option<Vec<String>>,
}

/// A group of entries (程式庫, 詞庫與資料集 …).
#[derive(Clone, Debug)]
pub struct Group {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// One program, data set or model.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// 「用途・授權・隨 App 提供」.
    pub summary: String,
    pub copyright: Option<String>,
    pub version: Option<String>,
    pub changes: Option<String>,
    pub url: Option<String>,
    pub files: Vec<String>,
}

/// The platform `sources.json` names this build by.
fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

fn distribution(value: &str) -> &'static str {
    match value {
        "bundled" => "隨 App 提供",
        "training" => "用於模型訓練",
        _ => "僅用於評測",
    }
}

/// A file name `sources.json` may name: relative, no parent steps.
fn safe(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('/') && !name.contains('\\') && !name.contains(':') && !name.split('/').any(|part| part == "..")
}

/// The groups shown on this platform, or None when the list cannot be read.
///
/// Qt 6 is left out: this build draws its windows with GPUI, whose crates are in the Rust crate list.
pub fn groups(data_dir: Option<&Path>) -> Option<Vec<Group>> {
    let list: SourceList = serde_json::from_str(SOURCES).ok()?;
    if list.version != 1 {
        return None;
    }
    let shown = list.groups.iter().filter_map(|group| {
        let entries: Vec<Entry> = list
            .entries
            .iter()
            .filter(|entry| entry.group == group.id && entry.id != "qt")
            .filter(|entry| entry.platforms.as_ref().is_none_or(|platforms| platforms.iter().any(|p| p == platform())))
            .map(|entry| Entry {
                id: entry.id.clone(),
                name: entry.name.clone(),
                summary: format!("{}・{}・{}", entry.usage, entry.license, distribution(&entry.distribution)),
                copyright: entry.copyright.clone(),
                version: entry.version.clone(),
                changes: entry.changes.clone(),
                url: entry.url.clone(),
                files: entry
                    .license_files
                    .iter()
                    .filter(|name| safe(name) && licence(name).is_some())
                    .chain(entry.notices.iter().filter(|name| safe(name) && notice_path(name, data_dir).is_some()))
                    .cloned()
                    .collect(),
            })
            .collect();
        (!entries.is_empty()).then(|| Group { title: group.title.clone(), entries })
    });
    Some(shown.collect())
}

/// Where a licence file installed next to the program is (`Licenses/` beside it, or `share/zhuyintype/Licenses`).
fn installed(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("Licenses"), dir.join("../share/zhuyintype/Licenses")]
        .into_iter()
        .map(|folder| folder.join(name))
        .find(|path| path.is_file())
}

/// A licence text by name: built in, or installed with the program.
pub fn licence(name: &str) -> Option<Document> {
    if let Some((_, text)) = EMBEDDED.iter().find(|(file, _)| *file == name) {
        return Some(Document { name: name.to_owned(), text: Some((*text).to_owned()), path: None });
    }
    installed(name).map(|path| Document::read(name, path))
}

fn notice_path(name: &str, data_dir: Option<&Path>) -> Option<PathBuf> {
    data_dir.map(|dir| dir.join(name)).filter(|path| path.is_file())
}

/// A file an entry names: a licence, or a model notice in the engine's data folder.
pub fn document(name: &str, data_dir: Option<&Path>) -> Option<Document> {
    licence(name).or_else(|| notice_path(name, data_dir).map(|path| Document::read(name, path)))
}

/// A file to read in the app, or (an archive, a data list) to show in the file manager.
#[derive(Clone, Debug)]
pub struct Document {
    pub name: String,
    /// The text, when it is text to read here.
    pub text: Option<String>,
    /// The file on disk, when there is one.
    pub path: Option<PathBuf>,
}

impl Document {
    fn read(name: &str, path: PathBuf) -> Self {
        let text = readable(name)
            .then(|| std::fs::metadata(&path).ok().filter(|meta| meta.len() <= MAX_TEXT).and_then(|_| std::fs::read_to_string(&path).ok()))
            .flatten();
        Self { name: name.to_owned(), text, path: Some(path) }
    }

    /// The text as shown: Markdown with its inline marks (`code`, **bold**, [links](…)) read as plain text, the way
    /// macOS reads it (inline only, whitespace kept); other files as they are.
    pub fn shown_text(&self) -> Option<String> {
        let text = self.text.as_ref()?;
        if !self.markdown() {
            return Some(text.clone());
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(start) = rest.find('[') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            match after.find("](").and_then(|close| after[close + 2..].find(')').map(|end| (close, close + 2 + end))) {
                Some((close, end)) if !after[..close].contains('\n') => {
                    out.push_str(&after[..close]);
                    rest = &after[end + 1..];
                }
                _ => {
                    out.push('[');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        Some(out.replace("**", "").replace('`', ""))
    }

    /// Markdown is shown in the UI font; licences monospaced.
    pub fn markdown(&self) -> bool {
        self.name.to_lowercase().ends_with(".md")
    }

    /// The file's own name (without the folder `sources.json` puts it in).
    pub fn file_name(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }
}

/// Text files the viewer opens: `.txt`, `.md`, and licences without an extension.
fn readable(name: &str) -> bool {
    let lower = name.to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    file.ends_with(".txt") || file.ends_with(".md") || !file.contains('.') || file.ends_with("license")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_parse_and_name_built_in_files() {
        let groups = groups(None).expect("sources.json parses");
        assert!(groups.iter().any(|group| group.entries.iter().any(|entry| entry.id == "zhuyintype")));
        assert!(!groups.iter().any(|group| group.entries.iter().any(|entry| entry.id == "qt" || entry.id == "sparkle")));
        let app = groups.iter().flat_map(|group| &group.entries).find(|entry| entry.id == "zhuyintype").unwrap();
        assert_eq!(app.files, ["AGPL-3.0.txt", "NOTICE.txt"]);
    }

    #[test]
    fn unsafe_names_are_refused() {
        assert!(!safe("../secret"));
        assert!(!safe("/etc/passwd"));
        assert!(safe("lm_chat/NOTICE.md"));
    }
}
