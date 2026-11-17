//! The keys of the rebindable commands, for every view that shows or changes them (the 快捷鍵 page, the 快捷鍵 rows of
//! the feature pages, the 介紹 demo).
//!
//! With the service running, everything goes through its [`Shortcuts`].  Without it (the engine not loaded yet, or the
//! core still being ported) a local port of the same rules works on the same settings key (`shortcuts`: only the
//! commands the user changed, `{command: [{key, modifiers}]}`), so the page stays usable and both agree on the file.
//! The rules and wording are those of `Shortcuts.cpp` (Qt) and macOS `Shortcuts.swift`, in the Linux/Windows words.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Result, anyhow, bail};
use gpui_kit::App;
use serde_json::{Value, json};
use zt_core::keys::{code, modifier};
use zt_core::settings::Settings;
use zt_core::shortcuts::{Binding, BindingLabel, Conflict, MAX_BINDINGS, Shortcuts};

use crate::app::{self, Place};
use crate::pages::Page;

/// The settings key the user's keys are stored under (shared with `zt_core::shortcuts`).
const STORAGE_KEY: &str = "shortcuts";

/// What the UI needs to know about one command.
#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub id: &'static str,
    /// The short name in conflict messages (「列出修正的字」).
    pub label: &'static str,
    /// The feature switch that turns it off, if any.
    pub feature: Option<&'static str>,
    /// Keeps at least one key.
    pub required: bool,
    pub defaults: Vec<Binding>,
}

fn b(key: i32, modifiers: u32) -> Binding {
    Binding { key, modifiers }
}

/// Every command, in the order of macOS `ShortcutCommand.allCases` (most used first).
pub fn commands() -> &'static [CommandInfo] {
    static LIST: OnceLock<Vec<CommandInfo>> = OnceLock::new();
    LIST.get_or_init(|| {
        use code::*;
        use modifier::{ALT, SHIFT};
        let c = |id, label, feature, required, defaults| CommandInfo { id, label, feature, required, defaults };
        vec![
            c("singleCharacterList", "只列一個字", Some("feature.singleCharacterList"), false, vec![b(DOWN, SHIFT)]),
            c("correctionsList", "列出修正的字", Some("feature.correctionsList"), false, vec![b(UP, SHIFT)]),
            c("typeZhuyin", "打出注音本身", Some("feature.typeZhuyin"), false, vec![b(QUOTE_LEFT, 0)]),
            c("nextPage", "下一頁", Some("feature.pageKeys"), false, vec![b(TAB, 0), b(PAGE_DOWN, 0), b(DOWN, ALT)]),
            c(
                "previousPage",
                "上一頁",
                Some("feature.pageKeys"),
                false,
                vec![b(TAB, SHIFT), b(PAGE_UP, 0), b(UP, ALT)],
            ),
            c("selectLeft", "往左選取", Some("feature.selectPhrase"), false, vec![b(LEFT, SHIFT)]),
            c("selectRight", "往右選取", Some("feature.selectPhrase"), false, vec![b(RIGHT, SHIFT)]),
            c("deleteWord", "刪掉一個詞", Some("feature.deleteWord"), false, vec![b(BACKSPACE, ALT)]),
            c("commitWithNewline", "送出並換行", None, true, vec![b(RETURN, SHIFT)]),
            c("previousCharacter", "改前一個字", None, true, vec![b(LEFT, ALT)]),
            c("nextCharacter", "改後一個字", None, true, vec![b(RIGHT, ALT)]),
            c("togglePunctuationWidth", "標點全半形", None, false, vec![b(SPACE, SHIFT)]),
            c("insertSpace", "插入空白", None, false, vec![b(SPACE, ALT)]),
            // No default key: the user chooses one.
            c("quickSettings", "快速設定", None, false, vec![]),
        ]
    })
}

/// The command `id`.
pub fn command(id: &str) -> Option<&'static CommandInfo> {
    commands().iter().find(|command| command.id == id)
}

/// Where the command's keys are set: its 「快捷鍵」 row (`keys-<id>`) on its feature's page (macOS
/// `ShortcutCommand.place`).
pub fn command_place(id: &str) -> Place {
    let page = match id {
        "typeZhuyin" | "correctionsList" | "selectLeft" | "selectRight" => Page::Choosing,
        "singleCharacterList" | "nextPage" | "previousPage" | "previousCharacter" | "nextCharacter" => Page::Candidates,
        _ => Page::Typing,
    };
    Place::row(page, format!("keys-{id}"))
}

