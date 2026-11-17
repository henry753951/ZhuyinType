//! What a screen reader says about the candidate window and the input mode.  The windows never take focus, so
//! Narrator, NVDA and JAWS do not follow them on their own: the window raises a UI Automation notification when a
//! list opens, its page turns, the highlight moves or a notice appears (`accessible`), with the same words as
//! VoiceOver on the Mac (`CandidateAnnouncer.swift`).  Plain Rust, tested on any system.

use zt_panel::model::{PanelModel, Row};

/// The part of a window an announcement depends on, to say only what changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shown {
    list: Option<ListShown>,
    notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ListShown {
    count: usize,
    page_size: usize,
    tolerant: Option<bool>,
    single: bool,
    reading: Option<String>,
    focused: Option<usize>,
    first: String,
    page: usize,
    selected: Option<usize>,
}

impl ListShown {
    fn of(model: &PanelModel) -> Option<Self> {
        model.has_list().then(|| Self {
            count: model.count,
            page_size: model.page_size,
            tolerant: model.tolerant,
            single: model.single,
            reading: model.reading.clone(),
            focused: model.focused_clause,
            first: if model.page == 0 {
                model.rows.first().map(|row| row.text.clone()).unwrap_or_default()
            } else {
                String::new()
            },
            page: model.page,
            selected: model.selected,
        })
    }

    /// The same list (another page or another highlighted row of it).
    fn same_list(&self, other: &Self) -> bool {
        self.count == other.count
            && self.page_size == other.page_size
            && self.tolerant == other.tolerant
            && self.single == other.single
            && self.reading == other.reading
            && self.focused == other.focused
            && (self.first == other.first || self.page != 0 || other.page != 0)
    }
}

impl Shown {
    pub(crate) fn of(model: &PanelModel) -> Self {
        Self { list: ListShown::of(model), notice: model.notice.clone().filter(|notice| !notice.is_empty()) }
    }
}

/// One candidate as its row reads: its key, its text and the corrected reading after it (「3，在」).
pub(crate) fn item(row: &Row, offset: usize) -> String {
    let key = row.key.map_or_else(|| (offset + 1).to_string(), String::from);
    [Some(key.as_str()), Some(row.text.as_str()), row.annotation.as_deref()]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("，")
}

/// The highlighted candidate: 「候選字 3，在」.
pub(crate) fn current(model: &PanelModel) -> Option<String> {
    let index = model.selected?;
    let row = model.rows.get(index)?;
    Some(format!("候選字 {}", item(row, index)))
}

/// What the window is, as its accessible name: the highlighted candidate, else the notice, else 「候選字」.
pub(crate) fn name(model: &PanelModel) -> String {
    current(model)
        .or_else(|| model.notice.clone().filter(|notice| !notice.is_empty()))
        .unwrap_or_else(|| "候選字".into())
}

/// What to say for `model` after `previous` (the default: nothing was shown), or None when nothing changed.
///
/// - A new list: 「ㄗㄞˋ，候選字清單，共 24 個，3 頁，候選字 1，在」 (修正的字 or 單字 for those lists);
/// - another page: 「第 2 頁，共 3 頁，候選字 1，在」;
/// - the highlight moved: 「候選字 3，在」;
/// - a new notice (「已加入自訂詞：王小明」) is said too, after the list's part.
pub(crate) fn announcement(model: &PanelModel, previous: &Shown) -> Option<String> {
    let now = Shown::of(model);
    if &now == previous {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(list) = &now.list
        && previous.list.as_ref() != Some(list)
        && let Some(current) = current(model)
    {
        match &previous.list {
            Some(before) if before.same_list(list) => {
                if before.page != list.page {
                    parts.push(format!("第 {} 頁，共 {} 頁", model.page + 1, model.page_count));
                }
            }
            _ => {
                if let Some(reading) = model.reading.as_deref().filter(|reading| !reading.is_empty()) {
                    parts.push(reading.to_owned());
                }
                let name = if model.tolerant == Some(true) {
                    "修正的字"
                } else if model.single {
                    "單字"
                } else {
                    "候選字清單"
                };
                parts.push(name.to_owned());
                parts.push(format!("共 {} 個", model.count));
                if model.page_count > 1 {
                    parts.push(format!("{} 頁", model.page_count));
                }
            }
        }
        parts.push(current);
    }
    if let Some(notice) = &now.notice
        && previous.notice.as_ref() != Some(notice)
    {
        parts.push(notice.clone());
    }
    (!parts.is_empty()).then(|| parts.join("，"))
}

/// 中/英 after the mode changed.
pub(crate) fn mode(english: bool) -> &'static str {
    if english { "英文" } else { "中文" }
}

