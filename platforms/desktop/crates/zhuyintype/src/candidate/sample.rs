//! Example lists for previews and screenshots (macOS `PreviewScenes` and the UI renderer's panels), as
//! `zt_core::presentation::PreviewList`s so they page like the real list.

use zhuyin_core::api::Candidate;
use zt_core::presentation::PreviewList;

/// The reading being changed: 「曉明」.
pub const READING: &str = "ㄒㄧㄠˇ ㄇㄧㄥˊ";
/// The corrections' reading (ㄠˋ for the typed ㄠˇ).
pub const CORRECTED: &str = "ㄒㄧㄠˋ ㄇㄧㄥˊ";

/// The settings preview's candidates (macOS `PreviewScenes.words`).
pub const WORDS: &[&str] = &["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"];

/// The panel renders' candidates: two pages of nine.
pub const PANEL_WORDS: &[&str] =
    &["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "洺", "螟蛉", "明明", "名命"];

fn candidates(words: &[&str], corrections: &[(&str, &str)]) -> Vec<Candidate> {
    corrections
        .iter()
        .map(|(text, annotation)| Candidate { text: (*text).to_owned(), annotation: Some((*annotation).to_owned()) })
        .chain(words.iter().map(|text| Candidate { text: (*text).to_owned(), annotation: None }))
        .collect()
}

/// A list of `words` (with the corrections first when `tolerant`), `page_size` a page, `selected` highlighted.
pub fn list(
    words: &[&str],
    corrections: &[(&str, &str)],
    tolerant: bool,
    page_size: usize,
    selected: usize,
) -> PreviewList {
    let mut list = PreviewList::default();
    list.set_page_size(page_size);
    let entries = if tolerant { candidates(words, corrections) } else { candidates(words, &[]) };
    list.present(entries, Some(READING.to_owned()), selected);
    if tolerant {
        list.toggle_corrections();
    }
    list
}

/// The settings preview's corrections: 校名 and 孝明 first.
pub const PREVIEW_CORRECTIONS: &[(&str, &str)] = &[("校名", CORRECTED), ("孝明", CORRECTED)];
/// The panel renders' corrections: 校名 first.
pub const PANEL_CORRECTIONS: &[(&str, &str)] = &[("校名", CORRECTED)];