// MARK: - Labels (Linux and Windows words: Ctrl, Alt, ⇧, Win or Super)

fn platform_key() -> &'static str {
    if cfg!(target_os = "windows") { "Win" } else { "Super" }
}

/// The keycaps of a binding: modifiers in the order Ctrl Alt ⇧ Win/Super, then the key.
pub fn keycaps(binding: Binding) -> Vec<String> {
    let mut caps = Vec::new();
    if binding.modifiers & modifier::CONTROL != 0 {
        caps.push("Ctrl".to_owned());
    }
    if binding.modifiers & modifier::ALT != 0 {
        caps.push("Alt".to_owned());
    }
    if binding.modifiers & modifier::SHIFT != 0 {
        caps.push("⇧".to_owned());
    }
    if binding.modifiers & modifier::META != 0 {
        caps.push(platform_key().to_owned());
    }
    caps.push(key_name(binding.key));
    caps
}

/// The name of a key on its cap.
pub fn key_name(key: i32) -> String {
    match key {
        code::SPACE => "空白".into(),
        code::TAB => "Tab".into(),
        code::RETURN => "Return".into(),
        code::ESCAPE => "Esc".into(),
        code::BACKSPACE => "⌫".into(),
        code::DELETE => "Del".into(),
        code::LEFT => "←".into(),
        code::RIGHT => "→".into(),
        code::UP => "↑".into(),
        code::DOWN => "↓".into(),
        code::HOME => "Home".into(),
        code::END => "End".into(),
        code::PAGE_UP => "PgUp".into(),
        code::PAGE_DOWN => "PgDn".into(),
        other => char::from_u32(other as u32).map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
    }
}

/// One chip: 「Alt+↓」, 「Ctrl+⇧U」, 「⇧Tab」, 「PgDn」.
pub fn compact(binding: Binding) -> String {
    let caps = keycaps(binding);
    let (last, modifiers) = caps.split_last().expect("a binding has a key");
    let mut text: String =
        modifiers.iter().map(|cap| if cap == "⇧" { cap.clone() } else { format!("{cap}+") }).collect();
    text.push_str(last);
    text
}

/// Read aloud and in tooltips: 「Shift+Tab」, 「Page Down」.
pub fn spoken(binding: Binding) -> String {
    let mut words: Vec<String> =
        keycaps(binding).into_iter().map(|cap| if cap == "⇧" { "Shift".to_owned() } else { cap }).collect();
    let name = match binding.key {
        code::BACKSPACE => Some("Backspace"),
        code::DELETE => Some("Delete"),
        code::PAGE_UP => Some("Page Up"),
        code::PAGE_DOWN => Some("Page Down"),
        _ => None,
    };
    if let (Some(name), Some(last)) = (name, words.last_mut()) {
        *last = name.to_owned();
    }
    words.join("+")
}

fn label(binding: Binding) -> BindingLabel {
    BindingLabel { binding, label: compact(binding), spoken: spoken(binding) }
}

/// The modifiers held so far, as the recorder shows them (「Ctrl+Alt+」, 「⇧」).
pub fn modifier_label(modifiers: u32) -> String {
    let mut text = String::new();
    if modifiers & modifier::CONTROL != 0 {
        text.push_str("Ctrl+");
    }
    if modifiers & modifier::ALT != 0 {
        text.push_str("Alt+");
    }
    if modifiers & modifier::SHIFT != 0 {
        text.push('⇧');
    }
    if modifiers & modifier::META != 0 {
        text.push_str(platform_key());
        text.push('+');
    }
    text
}

// MARK: - The store

/// The keys of every command, from the service or from the settings file.
pub enum KeyStore {
    Core(Shortcuts),
    Local(LocalShortcuts),
}

/// The store the views use now.
pub fn store(cx: &App) -> KeyStore {
    match app::service(cx) {
        Some(service) => KeyStore::Core(service.shortcuts().clone()),
        None => KeyStore::Local(LocalShortcuts::new(app::settings(cx).clone())),
    }
}

