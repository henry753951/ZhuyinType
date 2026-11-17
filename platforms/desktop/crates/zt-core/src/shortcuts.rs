//! The rebindable commands of the 快捷鍵 page (macOS `Shared/Shortcuts.swift` and `Preferences.bindings`, Qt
//! `Shortcuts`): each command has up to five keys; only the commands the user changed are stored (under the
//! `shortcuts` key of the settings file), the others follow their defaults minus any default the user gave to another
//! command.  The feature switches (`feature.*`) come from the same settings.
//!
//! Platform words: ⌃ is Ctrl, ⌥ is Alt, ⌘ is Win (Windows) or Super (Linux) and stays reserved; fn ↑ ↓ ← → ⌫ are PgUp,
//! PgDn, Home, End and Delete.  Ctrl with a letter is the apps' (Ctrl+C, Ctrl+V …).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use anyhow::{Result, anyhow, bail};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use zhuyin_core::api::KeyInput;
use zhuyin_core::composer::CandidateAction;

use crate::keys::{code, modifier};
use crate::settings::Settings;

/// One key with modifiers (Qt key code and modifier flags, normalised: no keypad flag, letters upper case, shifted
/// symbols as their unshifted key with Shift).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Binding {
    pub key: i32,
    pub modifiers: u32,
}

/// A binding with the text the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingLabel {
    pub binding: Binding,
    /// One chip: 「⇧↓」, 「Alt+↓」, 「PgDn」.
    pub label: String,
    /// Read aloud and in tooltips: 「Shift+Tab」, 「Page Down」.
    pub spoken: String,
}

/// What a command does when its key is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The composer receives this input.
    Input(KeyInput),
    /// 改前一個字／改後一個字: the list moves to the neighbouring character (-1 or 1).
    Neighbour(i32),
    /// 標點全半形.
    TogglePunctuationWidth,
    /// 不要再推薦, 忘記這個選字, 加入自訂詞: a word action on the highlighted candidate (or, adding, the whole
    /// composition).
    Word(CandidateAction),
    /// 快速設定: the host shows the quick settings panel at the caret.
    QuickSettings,
    /// 選取的字加入自訂詞: the field's selected text in the 加入自訂詞 form (only when not composing).
    AddSelectedText,
    /// 符號與 Emoji: opens (or closes) the symbol picker.
    SymbolPicker,
}

/// One rebindable command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// `singleCharacterList`, `nextPage`, …
    pub id: &'static str,
    /// The short name on the 快捷鍵 page and in conflict messages.
    pub label: &'static str,
    /// The row's longer title.
    pub title: &'static str,
    /// The settings page whose 快捷鍵 row shows it.
    pub page: &'static str,
    /// The feature switch that turns it off, if any.
    pub feature: Option<&'static str>,
    /// Keeps at least one key.
    pub required: bool,
    /// The default keys.
    pub defaults: Vec<Binding>,
    pub action: Action,
}

impl Command {
    /// The row id links scroll to (`keys-<id>`).
    #[must_use]
    pub fn row(&self) -> String {
        format!("keys-{}", self.id)
    }
}

/// Two commands with the same key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub command: &'static str,
    pub other: &'static str,
    /// The shared key's chip.
    pub label: String,
}

/// The most keys a command has.
pub const MAX_BINDINGS: usize = 5;

/// The settings key the user's keys are stored under: `{command: [{key, modifiers}]}`, only commands the user changed.
pub const STORAGE_KEY: &str = "shortcuts";

type State = BTreeMap<&'static str, Vec<Binding>>;

const fn b(key: i32, modifiers: u32) -> Binding {
    Binding { key, modifiers }
}

