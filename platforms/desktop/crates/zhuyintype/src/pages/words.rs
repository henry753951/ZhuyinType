//! 「字詞」 page (macOS `WordsPane`, `WordListPage`, `ListControllers.swift`): 記住選過的字詞 with its ⓘ card, and the
//! two list pages 記住的字詞 (learned words and the words set to 不記住) and 自訂詞 (user phrases, with the add and
//! edit sheet).  Personal data comes from the service through `data`.

pub(crate) mod data;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement, PathPromptOptions,
    Render, SharedString, Styled, Subscription, WeakEntity, Window,
};
use zt_core::engine::Phrase;

use super::Page;
use crate::app::{self, Place};
use crate::theme::size;
use crate::ui::alert::{AlertButton, alert, notice};
use crate::ui::content::{FACT_GAP as FIELD_GAP, chip, chips, fact_row, facts, labelled, under_fields};
use crate::ui::list::{ListConfig, ListItem, ListSort, ListView, MenuEntry, grouped};
use crate::ui::{self, v};

const LEARNED: &str = "learnedWords";
const PHRASES: &str = "phrases";
const EXCLUDED_PREFIX: &str = "excluded\t";

/// The add or edit sheet of 自訂詞 while it is open.
struct PhraseSheet {
    word: Entity<InputState>,
    reading: Entity<InputState>,
    editing: Option<ListItem>,
    _events: [Subscription; 2],
}

pub struct WordsPage {
    learned: Entity<ListView>,
    phrases: Entity<ListView>,
    counts: Option<(usize, usize)>,
    seen: Option<(u64, bool)>,
    sheet: Option<PhraseSheet>,
    /// Screenshot states still to open (`ZT_DEMO`).
    pending_demo: bool,
}