impl KeyStore {
    /// The keys of `command` now.
    pub fn bindings(&self, command: &str) -> Vec<BindingLabel> {
        match self {
            KeyStore::Core(core) => core.bindings(command),
            KeyStore::Local(local) => local.all().remove(command).unwrap_or_default().into_iter().map(label).collect(),
        }
    }

    /// Sets binding `index` of `command` (past the end adds one).
    ///
    /// # Errors
    /// Why not, in the page's words.
    pub fn record(&self, command: &str, key: i32, modifiers: u32, index: usize) -> Result<()> {
        match self {
            KeyStore::Core(core) => core.record(command, key, modifiers, Some(index)),
            KeyStore::Local(local) => local.record(command, key, modifiers, index),
        }
    }

    /// Removes one key; a required command keeps its last one.
    ///
    /// # Errors
    /// Why not, in the page's words.
    pub fn remove(&self, command: &str, index: usize) -> Result<()> {
        match self {
            KeyStore::Core(core) => core.remove(command, index),
            KeyStore::Local(local) => local.remove(command, index),
        }
    }

    /// The command's defaults back (those no other command uses); `None`: every command's.
    ///
    /// # Errors
    /// When every default is taken (naming who has it), or saving fails.
    pub fn reset(&self, command: Option<&str>) -> Result<()> {
        match self {
            KeyStore::Core(core) => core.reset(command),
            KeyStore::Local(local) => local.reset(command),
        }
    }

    pub fn is_customised(&self, command: &str) -> bool {
        match self {
            KeyStore::Core(core) => core.is_customised(command),
            KeyStore::Local(local) => local.user().contains_key(command),
        }
    }

    pub fn any_customised(&self) -> bool {
        match self {
            KeyStore::Core(core) => core.any_customised(),
            KeyStore::Local(local) => !local.user().is_empty(),
        }
    }

    /// The command's feature is on (or it has none).
    pub fn is_enabled(&self, command: &str) -> bool {
        match self {
            KeyStore::Core(core) => core.is_enabled(command),
            KeyStore::Local(local) => local.is_enabled(command),
        }
    }

    /// Bindings of `command` another enabled command also has: index → the other command.
    pub fn conflicts_of(&self, command: &str) -> BTreeMap<usize, &'static str> {
        match self {
            KeyStore::Core(core) => core.conflicts_of(command),
            KeyStore::Local(local) => local.conflicts_of(command, &local.all()),
        }
    }

    /// Every conflicting pair once.
    pub fn conflicts(&self) -> Vec<Conflict> {
        match self {
            KeyStore::Core(core) => core.conflicts(),
            KeyStore::Local(local) => local.conflicts(),
        }
    }

    /// The command's first key as a chip, or "" when it has none or its feature is off.
    pub fn active_label(&self, command: &str) -> String {
        if !self.is_enabled(command) {
            return String::new();
        }
        self.bindings(command).first().map(|binding| binding.label.clone()).unwrap_or_default()
    }

    /// Why the stored keys could not be read, if so.
    pub fn error(&self) -> Option<String> {
        match self {
            // Not a refused change: the field shows that one where it happened.
            KeyStore::Core(core) => core.storage_error(),
            KeyStore::Local(local) => local.error(),
        }
    }
}

/// The short name of `command` (「列出修正的字」).
pub fn command_label(id: &str) -> &'static str {
    command(id).map_or("", |command| command.label)
}

/// The command's default keys, read aloud and joined by 「或」.
pub fn defaults_spoken(id: &str) -> String {
    command(id)
        .map(|command| command.defaults.iter().map(|b| spoken(*b)).collect::<Vec<_>>().join(" 或 "))
        .unwrap_or_default()
}

// MARK: - The local port

/// The rules of `zt_core::shortcuts` on the settings file, for when the service is not running.
pub struct LocalShortcuts {
    settings: Settings,
}

type State = BTreeMap<&'static str, Vec<Binding>>;