static COMMANDS: LazyLock<Vec<Command>> = LazyLock::new(|| {
    use modifier::{ALT, CONTROL, SHIFT};
    let command = |id, label, title, page, feature, required, defaults: &[Binding], action| Command {
        id,
        label,
        title,
        page,
        feature,
        required,
        defaults: defaults.to_vec(),
        action,
    };
    vec![
        command(
            "singleCharacterList",
            "只列一個字",
            "只列一個字，再按一次還原",
            "appearance",
            Some("feature.singleCharacterList"),
            false,
            &[b(code::DOWN, SHIFT)],
            Action::Input(KeyInput::ShiftDown),
        ),
        command(
            "correctionsList",
            "列出修正的字",
            "列出按錯修正的字，再按一次還原",
            "choosing",
            Some("feature.correctionsList"),
            false,
            &[b(code::UP, SHIFT)],
            Action::Input(KeyInput::ShiftUp),
        ),
        command(
            "typeZhuyin",
            "打出注音本身",
            "打出注音本身",
            "choosing",
            Some("feature.typeZhuyin"),
            false,
            &[b(code::QUOTE_LEFT, 0)],
            Action::Input(KeyInput::Char('`')),
        ),
        command(
            "nextPage",
            "下一頁",
            "下一頁",
            "appearance",
            Some("feature.pageKeys"),
            false,
            &[b(code::TAB, 0), b(code::PAGE_DOWN, 0), b(code::DOWN, ALT)],
            Action::Input(KeyInput::Tab),
        ),
        command(
            "previousPage",
            "上一頁",
            "上一頁",
            "appearance",
            Some("feature.pageKeys"),
            false,
            &[b(code::TAB, SHIFT), b(code::PAGE_UP, 0), b(code::UP, ALT)],
            Action::Input(KeyInput::ShiftTab),
        ),
        command(
            "selectLeft",
            "往左選取",
            "往左選取，再按 Return 加入自訂詞",
            "choosing",
            Some("feature.selectPhrase"),
            false,
            &[b(code::LEFT, SHIFT)],
            Action::Input(KeyInput::ShiftLeft),
        ),
        command(
            "selectRight",
            "往右選取",
            "往右選取，再按 Return 加入自訂詞",
            "choosing",
            Some("feature.selectPhrase"),
            false,
            &[b(code::RIGHT, SHIFT)],
            Action::Input(KeyInput::ShiftRight),
        ),
        command(
            "deleteWord",
            "刪掉一個詞",
            "刪掉一個詞",
            "typing",
            Some("feature.deleteWord"),
            false,
            &[b(code::BACKSPACE, ALT)],
            Action::Input(KeyInput::OptionBackspace),
        ),
        command(
            "commitWithNewline",
            "送出並換行",
            "送出並換行",
            "typing",
            None,
            true,
            &[b(code::RETURN, SHIFT)],
            Action::Input(KeyInput::ShiftEnter),
        ),
        command(
            "previousCharacter",
            "改前一個字",
            "清單開著時改前一個字",
            "appearance",
            None,
            true,
            &[b(code::LEFT, ALT)],
            Action::Neighbour(-1),
        ),
        command(
            "nextCharacter",
            "改後一個字",
            "清單開著時改後一個字",
            "appearance",
            None,
            true,
            &[b(code::RIGHT, ALT)],
            Action::Neighbour(1),
        ),
        command(
            "togglePunctuationWidth",
            "標點全半形",
            "切換標點全半形",
            "typing",
            None,
            false,
            &[b(code::SPACE, SHIFT)],
            Action::TogglePunctuationWidth,
        ),
        command(
            "insertSpace",
            "插入空白",
            "在組字中放一個空白",
            "typing",
            None,
            false,
            &[b(code::SPACE, ALT)],
            Action::Input(KeyInput::ShiftSpace),
        ),
        // The word actions (macOS `ShortcutCommand` with the same ids); Ctrl+Z undoes them for 5 s.
        command(
            "blockCandidate",
            "不要再推薦",
            "不要再推薦選取的字詞",
            "choosing",
            None,
            false,
            &[b(code::DELETE, CONTROL)],
            Action::Word(CandidateAction::Block),
        ),
        command(
            "forgetCandidate",
            "忘記這個選字",
            "忘記選過這個字詞",
            "choosing",
            None,
            false,
            &[b(code::DELETE, CONTROL | SHIFT)],
            Action::Word(CandidateAction::Forget),
        ),
        command(
            "addCandidatePhrase",
            "加入自訂詞",
            "把選取的字詞或整段組字加入自訂詞",
            "choosing",
            None,
            false,
            &[b(code::RETURN, CONTROL)],
            Action::Word(CandidateAction::AddPhrase),
        ),
        command("quickSettings", "快速設定", "開啟快速設定", "typing", None, false, &[], Action::QuickSettings),
        // No key until the user gives it one.
        command(
            "addSelectedText",
            "選取的字加入自訂詞",
            "把 App 裡選取的字加入自訂詞",
            "choosing",
            None,
            false,
            &[],
            Action::AddSelectedText,
        ),
        command(
            "symbolPicker",
            "符號與 Emoji",
            "開啟符號與 Emoji 面板",
            "typing",
            Some("feature.symbolPicker"),
            false,
            &[b(i32::from(b'/'), CONTROL | SHIFT)],
            Action::SymbolPicker,
        ),
    ]
});