impl WordsPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        data::install_sample(cx);
        let page = cx.entity().downgrade();
        let learned = cx.new(|cx| {
            ListView::new(
                ListConfig {
                    id: "learned",
                    noun: "字詞",
                    sorts: vec![ListSort::Count, ListSort::Reading, ListSort::Word],
                    empty_title: "還沒有記住的字詞".into(),
                    empty_text: "從候選字清單選字後，ZhuyinType 會記住你的選擇，下次排在前面。".into(),
                },
                window,
                cx,
            )
        });
        let phrases = cx.new(|cx| {
            ListView::new(
                ListConfig {
                    id: "phrases",
                    noun: "自訂詞",
                    sorts: vec![ListSort::Reading, ListSort::Word],
                    empty_title: "還沒有自訂詞".into(),
                    empty_text: "按 ＋ 加入，或打字時選取字詞再按 Enter。自訂詞在選字時優先列出。".into(),
                },
                window,
                cx,
            )
        });
        learned.update(cx, |list, _| configure_learned(list, page.clone()));
        phrases.update(cx, |list, _| configure_phrases(list, page.clone()));
        Self { learned, phrases, counts: None, seen: None, sheet: None, pending_demo: true }
    }

    /// Reads the lists again when personal data changed.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let now = (data::revision(cx), data::available(cx));
        if self.seen == Some(now) {
            return;
        }
        self.seen = Some(now);
        self.reload(None, None, cx);
    }

    fn reload(&mut self, select_phrase: Option<SharedString>, select_learned: Option<BTreeSet<SharedString>>, cx: &mut Context<Self>) {
        let learned = data::learned_entries(cx);
        let excluded = data::excluded_entries(cx);
        let phrases = data::user_phrases(cx);
        self.counts = data::available(cx).then_some((learned.len(), phrases.len()));
        let rows = learned_rows(&learned, &excluded);
        let available = data::available(cx);
        self.learned.update(cx, |list, cx| list.set_items(rows, select_learned, cx));
        let rows = phrases.iter().map(phrase_row).collect();
        let footer = phrases_footer(cx);
        self.phrases.update(cx, |list, cx| {
            list.set_add_enabled(available);
            list.set_footer(Some(footer));
            list.set_items(rows, select_phrase.map(|id| BTreeSet::from([id])), cx);
        });
        self.learned.update(cx, |list, _| {
            list.set_footer(Some("選過的字詞下次排在前面；設為「不記住」的字詞之後不再學。".into()));
        });
    }

    /// Opens the screenshot states `ZT_DEMO` asks for, once.
    fn open_demo_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The states are for the list pages: wait for the first frame of one.
        let list = match app::nav(cx).subpage.as_deref() {
            Some(LEARNED) => self.learned.clone(),
            Some(PHRASES) => self.phrases.clone(),
            _ => return,
        };
        if !std::mem::take(&mut self.pending_demo) {
            return;
        }
        if let Some(query) = data::demo("search") {
            list.update(cx, |list, cx| list.set_query(&query, window, cx));
        }
        if data::demo("select").is_some() {
            list.update(cx, |list, cx| {
                let ids: Vec<SharedString> = list.items().iter().skip(1).take(2).map(|item| item.id.clone()).collect();
                list.select(ids, cx);
            });
        }
        let page = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            if data::demo("add").is_some() {
                open_sheet(&page, "", "", None, window, cx);
            }
            if data::demo("edit").is_some() {
                let item = ListItem::new("ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ\t王小明", "王小明").detail("ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ");
                open_sheet(&page, "王小明", "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ", Some(item), window, cx);
            }
            if data::demo("invalid").is_some() {
                show_invalid(&page, "台南市政府大樓前", "ㄊㄞˊ", None, window, cx);
            }
            if data::demo("clear-all").is_some() {
                confirm_clear_all(&page, window, cx);
            }
        });
    }

    fn main(&self, cx: &mut Context<Self>) -> AnyElement {
        let counts = self.counts;
        let value = |count: usize| format!("{} 個", grouped(count));
        let phrases_subtitle = if data::key_text("selectLeft", cx).is_some() {
            "選字時優先列出；打字時也能選取後加入"
        } else {
            "選字時優先列出"
        };
        v().gap(size::SECTION_GAP)
            .child(
                ui::section("")
                    .row(ui::setting_row("learningEnabled", cx).info("記住的字詞", |_, cx| learned_info(cx)))
                    .row(
                        ui::row("drill-learned", "記住的字詞")
                            .subtitle("檢視、移除、匯出或匯入")
                            .anchor(LEARNED)
                            .when_some(counts, |row, (learned, _)| row.value(value(learned)))
                            .on_click(|_, _, cx| app::go(Place::subpage(Page::Words, LEARNED), cx)),
                    ),
            )
            .child(
                ui::section("").row(
                    ui::row("drill-phrases", "自訂詞")
                        .subtitle(phrases_subtitle)
                        .anchor(PHRASES)
                        .when_some(counts, |row, (_, phrases)| row.value(value(phrases)))
                        .on_click(|_, _, cx| app::go(Place::subpage(Page::Words, PHRASES), cx)),
                ),
            )
            .into_any_element()
    }

    fn commit_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet.take() else { return };
        let word = sheet.word.read(cx).value().trim().to_owned();
        let reading = sheet.reading.read(cx).value().split_whitespace().collect::<Vec<_>>().join(" ");
        let result = match &sheet.editing {
            Some(old) => data::edit_phrase(&old.title, old.detail.as_deref().unwrap_or_default(), &word, &reading, cx),
            None => data::add_phrase(&word, &reading, cx),
        };
        match result {
            Ok(()) => {
                self.seen = Some((data::revision(cx), data::available(cx)));
                self.reload(Some(format!("{reading}\t{word}").into()), None, cx);
            }
            Err(error) => {
                log::info!("adding phrase {word} {reading}: {error:#}");
                let page = cx.entity().downgrade();
                let editing = sheet.editing.clone();
                window.defer(cx, move |window, cx| show_invalid(&page, &word, &reading, editing, window, cx));
            }
        }
    }
}

