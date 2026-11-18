//! A list page (macOS `ListPage.swift`): 記住的字詞, 自訂詞, 在候選字視窗顯示組字的 App.  A toolbar row (search,
//! count, sort, ＋ or ⋯), the rows on one card, and a footer; the page fills the window and the card scrolls inside it,
//! virtualised so thousands of rows stay smooth.  Rows show the item, its reading in secondary text and quiet details
//! on the right; 編輯 and 刪除 appear over the details on hover (no layout change), and through selection with
//! Delete and the context menu.  Click selects one row, Ctrl+click adds or removes one, Shift+click a range.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, window_paddings};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, Animation, AnimationExt as _, AnyElement, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement, Pixels, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement, Styled, Subscription, UniformListScrollHandle, Window, div, px,
    uniform_list,
};

use super::controls::{IconButtonKind, icon_button, popup_button};
use super::search::search_field;
use super::{h, v};
use crate::scroll::SmoothScroll;
use crate::theme::{self, motion, size, space, text, weight};

const TOOLBAR_H: Pixels = size::H_MD;
const TOOLBAR_GAP: Pixels = space::S12;
const ROW_H: Pixels = size::ROW_MIN_SINGLE;
/// The footer's line under the card.
const FOOTER_H: Pixels = px(28.);
const SEARCH_MAX_W: Pixels = px(260.);
const SEARCH_MIN_W: Pixels = px(96.);

/// The room on the right of a row for 編輯 and 刪除 (or 刪除 alone).
const ACTIONS_W: Pixels = px(96.);
const ACTIONS_W_REMOVE: Pixels = px(52.);

/// An empty state's explanation wraps at this width.
const EMPTY_TEXT_MAX_W: Pixels = px(420.);

/// How a list can be ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSort {
    /// 常用程度: larger counts first.
    Count,
    /// 注音.
    Reading,
    /// 字詞.
    Word,
    /// 名稱.
    Name,
}

impl ListSort {
    pub fn title(self) -> &'static str {
        match self {
            ListSort::Count => "常用程度",
            ListSort::Reading => "注音",
            ListSort::Word => "字詞",
            ListSort::Name => "名稱",
        }
    }
}

/// One row.
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub id: SharedString,
    pub title: SharedString,
    /// The reading, or a program's file name: secondary text after the title.
    pub detail: Option<SharedString>,
    /// Quiet text on the right (「不記住」).
    pub meta: Option<SharedString>,
    /// Sorting by count: larger first.
    pub count: f64,
    /// How much a learned word is used, 1–5, drawn as a quiet meter.
    pub level: Option<u8>,
    /// A secondary entry (a word set to 不記住): title in secondary text.
    pub marked: bool,
}

impl ListItem {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), detail: None, meta: None, count: 0., level: None, marked: false }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    pub fn count(mut self, count: f64) -> Self {
        self.count = count;
        self
    }

    pub fn level(mut self, level: u8) -> Self {
        self.level = Some(level);
        self
    }

    pub fn marked(mut self, marked: bool) -> Self {
        self.marked = marked;
        self
    }

    fn key(&self, sort: ListSort) -> &str {
        match sort {
            ListSort::Count | ListSort::Word | ListSort::Name => &self.title,
            ListSort::Reading => self.detail.as_deref().unwrap_or_default(),
        }
    }

    fn matches(&self, needle: &str) -> bool {
        [Some(&self.title), self.detail.as_ref(), self.meta.as_ref()]
            .into_iter()
            .flatten()
            .any(|text| text.to_lowercase().contains(needle))
    }
}

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// An entry of the ⋯ menu or a row's context menu.
#[derive(Clone)]
pub struct MenuEntry {
    pub title: SharedString,
    pub destructive: bool,
    pub enabled: bool,
    pub action: Action,
}