/// `(key, modifiers)` as bindings are compared: keypad flag dropped, Backtab as ⇧Tab, Enter as Return, letters upper
/// case, shifted symbols as their unshifted key with Shift.  `None` for keys a binding cannot have.
pub fn normalize(key: i32, modifiers: u32) -> Option<Binding> {
    let allowed = modifier::SHIFT | modifier::CONTROL | modifier::ALT | modifier::META | modifier::KEYPAD;
    if modifiers & !allowed != 0 {
        return None;
    }
    let mut binding = b(key, modifiers & !modifier::KEYPAD);
    if binding.key == code::BACKTAB {
        binding.key = code::TAB;
        binding.modifiers |= modifier::SHIFT;
    }
    if binding.key == code::ENTER {
        binding.key = code::RETURN;
    }
    if (i32::from(b'a')..=i32::from(b'z')).contains(&binding.key) {
        binding.key -= 32;
    }
    const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
    const UNSHIFTED: &str = "`1234567890-=[]\\;',./";
    if let Some(index) = char::from_u32(binding.key as u32).and_then(|c| SHIFTED.find(c)) {
        binding.key = i32::from(UNSHIFTED.as_bytes()[index]);
        binding.modifiers |= modifier::SHIFT;
    }
    let named = matches!(
        binding.key,
        code::TAB
            | code::RETURN
            | code::ESCAPE
            | code::BACKSPACE
            | code::DELETE
            | code::LEFT
            | code::RIGHT
            | code::UP
            | code::DOWN
            | code::HOME
            | code::END
            | code::PAGE_UP
            | code::PAGE_DOWN
    );
    ((code::SPACE..=0x7e).contains(&binding.key) || named).then_some(binding)
}

fn printable(key: i32) -> bool {
    key > code::SPACE && key <= 0x7e
}

fn letter(key: i32) -> bool {
    (i32::from(b'A')..=i32::from(b'Z')).contains(&key)
}