/// The badge's word for a screen reader: 「中」 → 「中文」, 「全」 → 「全形標點」.
pub(crate) fn badge(text: &str) -> String {
    match text {
        "中" => "中文".into(),
        "英" => "英文".into(),
        "全" => "全形標點".into(),
        "半" => "半形標點".into(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: char, text: &str) -> Row {
        Row { key: Some(key), text: text.into(), ..Row::default() }
    }

    fn list(page: usize, selected: usize) -> PanelModel {
        let texts = [["在", "再", "載"], ["賳", "宰", "崽"]];
        PanelModel {
            rows: texts[page].iter().zip(['1', '2', '3']).map(|(text, key)| row(key, text)).collect(),
            count: 6,
            page,
            page_count: 2,
            page_size: 3,
            selected: Some(selected),
            reading: Some("ㄗㄞˋ".into()),
            ..PanelModel::default()
        }
    }

    #[test]
    fn a_new_list_says_what_it_is() {
        let opened = announcement(&list(0, 0), &Shown::default());
        assert_eq!(opened.as_deref(), Some("ㄗㄞˋ，候選字清單，共 6 個，2 頁，候選字 1，在"));
        assert_eq!(announcement(&list(0, 0), &Shown::of(&list(0, 0))), None);
    }

    #[test]
    fn the_highlight_and_the_page() {
        let moved = announcement(&list(0, 2), &Shown::of(&list(0, 0)));
        assert_eq!(moved.as_deref(), Some("候選字 3，載"));
        let paged = announcement(&list(1, 0), &Shown::of(&list(0, 2)));
        assert_eq!(paged.as_deref(), Some("第 2 頁，共 2 頁，候選字 1，賳"));
        // Back to the first page of the same list.
        let back = announcement(&list(0, 0), &Shown::of(&list(1, 0)));
        assert_eq!(back.as_deref(), Some("第 1 頁，共 2 頁，候選字 1，在"));
    }

    #[test]
    fn corrections_single_and_unkeyed() {
        let mut corrections = list(0, 0);
        corrections.tolerant = Some(true);
        corrections.page_count = 1;
        corrections.count = 1;
        corrections.rows = vec![Row { annotation: Some("ㄗㄞˋ".into()), ..row('1', "再") }];
        corrections.reading = None;
        let said = announcement(&corrections, &Shown::of(&list(0, 0)));
        assert_eq!(said.as_deref(), Some("修正的字，共 1 個，候選字 1，再，ㄗㄞˋ"));
        let mut single = list(0, 1);
        single.single = true;
        single.reading = None;
        assert_eq!(announcement(&single, &Shown::default()).as_deref(), Some("單字，共 6 個，2 頁，候選字 2，再"));
        let mut unkeyed = list(0, 1);
        unkeyed.rows[1].key = None;
        assert_eq!(current(&unkeyed).as_deref(), Some("候選字 2，再"));
    }

    #[test]
    fn another_list_of_the_same_size_is_new() {
        let mut other = list(0, 0);
        other.rows[0].text = "早".into();
        other.reading = Some("ㄗㄠˇ".into());
        assert!(
            announcement(&other, &Shown::of(&list(0, 0))).is_some_and(|said| said.starts_with("ㄗㄠˇ，候選字清單"))
        );
    }

    #[test]
    fn notices_and_nothing() {
        let empty = PanelModel::default();
        assert_eq!(announcement(&empty, &Shown::default()), None);
        assert_eq!(name(&empty), "候選字");
        let notice = PanelModel { notice: Some("已加入自訂詞：王小明".into()), ..PanelModel::default() };
        assert_eq!(announcement(&notice, &Shown::default()).as_deref(), Some("已加入自訂詞：王小明"));
        assert_eq!(announcement(&notice, &Shown::of(&notice)), None);
        assert_eq!(name(&notice), "已加入自訂詞：王小明");
        assert_eq!(name(&list(0, 1)), "候選字 2，再");
    }

    #[test]
    fn modes() {
        assert_eq!((mode(false), mode(true)), ("中文", "英文"));
        assert_eq!([badge("中"), badge("英"), badge("全"), badge("半")], ["中文", "英文", "全形標點", "半形標點"]);
    }
}