impl MenuEntry {
    pub fn new(title: impl Into<SharedString>, action: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self { title: title.into(), destructive: false, enabled: true, action: Rc::new(action) }
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// What a list is: its noun (「字詞」 in 「搜尋字詞」), orders and empty state.
pub struct ListConfig {
    pub id: &'static str,
    pub noun: &'static str,
    pub sorts: Vec<ListSort>,
    pub empty_title: SharedString,
    pub empty_text: SharedString,
}

type ItemsHandler = Rc<dyn Fn(Vec<ListItem>, &mut Window, &mut App)>;
type ItemHandler = Rc<dyn Fn(ListItem, &mut Window, &mut App)>;
type MoreMenu = Rc<dyn Fn(Vec<ListItem>, &App) -> Vec<MenuEntry>>;
type RowMenu = Rc<dyn Fn(Vec<ListItem>) -> Vec<MenuEntry>>;

/// A list's state and view: its items, the search, the order, the selection, and what its actions do.
pub struct ListView {
    config: ListConfig,
    items: Vec<ListItem>,
    /// Indices into `items` of the rows shown, in order.
    visible: Vec<usize>,
    search: Entity<InputState>,
    needle: String,
    sort: ListSort,
    selection: BTreeSet<SharedString>,
    /// The row a Shift+click extends from.
    anchor: Option<SharedString>,
    hovered: Option<SharedString>,
    /// Rows fading out before they are removed.
    removing: Option<(BTreeSet<SharedString>, Instant)>,
    scroll: UniformListScrollHandle,
    /// The rows' wheel (the same handle as `scroll`).
    smooth: SmoothScroll<UniformListScrollHandle>,
    focus: FocusHandle,
    footer: Option<SharedString>,
    on_add: Option<Action>,
    add_enabled: bool,
    on_edit: Option<ItemHandler>,
    on_remove: Option<ItemsHandler>,
    more: Option<MoreMenu>,
    row_menu: Option<RowMenu>,
    _search_events: Subscription,
}

impl ListView {
    pub fn new(config: ListConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 「搜尋字詞」, 「搜尋 App」: a space before a Latin noun.
        let gap = if config.noun.starts_with(|c: char| c.is_ascii_alphabetic()) { " " } else { "" };
        let placeholder = format!("搜尋{gap}{}", config.noun);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe_in(&search, window, |this: &mut Self, search, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.needle = search.read(cx).value().trim().to_lowercase();
                this.refilter();
                cx.notify();
            }
        });
        let sort = config.sorts.first().copied().unwrap_or(ListSort::Name);
        let scroll = UniformListScrollHandle::new();
        Self {
            config,
            items: Vec::new(),
            visible: Vec::new(),
            search,
            needle: String::new(),
            sort,
            selection: BTreeSet::new(),
            anchor: None,
            hovered: None,
            removing: None,
            smooth: SmoothScroll::new(scroll.clone()),
            scroll,
            focus: cx.focus_handle(),
            footer: None,
            on_add: None,
            add_enabled: true,
            on_edit: None,
            on_remove: None,
            more: None,
            row_menu: None,
            _search_events: subscription,
        }
    }

    /// The ＋ button.
    pub fn on_add(&mut self, action: impl Fn(&mut Window, &mut App) + 'static) {
        self.on_add = Some(Rc::new(action));
    }

    /// Whether ＋ works (it needs the engine).
    pub fn set_add_enabled(&mut self, enabled: bool) {
        self.add_enabled = enabled;
    }

    /// 編輯 on hover and 編輯… in the context menu.
    pub fn on_edit(&mut self, action: impl Fn(ListItem, &mut Window, &mut App) + 'static) {
        self.on_edit = Some(Rc::new(action));
    }

    /// 刪除 (hover, Delete, context menu).
    pub fn on_remove(&mut self, action: impl Fn(Vec<ListItem>, &mut Window, &mut App) + 'static) {
        self.on_remove = Some(Rc::new(action));
    }

    /// The ⋯ menu, built when it opens from the selected rows.
    pub fn more_menu(&mut self, menu: impl Fn(Vec<ListItem>, &App) -> Vec<MenuEntry> + 'static) {
        self.more = Some(Rc::new(menu));
    }

    /// Extra context-menu entries for the rows it opens on.
    pub fn row_menu(&mut self, menu: impl Fn(Vec<ListItem>) -> Vec<MenuEntry> + 'static) {
        self.row_menu = Some(Rc::new(menu));
    }

    /// The note under the card.
    pub fn set_footer(&mut self, footer: Option<SharedString>) {
        self.footer = footer;
    }