/// The composer inputs that mean a command: they come only through a command's key, never straight from a key
/// (macOS `ShortcutCommand.commandInputs`).  A command's old default key, once unbound, is the app's.
#[must_use]
pub fn is_command_input(input: KeyInput) -> bool {
    COMMANDS.iter().any(|command| command.action == Action::Input(input))
}

const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
const UNSHIFTED: &str = "`1234567890-=[]\\;',./";

fn is_printable(key: i32) -> bool {
    key > code::SPACE && key <= 0x7e
}

fn is_letter(key: i32) -> bool {
    (i32::from(b'A')..=i32::from(b'Z')).contains(&key)
}

impl Binding {
    /// The binding as stored and compared, or `None` for a key that cannot be bound.
    #[must_use]
    pub fn normalized(key: i32, modifiers: u32) -> Option<Self> {
        let allowed = modifier::SHIFT | modifier::CONTROL | modifier::ALT | modifier::META | modifier::KEYPAD;
        if modifiers & !allowed != 0 {
            return None;
        }
        let mut binding = Self { key, modifiers: modifiers & !modifier::KEYPAD };
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
        // Qt reports the shifted symbol; the binding names the key it is on.
        if let Some(index) = u8::try_from(binding.key).ok().and_then(|c| SHIFTED.find(char::from(c))) {
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

    /// The keycaps: modifiers in the macOS order (Ctrl Alt ⇧ Win/Super), then the key.
    fn keycaps(self) -> Vec<String> {
        let mut caps = Vec::new();
        if self.modifiers & modifier::CONTROL != 0 {
            caps.push("Ctrl".to_owned());
        }
        if self.modifiers & modifier::ALT != 0 {
            caps.push("Alt".to_owned());
        }
        if self.modifiers & modifier::SHIFT != 0 {
            caps.push("⇧".to_owned());
        }
        if self.modifiers & modifier::META != 0 {
            caps.push(META_NAME.to_owned());
        }
        let name = match self.key {
            code::SPACE => "空白".to_owned(),
            code::TAB => "Tab".to_owned(),
            code::RETURN => "Return".to_owned(),
            code::ESCAPE => "Esc".to_owned(),
            code::BACKSPACE => "⌫".to_owned(),
            code::DELETE => "Del".to_owned(),
            code::LEFT => "←".to_owned(),
            code::RIGHT => "→".to_owned(),
            code::UP => "↑".to_owned(),
            code::DOWN => "↓".to_owned(),
            code::HOME => "Home".to_owned(),
            code::END => "End".to_owned(),
            code::PAGE_UP => "PgUp".to_owned(),
            code::PAGE_DOWN => "PgDn".to_owned(),
            key => char::from_u32(key as u32).map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        };
        caps.push(name);
        caps
    }

    /// One chip: 「Alt+↓」, 「Ctrl+⇧U」, 「⇧Tab」, 「PgDn」.
    #[must_use]
    pub fn label(self) -> String {
        let caps = self.keycaps();
        let (last, modifiers) = caps.split_last().expect("a key");
        let mut result: String =
            modifiers.iter().map(|cap| if cap == "⇧" { cap.clone() } else { format!("{cap}+") }).collect();
        result.push_str(last);
        result
    }

    /// How the key is read aloud and in tooltips: 「Ctrl+Shift+U」, 「Page Down」.
    #[must_use]
    pub fn spoken(self) -> String {
        let mut words: Vec<String> =
            self.keycaps().into_iter().map(|cap| if cap == "⇧" { "Shift".to_owned() } else { cap }).collect();
        let name = match self.key {
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

    fn describe(self) -> BindingLabel {
        BindingLabel { binding: self, label: self.label(), spoken: self.spoken() }
    }
}

const META_NAME: &str = if cfg!(windows) { "Win" } else { "Super" };

/// The user's keys, and the default keys for every other command, leaving out a default the user gave to another
/// command (a command keeps all its defaults when every one is taken, and shows the conflict; the punctuation-width
/// and space commands yield instead).
fn with_defaults(user: &State) -> State {
    let mut result = user.clone();
    let taken: BTreeSet<Binding> = user.values().flatten().copied().collect();
    for command in COMMANDS.iter() {
        if result.contains_key(command.id) {
            continue;
        }
        let free: Vec<Binding> = command.defaults.iter().filter(|b| !taken.contains(b)).copied().collect();
        let keeps_all = free.is_empty() && !matches!(command.id, "togglePunctuationWidth" | "insertSpace");
        result.insert(command.id, if keeps_all { command.defaults.clone() } else { free });
    }
    result
}

struct Cache {
    revision: Option<u64>,
    user: State,
    error: Option<String>,
    /// The error is a refused change (a key in use, the last key of a required command), not the stored keys
    /// failing to load or save.
    refused: bool,
}

/// The keys of every command.  Cheap to clone; clones share the settings store.
#[derive(Clone)]
pub struct Shortcuts {
    settings: Settings,
    cache: Arc<Mutex<Cache>>,
}

const UNREADABLE: &str = "無法讀取自訂按鍵；目前使用預設值，原始資料已保留。";

fn parse(value: &Value) -> Result<State, String> {
    let Value::Object(object) = value else { return Err(UNREADABLE.to_owned()) };
    if serde_json::to_string(value).map_or(0, |text| text.len()) > 65_536 {
        return Err("自訂按鍵資料過大；目前使用預設值，原始資料已保留。".to_owned());
    }
    let mut state = State::new();
    let mut seen = BTreeSet::new();
    for (id, list) in object {
        let command = Shortcuts::command(id).ok_or(UNREADABLE)?;
        let list = list.as_array().filter(|list| list.len() <= MAX_BINDINGS).ok_or(UNREADABLE)?;
        let mut bindings = Vec::new();
        for item in list {
            let row = item.as_object().filter(|row| row.len() == 2).ok_or(UNREADABLE)?;
            let number = |name| {
                row.get(name)
                    .and_then(Value::as_f64)
                    .filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= f64::from(i32::MAX))
            };
            let (Some(key), Some(modifiers)) = (number("key"), number("modifiers")) else {
                return Err(UNREADABLE.to_owned());
            };
            let binding = Binding { key: key as i32, modifiers: modifiers as u32 };
            if Binding::normalized(binding.key, binding.modifiers) != Some(binding) || !seen.insert(binding) {
                return Err(UNREADABLE.to_owned());
            }
            bindings.push(binding);
        }
        if command.required && bindings.is_empty() {
            continue;
        }
        state.insert(command.id, bindings);
    }
    Ok(state)
}

impl Shortcuts {
    /// The keys stored in `settings` (unreadable stored keys: the defaults, and [`Shortcuts::error`] says so; the
    /// stored value is kept until the user changes a key).
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            cache: Arc::new(Mutex::new(Cache { revision: None, user: State::new(), error: None, refused: false })),
        }
    }

    fn user(&self) -> State {
        let revision = self.settings.revision();
        let mut cache = self.cache.lock();
        if cache.revision != Some(revision) {
            let stored = self.settings.get(STORAGE_KEY);
            let (user, error) = match stored {
                Value::Null => (State::new(), None),
                value => match parse(&value) {
                    Ok(user) => (user, None),
                    Err(error) => (State::new(), Some(error)),
                },
            };
            cache.user = user;
            // A failed save's message stays until the next change.
            if error.is_some() || cache.revision.is_some() {
                cache.refused = false;
                cache.error = error;
            } else {
                cache.error = None;
            }
            cache.revision = Some(revision);
        }
        cache.user.clone()
    }

    fn state(&self) -> State {
        with_defaults(&self.user())
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T> {
        let message = message.into();
        let mut cache = self.cache.lock();
        cache.error = Some(message.clone());
        cache.refused = false;
        Err(anyhow!(message))
    }

    /// A change that is not allowed: reported like a failure ([`Self::error`]) but not as a problem with the stored
    /// keys ([`Self::storage_error`]).
    fn refuse<T>(&self, message: impl Into<String>) -> Result<T> {
        let message = message.into();
        let mut cache = self.cache.lock();
        cache.error = Some(message.clone());
        cache.refused = true;
        Err(anyhow!(message))
    }

    /// Every command, most used first (macOS `ShortcutCommand.allCases`).
    #[must_use]
    pub fn commands() -> &'static [Command] {
        &COMMANDS
    }

    /// The command `id`.
    #[must_use]
    pub fn command(id: &str) -> Option<&'static Command> {
        COMMANDS.iter().find(|command| command.id == id)
    }