impl Render for WordsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(cx);
        self.open_demo_states(window, cx);
        match app::nav(cx).subpage.as_deref() {
            Some(LEARNED) => self.learned.clone().into_any_element(),
            Some(PHRASES) => self.phrases.clone().into_any_element(),
            _ => self.main(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    match id {
        LEARNED => Some("記住的字詞"),
        PHRASES => Some("自訂詞"),
        _ => None,
    }
}

// MARK: - Rows

fn phrase_row(phrase: &Phrase) -> ListItem {
    ListItem::new(format!("{}\t{}", phrase.reading, phrase.word), phrase.word.clone()).detail(phrase.reading.clone())
}

/// Learned words with their 常用程度 (1–5 on a log scale of the top count), then the 不記住 ones.
fn learned_rows(learned: &[Phrase], excluded: &[Phrase]) -> Vec<ListItem> {
    let top = learned.iter().map(|entry| entry.count).fold(1.0f64, f64::max).ln_1p();
    let mut rows: Vec<ListItem> = learned
        .iter()
        .map(|entry| {
            let level = (entry.count.max(0.).ln_1p() / top.max(0.0001) * 5.).ceil().clamp(1., 5.) as u8;
            ListItem::new(format!("{}\t{}", entry.reading, entry.word), entry.word.clone())
                .detail(entry.reading.clone())
                .count(entry.count)
                .level(level)
        })
        .collect();
    rows.extend(excluded.iter().map(|entry| {
        ListItem::new(format!("{EXCLUDED_PREFIX}{}\t{}", entry.reading, entry.word), entry.word.clone())
            .detail(entry.reading.clone())
            .meta("不記住")
            .count(-1.)
            .marked(true)
    }));
    rows
}

fn phrases_footer(cx: &App) -> SharedString {
    match (data::key_text("selectLeft", cx), data::key_text("selectRight", cx)) {
        (Some(left), Some(right)) => format!("自訂詞在選字時優先列出；打字時用 {left} {right} 選取字詞，再按 Enter 也能加入。").into(),
        _ => "自訂詞在選字時優先列出。".into(),
    }
}

fn reading_of(item: &ListItem) -> String {
    item.detail.as_deref().unwrap_or_default().to_owned()
}

/// Runs `change` for each row, then reloads the page.
fn apply(page: &WeakEntity<WordsPage>, rows: &[ListItem], cx: &mut App, change: impl Fn(&ListItem, &mut App) -> anyhow::Result<()>) {
    for row in rows {
        if let Err(error) = change(row, cx) {
            log::warn!("changing {}: {error:#}", row.title);
        }
    }
    page.update(cx, |page, cx| {
        page.seen = None;
        page.refresh(cx);
    })
    .ok();
}

// MARK: - 記住的字詞

fn configure_learned(list: &mut ListView, page: WeakEntity<WordsPage>) {
    let remove_page = page.clone();
    // Removing a learned word forgets it; removing a 不記住 one lets it be learned again.
    list.on_remove(move |rows, _, cx| {
        apply(&remove_page, &rows, cx, |row, cx| {
            if row.marked {
                data::include_learned(&row.title, &reading_of(row), cx)
            } else {
                data::forget_learned(&row.title, &reading_of(row), cx)
            }
        });
    });
    let more_page = page.clone();
    list.more_menu(move |selected, cx| {
        let chosen: Vec<ListItem> = selected.into_iter().filter(|row| !row.marked).collect();
        let available = data::available(cx);
        let (exclude_page, export_page, import_page, clear_page) =
            (more_page.clone(), more_page.clone(), more_page.clone(), more_page.clone());
        let any_chosen = !chosen.is_empty();
        vec![
            MenuEntry::new("不再記住所選字詞", move |_, cx| exclude(&exclude_page, &chosen, cx)).enabled(available && any_chosen),
            MenuEntry::new("匯出…", move |window, cx| export_data(&export_page, window, cx)).enabled(available),
            MenuEntry::new("匯入…", move |window, cx| import_data(&import_page, window, cx)).enabled(available),
            MenuEntry::new("全部清除…", move |window, cx| confirm_clear_all(&clear_page, window, cx))
                .destructive()
                .enabled(available),
        ]
    });
    let row_page = page;
    list.row_menu(move |rows| {
        let learned: Vec<ListItem> = rows.into_iter().filter(|row| !row.marked).collect();
        if learned.is_empty() {
            return Vec::new();
        }
        let page = row_page.clone();
        vec![MenuEntry::new("不再記住", move |_, cx| exclude(&page, &learned, cx))]
    });
}

fn exclude(page: &WeakEntity<WordsPage>, rows: &[ListItem], cx: &mut App) {
    apply(page, rows, cx, |row, cx| data::exclude_learned(&row.title, &reading_of(row), cx));
}

fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn export_data(page: &WeakEntity<WordsPage>, window: &mut Window, cx: &mut App) {
    let chosen = cx.prompt_for_new_path(&home(), Some("ZhuyinType 個人資料.txt"));
    let _ = page;
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(path))) = chosen.await else { return };
            cx.update(|window, cx| {
                if let Err(error) = data::export_personal(&path, cx) {
                    notice("無法匯出", format!("{error:#}"), window, cx);
                }
            })
            .ok();
        })
        .detach();
}