    /// Replaces the items, keeping the selection where the ids survive (or selecting `select`).
    pub fn set_items(&mut self, items: Vec<ListItem>, select: Option<BTreeSet<SharedString>>, cx: &mut Context<Self>) {
        if items == self.items && select.is_none() {
            return;
        }
        let ids: BTreeSet<SharedString> = items.iter().map(|item| item.id.clone()).collect();
        let wanted = select.clone().unwrap_or_else(|| self.selection.clone());
        self.selection = wanted.intersection(&ids).cloned().collect();
        self.items = items;
        self.removing = None;
        self.refilter();
        if let Some(first) = select.and_then(|ids| self.visible.iter().position(|&i| ids.contains(&self.items[i].id))) {
            self.scroll.scroll_to_item(first, ScrollStrategy::Center);
        }
        cx.notify();
    }

    /// Types into the search field (screenshots).
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_value(query.to_owned(), window, cx));
        self.needle = query.trim().to_lowercase();
        self.refilter();
        cx.notify();
    }

    /// Selects rows by id (screenshots).
    pub fn select(&mut self, ids: impl IntoIterator<Item = SharedString>, cx: &mut Context<Self>) {
        self.selection = ids.into_iter().collect();
        cx.notify();
    }

    pub fn items(&self) -> &[ListItem] {
        &self.items
    }

    /// The selected rows, in list order.
    pub fn selected(&self) -> Vec<ListItem> {
        self.items.iter().filter(|item| self.selection.contains(&item.id)).cloned().collect()
    }

    fn refilter(&mut self) {
        let needle = self.needle.clone();
        let mut visible: Vec<usize> =
            (0..self.items.len()).filter(|&i| needle.is_empty() || self.items[i].matches(&needle)).collect();
        let sort = self.sort;
        let items = &self.items;
        visible.sort_by(|&a, &b| {
            let (a, b) = (&items[a], &items[b]);
            if sort == ListSort::Count && a.count != b.count {
                return b.count.total_cmp(&a.count);
            }
            a.key(sort).cmp(b.key(sort)).then_with(|| a.id.cmp(&b.id))
        });
        self.visible = visible;
    }

    fn set_sort(&mut self, sort: ListSort, cx: &mut Context<Self>) {
        self.sort = sort;
        self.refilter();
        cx.notify();
    }

    /// Fades `rows` out, then hands them to `on_remove`.
    fn remove(&mut self, rows: Vec<ListItem>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(on_remove) = self.on_remove.clone() else { return };
        if rows.is_empty() {
            return;
        }
        if cx.reduce_motion() {
            window.defer(cx, move |window, cx| on_remove(rows, window, cx));
            return;
        }
        self.removing = Some((rows.iter().map(|row| row.id.clone()).collect(), Instant::now()));
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(motion::FAST).await;
            this.update_in(cx, |this, window, cx| {
                this.removing = None;
                cx.notify();
                // The page reloads this list in the callback: run it once this list is no longer being updated.
                window.defer(cx, move |window, cx| on_remove(rows, window, cx));
            })
            .ok();
        })
        .detach();
    }

    fn click(&mut self, index: usize, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&item) = self.visible.get(index) else { return };
        let id = self.items[item].id.clone();
        let modifiers = event.modifiers();
        if modifiers.shift {
            let from = self
                .anchor
                .as_ref()
                .and_then(|anchor| self.visible.iter().position(|&i| &self.items[i].id == anchor))
                .unwrap_or(index);
            let (start, end) = (from.min(index), from.max(index));
            self.selection = self.visible[start..=end].iter().map(|&i| self.items[i].id.clone()).collect();
        } else if modifiers.secondary(cx) {
            if !self.selection.remove(&id) {
                self.selection.insert(id.clone());
            }
            self.anchor = Some(id);
        } else {
            let only = self.selection.len() == 1 && self.selection.contains(&id);
            self.selection.clear();
            if !only {
                self.selection.insert(id.clone());
            }
            self.anchor = Some(id);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "delete" | "backspace" if !self.selection.is_empty() => {
                let rows = self.selected();
                self.remove(rows, window, cx);
            }
            "escape" if !self.selection.is_empty() => {
                self.selection.clear();
                cx.notify();
            }
            key @ ("up" | "down") if !self.visible.is_empty() => {
                let current = self.visible.iter().position(|&i| self.selection.contains(&self.items[i].id));
                let next = match (key, current) {
                    ("down", Some(at)) => (at + 1).min(self.visible.len() - 1),
                    ("up", Some(at)) => at.saturating_sub(1),
                    (_, None) => 0,
                    _ => return,
                };
                let id = self.items[self.visible[next]].id.clone();
                self.selection = BTreeSet::from([id.clone()]);
                self.anchor = Some(id);
                self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let total = self.items.len();
        let count = if self.needle.is_empty() {
            format!("共 {} 個", grouped(total))
        } else {
            format!("{} / {} 個", grouped(self.visible.len()), grouped(total))
        };
        let search = search_field(&self.search, format!("搜尋{}", self.config.noun), cx)
            .flex_1()
            .max_w(SEARCH_MAX_W)
            .min_w(SEARCH_MIN_W);
        let sorts: Vec<(SharedString, ListSort)> =
            self.config.sorts.iter().map(|&sort| (SharedString::from(format!("依{}", sort.title())), sort)).collect();
        let entity = cx.entity().downgrade();
        let sort_picker = (sorts.len() > 1).then(|| {
            popup_button(
                SharedString::from(format!("{}-sort", self.config.id)),
                sorts,
                self.sort,
                move |sort, _, cx| {
                    entity.update(cx, |this, cx| this.set_sort(sort, cx)).ok();
                },
                cx,
            )
        });
        let add = self.on_add.clone().map(|on_add| {
            icon_button("add", IconName::Plus, IconButtonKind::Bordered, cx)
                .disabled(!self.add_enabled)
                .tooltip(format!("加入{}", self.config.noun))
                .on_click(move |_, window, cx| on_add(window, cx))
        });
        let more = self.more.clone().map(|more| {
            let selected = self.selected();
            icon_button("more", IconName::Ellipsis, IconButtonKind::Ghost, cx)
                .tooltip("更多")
                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
                    menu_entries(menu, more(selected.clone(), cx))
                })
        });
        // Boxed controls start and end at the card's edges, under which the rows' text is inset.
        h().h(TOOLBAR_H).w_full().gap(space::S8).child(search).child(
            h().ml_auto()
                .flex_shrink_0()
                .gap(space::S8)
                .child(div().pr(space::S4).text_size(text::SUBTITLE).text_color(p.text_secondary).child(count))
                .children(sort_picker)
                .children(add)
                .children(more),
        )
    }

    fn empty_card(&self, cx: &App) -> AnyElement {
        let p = theme::palette(cx);
        let searching = !self.needle.is_empty();
        let (icon, title, body) = if searching {
            (
                IconName::Search,
                SharedString::from(format!(
                    "沒有符合「{}」的{}",
                    self.search.read(cx).value().trim(),
                    self.config.noun
                )),
                SharedString::from("試試較短的字，或用注音搜尋。"),
            )
        } else {
            (IconName::Inbox, self.config.empty_title.clone(), self.config.empty_text.clone())
        };
        // No box: the message stands centred where the rows would be.
        v().w_full()
            .items_center()
            .gap(space::S6)
            .py(space::S40)
            .px(size::ROW_PADDING_X)
            .child(div().pb(space::S4).child(Icon::new(icon).size(size::ICON_XL).text_color(p.text_tertiary)))
            .child(div().text_size(text::HEADLINE).font_weight(weight::SEMIBOLD).text_color(p.text).child(title))
            .child(
                div()
                    .max_w(EMPTY_TEXT_MAX_W)
                    .text_size(text::SUBTITLE)
                    .text_color(p.text_secondary)
                    .text_center()
                    .child(body),
            )
            .with_animation(
                SharedString::from(format!("{}-empty-{searching}", self.config.id)),
                Animation::new(motion::NORMAL),
                |this, t| this.opacity(t),
            )
            .into_any_element()
    }

    fn render_rows(&mut self, range: std::ops::Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = theme::palette(cx);
        let can_edit = self.on_edit.is_some();
        range
            .filter_map(|index| {
                let item = self.items.get(*self.visible.get(index)?)?.clone();
                let selected = self.selection.contains(&item.id);
                let hovered = self.hovered.as_ref() == Some(&item.id);
                let removing = self.removing.as_ref().filter(|(ids, _)| ids.contains(&item.id)).map(|(_, at)| *at);
                let id = item.id.clone();
                let hover_id = item.id.clone();
                let details: AnyElement = match item.level {
                    Some(level) => usage_meter(level, cx).into_any_element(),
                    None => div()
                        .text_size(text::SUBTITLE)
                        .text_color(p.text_tertiary)
                        .child(item.meta.clone().unwrap_or_default())
                        .into_any_element(),
                };
                let edit_item = item.clone();
                let remove_item = item.clone();
                let actions = h()
                    .absolute()
                    .right_0()
                    .gap(space::S2)
                    .when(can_edit, |this| {
                        this.child(row_action("edit", "編輯", cx).on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(on_edit) = this.on_edit.clone() {
                                let item = edit_item.clone();
                                window.defer(cx, move |window, cx| on_edit(item, window, cx));
                            }
                        })))
                    })
                    .child(row_action("remove", "刪除", cx).on_click(cx.listener(move |this, _, window, cx| {
                        this.remove(vec![remove_item.clone()], window, cx);
                    })));
                let context_item = item.clone();
                let entity = cx.entity().downgrade();
                let row = h()
                    .id(SharedString::from(format!("row-{id}")))
                    .relative()
                    .w_full()
                    .h(ROW_H)
                    .px(size::ROW_PADDING_X)
                    .gap(space::S12)
                    .when(index > 0, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_0()
                                .left(size::ROW_PADDING_X)
                                .right(size::ROW_PADDING_X)
                                .h(size::HAIRLINE)
                                .bg(p.separator),
                        )
                    })
                    // Selection and hover: a neutral fill inset in the card (radius concentric with the card's).
                    .child(
                        div()
                            .absolute()
                            .left(space::S4)
                            .right(space::S4)
                            .top(space::S2)
                            .bottom(space::S2)
                            .rounded(size::R_SM)
                            .when(selected, |this| this.bg(p.selection))
                            .when(!selected && hovered, |this| this.bg(p.hover)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(text::BODY)
                            .text_color(if item.marked { p.text_secondary } else { p.text })
                            .child(item.title.clone()),
                    )
                    .when_some(item.detail.clone(), |this, detail| {
                        this.child(
                            div()
                                .min_w(px(0.))
                                .flex_shrink(1.)
                                .truncate()
                                .text_size(text::SUBTITLE)
                                .text_color(p.text_secondary)
                                .child(detail),
                        )
                    })
                    .child(div().flex_1().min_w(space::S8))
                    .child(
                        h().relative()
                            .flex_shrink_0()
                            .h_full()
                            .justify_end()
                            .min_w(if can_edit { ACTIONS_W } else { ACTIONS_W_REMOVE })
                            .child(div().when(hovered, |this| this.opacity(0.)).child(details))
                            .when(hovered, |this| this.child(actions)),
                    )
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        if *hovering {
                            this.hovered = Some(hover_id.clone());
                        } else if this.hovered.as_ref() == Some(&hover_id) {
                            this.hovered = None;
                        }
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, event, window, cx| this.click(index, event, window, cx)))
                    .context_menu(move |menu, _, cx| {
                        let Some(this) = entity.upgrade() else { return menu };
                        let (targets, can_edit, row_menu) = {
                            let this = this.read(cx);
                            let targets = if this.selection.contains(&context_item.id) {
                                this.selected()
                            } else {
                                vec![context_item.clone()]
                            };
                            (targets, this.on_edit.is_some(), this.row_menu.clone())
                        };
                        let mut entries = Vec::new();
                        if can_edit && targets.len() == 1 {
                            let target = targets[0].clone();
                            let this = this.clone();
                            entries.push(MenuEntry::new("編輯…", move |window, cx| {
                                if let Some(on_edit) = this.read(cx).on_edit.clone() {
                                    on_edit(target.clone(), window, cx);
                                }
                            }));
                        }
                        if let Some(row_menu) = row_menu {
                            entries.extend(row_menu(targets.clone()));
                        }
                        let mut menu = menu_entries(menu, entries);
                        if !menu.is_empty() {
                            menu = menu.separator();
                        }
                        let label = if targets.len() > 1 {
                            format!("刪除 {} 個", targets.len())
                        } else {
                            "刪除".to_owned()
                        };
                        let remove = this.clone();
                        menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                            let targets = targets.clone();
                            remove.update(cx, |this, cx| this.remove(targets, window, cx));
                        }))
                    });
                Some(match removing {
                    Some(at) => row
                        .with_animation(
                            SharedString::from(format!("removing-{id}-{at:?}")),
                            Animation::new(motion::FAST),
                            |this, t| this.opacity(1. - t),
                        )
                        .into_any_element(),
                    None => row.into_any_element(),
                })
            })
            .collect()
    }
}