    /// The keys of `command` now.
    #[must_use]
    pub fn bindings(&self, command: &str) -> Vec<BindingLabel> {
        self.state().get(command).map(|list| list.iter().map(|b| b.describe()).collect()).unwrap_or_default()
    }

    /// Every command's keys now.
    #[must_use]
    pub fn all(&self) -> BTreeMap<&'static str, Vec<BindingLabel>> {
        self.state().into_iter().map(|(id, list)| (id, list.into_iter().map(Binding::describe).collect())).collect()
    }

    /// The command a key runs, if any (an enabled command wins over one whose feature is off).
    #[must_use]
    pub fn resolve(&self, key: i32, modifiers: u32) -> Option<&'static Command> {
        let needle = Binding::normalized(key, modifiers)?;
        let state = self.state();
        let mut fallback = None;
        for command in COMMANDS.iter() {
            if state.get(command.id).is_some_and(|list| list.contains(&needle)) {
                if self.is_enabled(command.id) {
                    return Some(command);
                }
                fallback = fallback.or(Some(command));
            }
        }
        fallback
    }

    /// Why `binding` cannot be a key of `command` (macOS `Shortcut.reservedReason(for:)`), if it cannot.  PgUp and PgDn
    /// page the list anyway, so they can be keys of 上一頁 and 下一頁.
    fn reserved(&self, binding: Binding, command: &str) -> Option<String> {
        let plain = binding.modifiers == 0;
        if plain {
            if (binding.key == code::PAGE_DOWN && command == "nextPage")
                || (binding.key == code::PAGE_UP && command == "previousPage")
            {
                return None;
            }
            let spoken = binding.spoken();
            match binding.key {
                code::PAGE_UP | code::PAGE_DOWN => return Some(format!("{spoken} 用來翻頁")),
                code::HOME => return Some(format!("{spoken} 移到組字的開頭")),
                code::END => return Some(format!("{spoken} 移到組字的結尾")),
                code::DELETE => return Some(format!("{spoken} 刪除游標後的字")),
                _ => {}
            }
        }
        if binding.modifiers & modifier::META != 0 {
            return Some(format!("{META_NAME} 組合鍵留給系統"));
        }
        let printable = is_printable(binding.key);
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
                return Some("這個鍵用在打字、選字或送出".to_owned());
            }
            if printable && binding.key != code::QUOTE_LEFT {
                return Some("這個鍵用在打注音、聲調或標點".to_owned());
            }
        }
        if printable && binding.modifiers == modifier::SHIFT {
            return Some("Shift 加這個鍵打中文標點或大寫字母".to_owned());
        }
        if printable && alt && !control && !is_letter(binding.key) {
            return Some("Alt 加這個鍵打鍵上印的符號".to_owned());
        }
        if printable
            && binding.modifiers == modifier::CONTROL
            && !is_letter(binding.key)
            && self.settings.get("feature.controlPunctuation").as_bool().unwrap_or(true)
        {
            return Some("Ctrl 加符號鍵打中文標點".to_owned());
        }
        if control && !alt && is_letter(binding.key) {
            return Some("Ctrl 加字母留給 App".to_owned());
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
                        format!("已用於「{}」", Self::command(other).map_or("", |c| c.label))
                    });
                }
            }
        }
        None
    }

    /// Stores `next` as the user's keys (`Preferences.bindings`' setter): a command that was on its defaults and still
    /// has what they resolved to stays on its defaults.
    fn store(&self, next: &State) -> Result<()> {
        let user = self.user();
        let resolved = with_defaults(&user);
        let mut object = Map::new();
        for command in COMMANDS.iter() {
            let list = next.get(command.id).cloned().unwrap_or_default();
            let keep = if user.contains_key(command.id) {
                list != command.defaults
            } else {
                Some(&list) != resolved.get(command.id) && list != command.defaults
            };
            if keep {
                let rows = list.iter().map(|b| json!({"key": b.key, "modifiers": b.modifiers})).collect();
                object.insert(command.id.to_owned(), Value::Array(rows));
            }
        }
        if self.settings.set(STORAGE_KEY, Value::Object(object)).is_err() {
            return self.fail("無法儲存按鍵，請檢查設定檔的寫入權限。");
        }
        self.cache.lock().error = None;
        Ok(())
    }

    /// Sets binding `index` of `command` (`None` or past the end adds one).
    ///
    /// # Errors
    /// Why not, in the page's words (reserved key, used by another command, too many keys, cannot save).
    pub fn record(&self, command: &str, key: i32, modifiers: u32, index: Option<usize>) -> Result<()> {
        let Some(entry) = Self::command(command) else { return self.refuse("找不到這個按鍵功能。") };
        let Some(binding) = Binding::normalized(key, modifiers) else { return self.refuse("這個鍵不能用") };
        let mut all = self.state();
        let mut list = all.get(entry.id).cloned().unwrap_or_default();
        let index = index.unwrap_or(list.len());
        if index > list.len() {
            return self.refuse("找不到要修改的按鍵。");
        }
        if index == list.len() && list.len() >= MAX_BINDINGS {
            return self.refuse(format!("每個功能最多 {MAX_BINDINGS} 個按鍵"));
        }
        if let Some(reason) = self.problem(binding, entry.id, index, &all) {
            return self.refuse(reason);
        }
        if index == list.len() {
            list.push(binding);
        } else {
            list[index] = binding;
        }
        all.insert(entry.id, list);
        self.store(&all)
    }

    /// Removes one key, default or not; a required command keeps its last one.
    ///
    /// # Errors
    /// When there is no such key, the command is required and this is its last key, or saving fails.
    pub fn remove(&self, command: &str, index: usize) -> Result<()> {
        let mut all = self.state();
        let Some(entry) = Self::command(command) else { return self.refuse("找不到要移除的按鍵。") };
        let Some(list) = all.get_mut(entry.id).filter(|list| index < list.len()) else {
            return self.refuse("找不到要移除的按鍵。");
        };
        if entry.required && list.len() == 1 {
            return self.refuse(format!("「{}」至少要有一個按鍵", entry.label));
        }
        list.remove(index);
        self.store(&all)
    }

    /// Restores the command's defaults that no other command uses now; with `None`, every command's (全部還原).
    ///
    /// # Errors
    /// When every default is taken (naming the command that has it), or saving fails.
    pub fn reset(&self, command: Option<&str>) -> Result<()> {
        let Some(command) = command else {
            if self.settings.set(STORAGE_KEY, Value::Object(Map::new())).is_err() {
                return self.fail("無法儲存按鍵，請檢查設定檔的寫入權限。");
            }
            self.cache.lock().error = None;
            return Ok(());
        };
        let Some(entry) = Self::command(command) else { return self.refuse("找不到這個按鍵功能。") };
        let mut all = self.state();
        all.insert(entry.id, Vec::new());
        if entry.defaults.is_empty() {
            return self.store(&all);
        }
        let free: Vec<Binding> =
            entry.defaults.iter().filter(|b| self.problem(**b, entry.id, 0, &all).is_none()).copied().collect();
        if free.is_empty() {
            let reason = self.problem(entry.defaults[0], entry.id, 0, &all).unwrap_or_default();
            return self.refuse(reason);
        }
        all.insert(entry.id, free);
        self.store(&all)
    }

    /// The user changed this command's keys.
    #[must_use]
    pub fn is_customised(&self, command: &str) -> bool {
        self.user().contains_key(command)
    }

    /// The user changed any command's keys.
    #[must_use]
    pub fn any_customised(&self) -> bool {
        !self.user().is_empty()
    }

    /// The command can be used (its feature is on, or it has none).
    #[must_use]
    pub fn is_enabled(&self, command: &str) -> bool {
        Self::command(command).is_some_and(|entry| {
            entry.feature.is_none_or(|feature| self.settings.get(feature).as_bool().unwrap_or(true))
        })
    }

    /// Bindings of `command` that another enabled command also has: binding index → the other command.
    #[must_use]
    pub fn conflicts_of(&self, command: &str) -> BTreeMap<usize, &'static str> {
        let mut result = BTreeMap::new();
        if !self.is_enabled(command) {
            return result;
        }
        let all = self.state();
        let list = all.get(command).cloned().unwrap_or_default();
        for (index, binding) in list.iter().enumerate() {
            if let Some(other) = COMMANDS.iter().find(|other| {
                other.id != command
                    && self.is_enabled(other.id)
                    && all.get(other.id).is_some_and(|list| list.contains(binding))
            }) {
                result.insert(index, other.id);
            }
        }
        result
    }

    /// Every conflicting pair once, for the 按鍵衝突 section.
    #[must_use]
    pub fn conflicts(&self) -> Vec<Conflict> {
        let all = self.state();
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        for command in COMMANDS.iter() {
            for (index, other) in self.conflicts_of(command.id) {
                let pair = if command.id < other { (command.id, other) } else { (other, command.id) };
                if seen.insert(pair) {
                    let label =
                        all.get(command.id).and_then(|list| list.get(index)).map(|b| b.label()).unwrap_or_default();
                    result.push(Conflict { command: command.id, other, label });
                }
            }
        }
        result
    }

    /// The command's first key as a chip (「⇧↓」), or "" when it has none or its feature is off.
    #[must_use]
    pub fn active_label(&self, command: &str) -> String {
        if !self.is_enabled(command) {
            return String::new();
        }
        self.state().get(command).and_then(|list| list.first()).map(|b| b.label()).unwrap_or_default()
    }

    /// The modifiers held so far, as the recorder shows them (「Ctrl+Alt+」).
    #[must_use]
    pub fn modifier_label(modifiers: u32) -> String {
        let mut label = String::new();
        if modifiers & modifier::CONTROL != 0 {
            label.push_str("Ctrl+");
        }
        if modifiers & modifier::ALT != 0 {
            label.push_str("Alt+");
        }
        if modifiers & modifier::SHIFT != 0 {
            label.push('⇧');
        }
        if modifiers & modifier::META != 0 {
            label.push_str(META_NAME);
            label.push('+');
        }
        label
    }

    /// Why the stored keys could not be read or saved, if so.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.user();
        self.cache.lock().error.clone()
    }

    /// Why the stored keys could not be read or saved, if so (a refused change is not such a problem).
    #[must_use]
    pub fn storage_error(&self) -> Option<String> {
        self.user();
        let cache = self.cache.lock();
        cache.error.clone().filter(|_| !cache.refused)
    }
}

/// Fails unless `id` names a command (for callers that take ids from the UI).
///
/// # Errors
/// When it does not.
pub fn require_command(id: &str) -> Result<&'static Command> {
    match Shortcuts::command(id) {
        Some(command) => Ok(command),
        None => bail!("找不到這個按鍵功能。"),
    }
}