fn import_data(page: &WeakEntity<WordsPage>, window: &mut Window, cx: &mut App) {
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("匯入".into()),
    });
    let page = page.clone();
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = chosen.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            cx.update(|window, cx| {
                match data::import_personal(&path, cx) {
                    Ok(_) => {
                        page.update(cx, |page, cx| {
                            page.seen = None;
                            page.refresh(cx);
                        })
                        .ok();
                    }
                    Err(error) => {
                        log::info!("importing {}: {error:#}", path.display());
                        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                        notice(format!("無法匯入「{name}」"), "這個檔案不是 ZhuyinType 匯出的個人資料。", window, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
}

fn confirm_clear_all(page: &WeakEntity<WordsPage>, window: &mut Window, cx: &mut App) {
    let page = page.clone();
    alert("要清除所有記住的字詞嗎？")
        .message("清除後無法復原。不記住的字詞會保留。")
        .buttons([
            AlertButton::cancel(),
            AlertButton::new("清除", move |_, cx| {
                if let Err(error) = data::forget_all_learned(cx) {
                    log::warn!("forgetting learned words: {error:#}");
                }
                page.update(cx, |page, cx| {
                    page.seen = None;
                    page.refresh(cx);
                })
                .ok();
            })
            .destructive(),
        ])
        .show(window, cx);
}

/// What is remembered and for how long (docs/product/DESIGN.md §2.5).
pub(crate) fn learned_info(cx: &App) -> AnyElement {
    facts([
        fact_row(IconName::CircleCheck, "選過的字詞：同樣的讀音直接採用", cx).into_any_element(),
        fact_row(IconName::Link, "常接在一起的詞：送出時學習，選字時參考", cx).into_any_element(),
        fact_row(IconName::Keyboard, "按錯習慣：常按錯的鍵更容易修正", cx).into_any_element(),
        fact_row(IconName::Clock, "近期與長期分開記，半衰期 8 小時與 90 天", cx).into_any_element(),
        fact_row(IconName::Lock, "密碼欄位中暫停學習", cx).into_any_element(),
        fact_row(IconName::Ban, "設為「不記住」的字詞之後不再學", cx).into_any_element(),
    ])
    .into_any_element()
}

// MARK: - 自訂詞

fn configure_phrases(list: &mut ListView, page: WeakEntity<WordsPage>) {
    let add_page = page.clone();
    list.on_add(move |window, cx| open_sheet(&add_page, "", "", None, window, cx));
    let edit_page = page.clone();
    list.on_edit(move |item, window, cx| {
        let reading = reading_of(&item);
        open_sheet(&edit_page, &item.title.clone(), &reading, Some(item), window, cx);
    });
    list.on_remove(move |rows, _, cx| {
        apply(&page, &rows, cx, |row, cx| data::remove_phrase(&row.title, &reading_of(row), cx));
    });
}

/// Return in a field of the sheet: the same as its 加入 or 儲存.
fn submit_sheet(page: &mut WordsPage, window: &mut Window, cx: &mut Context<WordsPage>) {
    if page.sheet.is_some() {
        window.close_dialog(cx);
        page.commit_sheet(window, cx);
    }
}

/// The sheet that adds or edits a phrase: 詞 and 讀音, the lexicon's readings of the word as suggestions.
fn open_sheet(page: &WeakEntity<WordsPage>, word: &str, reading: &str, editing: Option<ListItem>, window: &mut Window, cx: &mut App) {
    if !data::available(cx) {
        return;
    }
    let Some(entity) = page.upgrade() else { return };
    let word_input = cx.new(|cx| InputState::new(window, cx).placeholder("例如：王小明").default_value(word.to_owned()));
    let reading_input =
        cx.new(|cx| InputState::new(window, cx).placeholder("ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ").default_value(reading.to_owned()));
    let adding = editing.is_none();
    entity.update(cx, |page, cx| {
        let target = reading_input.clone();
        let word_events = cx.subscribe_in(&word_input, window, move |page, input, event, window, cx| match event {
            InputEvent::Change => {
                let word = input.read(cx).value().trim().to_owned();
                let readings = data::readings(&word, cx);
                let current = target.read(cx).value().to_string();
                if let Some(first) = readings.first()
                    && !readings.contains(&current)
                {
                    let first = first.clone();
                    target.update(cx, |input, cx| input.set_value(first, window, cx));
                }
            }
            InputEvent::PressEnter { .. } => submit_sheet(page, window, cx),
            _ => {}
        });
        let reading_events = cx.subscribe_in(&reading_input, window, |page, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                submit_sheet(page, window, cx);
            }
        });
        page.sheet = Some(PhraseSheet {
            word: word_input.clone(),
            reading: reading_input.clone(),
            editing,
            _events: [word_events, reading_events],
        });
    });
    let commit_page = page.clone();
    let first_frame = Rc::new(Cell::new(true));
    let (fields_word, fields_reading) = (word_input.clone(), reading_input.clone());
    alert(if adding { "加入自訂詞" } else { "編輯自訂詞" })
        .message("自訂詞在選字時優先列出。")
        .width(420.)
        .content(move |window, cx| {
            if first_frame.replace(false) {
                fields_word.focus_handle(cx).focus(window, cx);
            }
            sheet_fields(&fields_word, &fields_reading, window, cx)
        })
        .buttons([
            AlertButton::cancel(),
            AlertButton::new(if adding { "加入" } else { "儲存" }, move |window, cx| {
                commit_page.update(cx, |page, cx| page.commit_sheet(window, cx)).ok();
            })
            .default(),
        ])
        .show(window, cx);
}

fn sheet_fields(word: &Entity<InputState>, reading: &Entity<InputState>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let readings = data::readings(word.read(cx).value().trim(), cx);
    let current = reading.read(cx).value().to_string();
    let chips_row = (readings.len() > 1).then(|| {
        under_fields(chips(readings.into_iter().enumerate().map(|(index, option)| {
            let target = reading.clone();
            let value = option.clone();
            chip(
                SharedString::from(format!("reading-{index}")),
                option.clone(),
                option == current,
                move |_, window, cx| {
                    let value = value.clone();
                    target.update(cx, |input, cx| input.set_value(value, window, cx));
                },
                cx,
            )
            .into_any_element()
        })))
    });
    v().gap(FIELD_GAP)
        .child(labelled("詞：", Input::new(word), cx))
        .child(labelled("讀音：", Input::new(reading), cx))
        .children(chips_row)
        .into_any_element()
}

/// 「無法加入」 with 修改 (back to the sheet with the same text) and 取消.
fn show_invalid(page: &WeakEntity<WordsPage>, word: &str, reading: &str, editing: Option<ListItem>, window: &mut Window, cx: &mut App) {
    let page = page.clone();
    let (word, reading) = (word.to_owned(), reading.to_owned());
    let title = format!("無法加入「{word}」");
    alert(title)
        .message("詞需為 1 到 6 個字，讀音需為每個字一個注音，以空白分隔。")
        .buttons([
            AlertButton::cancel(),
            AlertButton::new("修改", move |window, cx| {
                let (page, word, reading, editing) = (page.clone(), word.clone(), reading.clone(), editing.clone());
                window.defer(cx, move |window, cx| open_sheet(&page, &word, &reading, editing, window, cx));
            })
            .default(),
        ])
        .show(window, cx);
}