/// 1234 → 「1,234」 (macOS formats counts this way).
pub fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// 編輯 or 刪除 over a row's details.
fn row_action(id: &'static str, label: &'static str, cx: &App) -> Button {
    let p = theme::palette(cx);
    Button::new(id).xsmall().ghost().child(div().text_size(text::SUBTITLE).text_color(p.accent).child(label))
}

/// Adds `entries` to `menu`.
fn menu_entries(mut menu: PopupMenu, entries: Vec<MenuEntry>) -> PopupMenu {
    for entry in entries {
        let action = entry.action.clone();
        let item = PopupMenuItem::new(entry.title.clone())
            .disabled(!entry.enabled)
            .on_click(move |_, window, cx| action(window, cx));
        menu = menu.item(item);
    }
    menu
}

/// 常用程度: five small bars, the used ones in the secondary colour.
pub fn usage_meter(level: u8, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let names = ["很少", "偶爾", "有時", "常用", "很常用"];
    let name = names[usize::from(level.clamp(1, 5) - 1)];
    h().id(SharedString::from(format!("meter-{level}")))
        .items_end()
        .h(size::ICON_XS)
        .gap(space::S2)
        .children((1..=5u8).map(|index| {
            div().w(px(3.)).h(px(4. + f32::from(index) * 1.6)).rounded(px(1.)).bg(if index <= level {
                p.text_secondary
            } else {
                p.separator
            })
        }))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(format!("常用程度：{name}")).build(window, cx)
        })
}