impl LocalShortcuts {
    /// The local rules on `settings`.
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    fn stored(&self) -> Result<BTreeMap<&'static str, Vec<Binding>>, String> {
        let value = self.settings.get(STORAGE_KEY);
        if value.is_null() {
            return Ok(BTreeMap::new());
        }
        let broken = || "無法讀取自訂按鍵；目前使用預設值，原始資料已保留。".to_owned();
        let object = value.as_object().ok_or_else(broken)?;
        let mut result = BTreeMap::new();
        let mut seen = Vec::new();
        for (id, list) in object {
            let command = command(id).ok_or_else(broken)?;
            let list = list.as_array().filter(|list| list.len() <= MAX_BINDINGS).ok_or_else(broken)?;
            let mut bindings = Vec::new();
            for entry in list {
                let key = entry.get("key").and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok());
                let modifiers = entry.get("modifiers").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok());
                let (Some(key), Some(modifiers)) = (key, modifiers) else { return Err(broken()) };
                let binding = b(key, modifiers);
                if normalize(key, modifiers) != Some(binding) || seen.contains(&binding) {
                    return Err(broken());
                }
                seen.push(binding);
                bindings.push(binding);
            }
            if command.required && bindings.is_empty() {
                continue;
            }
            result.insert(command.id, bindings);
        }
        Ok(result)
    }

    /// The commands the user changed.
    pub fn user(&self) -> State {
        self.stored().unwrap_or_default()
    }

    pub fn error(&self) -> Option<String> {
        self.stored().err()
    }

    /// Every command's keys: the user's, and the defaults of the others minus defaults the user gave elsewhere (a
    /// command keeps all its defaults when every one is taken, and shows the conflict).
    pub fn all(&self) -> State {
        with_defaults(self.user())
    }

    pub fn is_enabled(&self, command_id: &str) -> bool {
        command(command_id).is_some_and(|command| command.feature.is_none_or(|feature| self.settings.bool(feature)))
    }

    fn reserved(&self, binding: Binding, command: &str) -> Option<String> {
        let plain = binding.modifiers == 0;
        if plain {
            if (binding.key == code::PAGE_DOWN && command == "nextPage")
                || (binding.key == code::PAGE_UP && command == "previousPage")
            {
                return None;
            }
            match binding.key {
                code::PAGE_UP | code::PAGE_DOWN => return Some(format!("{} 用來翻頁", spoken(binding))),
                code::HOME => return Some(format!("{} 移到組字的開頭", spoken(binding))),
                code::END => return Some(format!("{} 移到組字的結尾", spoken(binding))),
                code::DELETE => return Some(format!("{} 刪除游標後的字", spoken(binding))),
                _ => {}
            }
        }
        if binding.modifiers & modifier::META != 0 {
            return Some(format!("{} 組合鍵留給系統", platform_key()));
        }
        let printable = printable(binding.key);
        let control = binding.modifiers & modifier::CONTROL != 0;
        let alt = binding.modifiers & modifier::ALT != 0;
        if plain {
            let typing = matches!(
                binding.key,
                code::DOWN
                    | code::UP
                    | code::LEFT
                    | code::RIGHT
                    | code::SPACE
                    | code::RETURN
                    | code::ESCAPE
                    | code::BACKSPACE
                    | code::DELETE
                    | code::HOME
                    | code::END
                    | code::PAGE_UP
                    | code::PAGE_DOWN
            );
            if typing {
                return Some("這個鍵用在打字、選字或送出".into());
            }
            if printable && binding.key != code::QUOTE_LEFT {
                return Some("這個鍵用在打注音、聲調或標點".into());
            }
        }
        if printable && binding.modifiers == modifier::SHIFT {
            return Some("Shift 加這個鍵打中文標點或大寫字母".into());
        }
        if printable && alt && !control && !letter(binding.key) {
            return Some("Alt 加這個鍵打鍵上印的符號".into());
        }
        if printable
            && binding.modifiers == modifier::CONTROL
            && !letter(binding.key)
            && self.settings.bool("feature.controlPunctuation")
        {
            return Some("Ctrl 加符號鍵打中文標點".into());
        }
        if control && !alt && letter(binding.key) {
            return Some("Ctrl 加字母留給 App".into());
        }
        None
    }

    /// Why `binding` cannot be key `index` of `command`, naming the command that has it.
    fn problem(&self, binding: Binding, command: &str, index: usize, all: &State) -> Option<String> {
        if let Some(reason) = self.reserved(binding, command) {
            return Some(reason);
        }
        for (other, list) in all {
            for (position, bound) in list.iter().enumerate() {
                if *bound == binding && !(*other == command && position == index) {
                    return Some(if *other == command {
                        "已是這個功能的另一個按鍵".to_owned()
                    } else {
                        format!("已用於「{}」", command_label(other))
                    });
                }
            }
        }
        None
    }

    /// Stores `next` as the user's keys: a command that was on its defaults and still has what they resolved to stays
    /// on its defaults.
    fn store(&self, next: &State) -> Result<()> {
        let user = self.user();
        let resolved = self.all();
        let mut object = serde_json::Map::new();
        for command in commands() {
            let list = next.get(command.id).cloned().unwrap_or_default();
            let keep = if user.contains_key(command.id) {
                list != command.defaults
            } else {
                Some(&list) != resolved.get(command.id) && list != command.defaults
            };
            if keep {
                let entries =
                    list.iter().map(|b| json!({ "key": b.key, "modifiers": b.modifiers })).collect::<Vec<_>>();
                object.insert(command.id.to_owned(), Value::Array(entries));
            }
        }
        let value = if object.is_empty() { Value::Null } else { Value::Object(object) };
        let result =
            if value.is_null() { self.settings.reset(STORAGE_KEY) } else { self.settings.set(STORAGE_KEY, value) };
        result.map_err(|_| anyhow!("無法儲存按鍵，請檢查設定檔的寫入權限。"))
    }

    fn record(&self, command_id: &str, key: i32, modifiers: u32, index: usize) -> Result<()> {
        let Some(command) = command(command_id) else { bail!("找不到這個按鍵功能。") };
        let Some(binding) = normalize(key, modifiers) else { bail!("這個鍵不能用") };
        let mut all = self.all();
        let mut list = all.get(command.id).cloned().unwrap_or_default();
        let index = index.min(list.len());
        if index == list.len() && list.len() >= MAX_BINDINGS {
            bail!("每個功能最多 {MAX_BINDINGS} 個按鍵");
        }
        if let Some(reason) = self.problem(binding, command.id, index, &all) {
            bail!(reason);
        }
        if index == list.len() {
            list.push(binding);
        } else {
            list[index] = binding;
        }
        all.insert(command.id, list);
        self.store(&all)
    }

    fn remove(&self, command_id: &str, index: usize) -> Result<()> {
        let Some(command) = command(command_id) else { bail!("找不到要移除的按鍵。") };
        let mut all = self.all();
        let list = all.entry(command.id).or_default();
        if index >= list.len() {
            bail!("找不到要移除的按鍵。");
        }
        if command.required && list.len() == 1 {
            bail!("「{}」至少要有一個按鍵", command.label);
        }
        list.remove(index);
        self.store(&all)
    }

    fn reset(&self, command_id: Option<&str>) -> Result<()> {
        let Some(command_id) = command_id else {
            return self.settings.reset(STORAGE_KEY).map_err(|_| anyhow!("無法儲存按鍵，請檢查設定檔的寫入權限。"));
        };
        let Some(command) = command(command_id) else { bail!("找不到這個按鍵功能。") };
        let mut all = self.all();
        all.insert(command.id, Vec::new());
        // The defaults another command now uses stay with it; the rest come back.
        let free: Vec<Binding> =
            command.defaults.iter().copied().filter(|b| self.problem(*b, command.id, 0, &all).is_none()).collect();
        if free.is_empty() {
            let reason = self.problem(command.defaults[0], command.id, 0, &all).unwrap_or_default();
            bail!(reason);
        }
        all.insert(command.id, free);
        self.store(&all)
    }

    fn conflicts_of(&self, command_id: &str, all: &State) -> BTreeMap<usize, &'static str> {
        let mut result = BTreeMap::new();
        if !self.is_enabled(command_id) {
            return result;
        }
        let Some(list) = all.get(command_id) else { return result };
        for (index, binding) in list.iter().enumerate() {
            if let Some(other) = commands().iter().find(|other| {
                other.id != command_id
                    && self.is_enabled(other.id)
                    && all.get(other.id).is_some_and(|l| l.contains(binding))
            }) {
                result.insert(index, other.id);
            }
        }
        result
    }

    fn conflicts(&self) -> Vec<Conflict> {
        let all = self.all();
        let mut seen: Vec<(&str, &str)> = Vec::new();
        let mut result = Vec::new();
        for command in commands() {
            for (index, other) in self.conflicts_of(command.id, &all) {
                let pair = if command.id < other { (command.id, other) } else { (other, command.id) };
                if seen.contains(&pair) {
                    continue;
                }
                seen.push(pair);
                let label =
                    all.get(command.id).and_then(|list| list.get(index)).map(|b| compact(*b)).unwrap_or_default();
                result.push(Conflict { command: command.id, other, label });
            }
        }
        result
    }
}