/// The height a list page may take: from the top of the page to the bottom of the window.
fn page_height(window: &Window) -> Pixels {
    let insets = window_paddings(window);
    window.viewport_size().height
        - insets.top
        - insets.bottom
        - size::TITLE_BAR
        - size::SURFACE_INSET
        - size::PAGE_PADDING_TOP
        - size::PAGE_PADDING_BOTTOM
}

impl Render for ListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let footer_h = if self.footer.is_some() { FOOTER_H } else { px(0.) };
        let card_max = (page_height(window) - TOOLBAR_H - TOOLBAR_GAP - footer_h - size::HAIRLINE * 2. - space::S8)
            .max(ROW_H * 3.);
        let card: AnyElement = if self.visible.is_empty() {
            self.empty_card(cx)
        } else {
            let count = self.visible.len();
            div()
                .id(SharedString::from(format!("{}-card", self.config.id)))
                .w_full()
                .py(space::S4)
                .rounded(size::R_LG)
                .bg(p.card)
                .border_1()
                .border_color(p.separator)
                .overflow_hidden()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(Self::key_down))
                .child(
                    uniform_list(
                        SharedString::from(format!("{}-rows", self.config.id)),
                        count,
                        cx.processor(|this, range, _window, cx| this.render_rows(range, cx)),
                    )
                    .with_sizing_behavior(ListSizingBehavior::Infer)
                    .track_scroll(&self.scroll)
                    .max_h(card_max)
                    .w_full(),
                )
                // After the rows: sees their wheel events before the list does.
                .child(self.smooth.wheel())
                .into_any_element()
        };
        let scrollbar = (!self.visible.is_empty())
            .then(|| gpui_kit::component::scroll::Scrollbar::vertical(&self.scroll).into_any_element());
        v().w_full()
            .gap(TOOLBAR_GAP)
            .child(self.toolbar(cx))
            .child(div().relative().w_full().child(card).children(scrollbar))
            .when_some(self.footer.clone(), |this, footer| {
                this.child(
                    div()
                        .px(size::ROW_PADDING_X)
                        .min_h(FOOTER_H - TOOLBAR_GAP)
                        .text_size(text::SUBTITLE)
                        .text_color(p.text_secondary)
                        .child(footer),
                )
            })
    }
}