fn with_defaults(user: State) -> State {
    let taken: Vec<Binding> = user.values().flatten().copied().collect();
    let mut result = user;
    for command in commands() {
        if result.contains_key(command.id) {
            continue;
        }
        let free: Vec<Binding> = command.defaults.iter().copied().filter(|b| !taken.contains(b)).collect();
        result.insert(command.id, if free.is_empty() { command.defaults.clone() } else { free });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local() -> LocalShortcuts {
        LocalShortcuts::new(Settings::open(None))
    }

    #[test]
    fn labels_use_the_platform_words() {
        assert_eq!(compact(b(code::DOWN, modifier::ALT)), "Alt+↓");
        assert_eq!(compact(b(code::TAB, modifier::SHIFT)), "⇧Tab");
        assert_eq!(compact(b(i32::from(b'U'), modifier::CONTROL | modifier::SHIFT)), "Ctrl+⇧U");
        assert_eq!(spoken(b(code::PAGE_DOWN, 0)), "Page Down");
        assert_eq!(modifier_label(modifier::CONTROL | modifier::ALT), "Ctrl+Alt+");
    }

    #[test]
    fn record_conflict_and_reset() {
        let store = local();
        assert_eq!(store.all()["nextPage"].len(), 3);
        assert!(store.record("singleCharacterList", code::SPACE, 0, 0).is_err());
        let error = store.record("singleCharacterList", code::UP, modifier::SHIFT, 0).unwrap_err().to_string();
        assert_eq!(error, "已用於「列出修正的字」");
        store.record("singleCharacterList", i32::from(b'J'), modifier::CONTROL | modifier::ALT, 1).unwrap();
        assert_eq!(store.user()["singleCharacterList"].len(), 2);
        store.reset(Some("singleCharacterList")).unwrap();
        assert!(store.user().is_empty());
        assert!(store.remove("commitWithNewline", 0).is_err());
    }
}
