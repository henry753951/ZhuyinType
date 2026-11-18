//! What the broker does with each request (Qt `ImeHost::dispatch`): one focused field (connection and context) owns
//! the shared composer; every reply carries the commits made since the last one, exactly once; a stale context
//! cannot operate or receive another field's text.

use serde_json::{Map, Value, json};

use zhuyin_core::composer::CandidateAction;

use crate::controller::{ControllerEvent, InputController};
use crate::keys::{Key, ShiftSide, code, modifier};
use crate::presentation::Rect;

/// A connection, numbered by the broker.
pub type ConnectionId = u64;

/// What a request asks of the UI, besides the reply.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Show the settings window, or one place of it (`place` of `show-settings`: words, about, wizard, tour).
    ShowSettings(Option<&'static str>),
    /// The badge by the caret: 中 or 英, 全 or 半.
    Toast {
        text: &'static str,
    },
    Boundary(i32),
    /// A list showed the word actions' tip: count it (`wordActionHintCount`).
    WordActionTip,
    /// The 開啟快速設定 shortcut: show the 快速設定 panel at the caret.
    QuickPanel,
    /// 選取的字加入自訂詞: ask the adapter for the selected text (`selection: true` in the reply).
    SelectionWanted,
    /// The adapter's selected text (op `selection`): open the 加入自訂詞 form with it.
    AddSelectedText(String),
}

const OPS: &[&str] =
    &["focus", "key", "poll", "blur", "reset", "secure", "context", "mode", "action", "quick", "selection"];

/// The switches of `op: "quick"` (快速設定), in the reply's `quick` object too.
const QUICK: &[&str] = &["english", "fullWidth", "simplified", "learningPaused"];

fn failure(message: &str) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("ok".into(), false.into());
    object.insert("error".into(), message.into());
    object.insert("handled".into(), false.into());
    object
}

/// A whole number in `minimum..=maximum`.
fn integer(value: Option<&Value>, minimum: f64, maximum: f64) -> Option<f64> {
    let number = value?.as_f64()?;
    (number.is_finite() && number.fract() == 0.0 && number >= minimum && number <= maximum).then_some(number)
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// A click in a candidate window the adapter draws (`op: "action"`).
#[derive(Clone, Copy, Debug)]
enum Action {
    Pick(usize),
    Page(i32),
    Single,
    Corrections,
    Zhuyin,
    Word(CandidateAction, usize),
    Undo,
    /// 符號與 Emoji: item `n` of the picker's whole list (a cell of the grid).
    Symbol(usize),
    /// 符號與 Emoji: tab `n`.
    SymbolTab(usize),
}

/// A command the user can run with nothing composed, whose keys the adapter must claim (`Host::claims`).
struct IdleCommand {
    id: &'static str,
    /// Also in English mode.
    in_english: bool,
    /// Also in a private field (no 前文 is sent from it).
    in_private: bool,
}

/// Every command reachable with nothing composed.  Others act on the composition or the candidate list, while the
/// adapter claims every key anyway.
const IDLE_COMMANDS: [IdleCommand; 3] = [
    // 開啟快速設定 and 符號與 Emoji: in either mode (Ctrl+Alt chords are otherwise the application's or AltGr's).
    IdleCommand { id: "quickSettings", in_english: true, in_private: true },
    IdleCommand { id: "symbolPicker", in_english: true, in_private: true },
    // 選取的字加入自訂詞 reads the field's selection: Chinese mode only, never from a private field.
    IdleCommand { id: "addSelectedText", in_english: false, in_private: false },
];

/// The broker's state: who has focus, and the text waiting for its next reply.
#[derive(Debug)]
pub struct Host {
    active: Option<ConnectionId>,
    context: String,
    commit: String,
    secure: bool,
    inline_preedit: bool,
    /// The focused field's adapter draws the candidate window itself (`IPC.md` §候選字視窗).
    draws_panel: bool,
    /// Linux with a screen reader: the Fcitx5 addon did not ask to show the list, but this app tells it to
    /// (`fcitxPanel` in the focus reply), since Fcitx's list is read by screen readers and GPUI's windows are not.
    panel_for_screen_reader: bool,
    client_app: String,
    anchor: Rect,
    /// A blur asked for the personal data to be written; the service does it after answering, off its lock
    /// (`take_flush`).
    flush: bool,
    /// 在候選字視窗顯示組字的 App (executable names, compared without case).
    pub popup_apps: Vec<String>,
    /// 繁／簡 asked for by `op: "quick"`, for the service to set once its lock is released (a setting's observer
    /// takes that lock).
    pending_script: Option<bool>,
    /// The keys last sent in `claims` (IPC.md §攔截的按鍵).
    sent_claims: Option<Vec<(i32, u32)>>,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            active: None,
            context: String::new(),
            commit: String::new(),
            secure: false,
            inline_preedit: true,
            draws_panel: false,
            panel_for_screen_reader: false,
            client_app: String::new(),
            anchor: Rect::default(),
            flush: false,
            popup_apps: Vec::new(),
            pending_script: None,
            sent_claims: None,
        }
    }
}

impl Host {
    /// The caret of the focused field, in logical pixels.
    #[must_use]
    pub fn anchor(&self) -> Rect {
        self.anchor
    }

    /// A field has focus and is not secure.
    #[must_use]
    pub fn focused(&self) -> bool {
        self.active.is_some() && !self.secure
    }

    /// The focused field's adapter draws the candidate window itself: the app's own window stays hidden and the
    /// replies carry the window instead.
    #[must_use]
    pub fn draws_panel(&self) -> bool {
        self.focused() && self.draws_panel
    }

    /// The field last focused belongs to an adapter that draws the candidate window and the 中/英 badge itself
    /// (even after its blur): the app never draws them for it.
    #[must_use]
    pub fn adapter_draws_panel(&self) -> bool {
        self.draws_panel
    }

    /// A blur happened since the last call: the personal data should be written now.  The service does that after
    /// the reply, without holding its lock, so the blur (TSF has 200 ms for it) and every other field's keys never
    /// wait for the disk.
    pub fn take_flush(&mut self) -> bool {
        std::mem::take(&mut self.flush)
    }

    /// The focused field's adapter shows the list because a screen reader runs (see `panel_for_screen_reader`).
    #[must_use]
    pub fn panel_for_screen_reader(&self) -> bool {
        self.focused() && self.panel_for_screen_reader
    }

    fn popup_app(&self) -> bool {
        !self.client_app.is_empty() && self.popup_apps.iter().any(|app| app.eq_ignore_ascii_case(&self.client_app))
    }

    /// The app shows the composition itself: not when it cannot, or when it is on the 在候選字視窗顯示組字 list.
    #[must_use]
    pub fn inline_preedit(&self) -> bool {
        self.inline_preedit && !self.popup_app()
    }

    /// As macOS `ControllerCore.render`: the window shows a list, a notice, or the composition for apps that cannot;
    /// 聯想詞 are a list with nothing composed.
    #[must_use]
    pub fn visible(&self, controller: &InputController) -> bool {
        let p = controller.presentation();
        self.focused()
            && (controller.picker_open()
                || ((!p.is_empty() || (p.associated && p.has_list()))
                    && (p.has_list() || p.notice.is_some() || !self.inline_preedit())))
    }

    /// Takes the controller's events: commits go into the next reply (when a field has focus and is not secure);
    /// the rest are returned for the UI.
    pub fn drain(&mut self, controller: &mut InputController) -> Vec<Effect> {
        let mut effects = Vec::new();
        for event in controller.take_events() {
            match event {
                ControllerEvent::Commit(text) => {
                    if self.focused() {
                        self.commit.push_str(&text);
                    }
                }
                ControllerEvent::ModeSwitched { english } if self.focused() => {
                    let chinese = if controller.simplified() { "简" } else { "中" };
                    effects.push(Effect::Toast { text: if english { "英" } else { chinese } });
                }
                ControllerEvent::QuickPanel if self.focused() => effects.push(Effect::QuickPanel),
                ControllerEvent::WidthSwitched { full } if self.focused() => {
                    effects.push(Effect::Toast { text: if full { "全" } else { "半" } });
                }
                ControllerEvent::Boundary(direction) => effects.push(Effect::Boundary(direction)),
                ControllerEvent::WordActionTip if self.focused() => effects.push(Effect::WordActionTip),
                ControllerEvent::SelectionWanted if self.focused() => effects.push(Effect::SelectionWanted),
                _ => {}
            }
        }
        effects
    }

    fn discard(&mut self, controller: &mut InputController) {
        self.active = None;
        self.context.clear();
        self.commit.clear();
        self.client_app.clear();
        controller.set_secure(true);
        controller.take_events();
        self.secure = true;
    }

    fn set_secure(&mut self, controller: &mut InputController, value: bool) {
        if value || value != self.secure {
            if value {
                self.commit.clear();
            }
            controller.set_secure(value);
        }
        self.secure = value;
    }

    /// The connection closed: its field loses focus and its composition is dropped.
    pub fn disconnected(&mut self, controller: &mut InputController, connection: ConnectionId) {
        if self.active == Some(connection) {
            self.discard(controller);
        }
    }

    /// The keys an adapter that passes on only the keys it knows the service takes (TSF) must claim with nothing
    /// composed (IPC.md §攔截的按鍵): the bound keys of every command in `IDLE_COMMANDS` that can run in this field
    /// and mode now.  None in a secure field, where the service takes no key.
    fn claims(&self, controller: &InputController) -> Vec<(i32, u32)> {
        let mut claims = Vec::new();
        if self.secure {
            return claims;
        }
        let shortcuts = controller.shortcuts();
        let allowed = |command: &IdleCommand| {
            (command.in_english || !controller.english())
                && (command.in_private || !controller.private())
                && shortcuts.is_enabled(command.id)
        };
        for command in IDLE_COMMANDS.iter().filter(|command| allowed(command)) {
            for label in shortcuts.bindings(command.id) {
                let key = (label.binding.key, label.binding.modifiers);
                if !claims.contains(&key) {
                    claims.push(key);
                }
            }
        }
        claims
    }

    fn snapshot(&mut self, controller: &InputController, handled: bool) -> Map<String, Value> {
        let commit = std::mem::take(&mut self.commit);
        // 繁／簡: the composition as the app shows it (the conversion keeps offsets and clauses).
        let p = controller.output_presentation();
        // A listed app shows nothing itself: the composition is in the candidate window.
        let preedit = if self.secure || self.popup_app() { String::new() } else { p.composition.clone() };
        let cursor = if preedit.is_empty() { 0 } else { p.cursor };
        let visible = self.visible(controller);
        let mut object = Map::new();
        object.insert("ok".into(), true.into());
        object.insert("handled".into(), handled.into());
        object.insert("commit".into(), if self.secure { String::new() } else { commit }.into());
        if !preedit.is_empty() {
            // How the application marks the composition (IPC.md): every word underlined, the word the candidate
            // list replaces (`focus`) or the one ↓ would change (`target`) set apart.
            let clauses: Vec<Value> = p.clauses.iter().map(|range| Value::from(vec![range.start, range.end])).collect();
            let index =
                |clause: Option<usize>| clause.filter(|&index| index < clauses.len()).map_or(Value::Null, Value::from);
            object.insert("focus".into(), index(p.focused_clause));
            object.insert("target".into(), index(p.target_clause));
            object.insert("clauses".into(), clauses.into());
            // The words shown through an automatic correction (「標示自動修正」: empty, so left out, when off).
            if !p.corrected.is_empty() {
                let corrections: Vec<Value> =
                    p.corrected.iter().map(|range| Value::from(vec![range.start, range.end])).collect();
                object.insert("corrections".into(), corrections.into());
            }
        }
        object.insert("preedit".into(), preedit.into());
        object.insert("cursor".into(), cursor.into());
        object.insert("candidates".into(), visible.into());
        object.insert("english".into(), controller.english().into());
        object.insert("simplified".into(), controller.simplified().into());
        object.insert("learningPaused".into(), controller.learning_paused().into());
        object.insert("fullWidth".into(), controller.full_width().into());
        // The composition, even when a listed app shows it only in the window: TSF claims keys while it lasts.  A
        // passive 聯想詞 list is not: it takes only ↓ (`associated`), and TSF must not take the other keys from the
        // application (a Win32 field loses a key the test claimed, even when the host then declines it).
        let passive = visible && !controller.picker_open() && p.passive_suggestions();
        object.insert("composing".into(), (!self.secure && (!p.is_empty() || visible) && !passive).into());
        if passive && !self.secure {
            object.insert("associated".into(), true.into());
        }
        object
    }

    /// One request; returns the reply (without `v`, `id` and `context`, which the broker adds) and what the UI
    /// should do.
    pub fn dispatch(
        &mut self,
        controller: &mut InputController,
        connection: ConnectionId,
        r: &Map<String, Value>,
    ) -> (Map<String, Value>, Vec<Effect>) {
        match self.handle(controller, connection, r) {
            Ok((reply, effects)) => (reply, effects),
            Err(message) => (failure(message), Vec::new()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle(
        &mut self,
        controller: &mut InputController,
        connection: ConnectionId,
        r: &Map<String, Value>,
    ) -> Result<(Map<String, Value>, Vec<Effect>), &'static str> {
        let context = r.get("context").and_then(Value::as_str).unwrap_or_default();
        let op = r.get("op").and_then(Value::as_str).unwrap_or_default();
        let version = r.get("v").and_then(Value::as_f64);
        if version != Some(1.0) || context.is_empty() || utf16_len(context) > 128 {
            return Err("通訊格式無效。");
        }
        if op == "show-settings" {
            let place = match r.get("place") {
                None => None,
                Some(value) => Some(
                    ["words", "about", "wizard", "tour"]
                        .into_iter()
                        .find(|place| value.as_str() == Some(place))
                        .ok_or("設定頁面無效。")?,
                ),
            };
            return Ok((Map::from_iter([("ok".to_owned(), json!(true))]), vec![Effect::ShowSettings(place)]));
        }
        if !OPS.contains(&op) {
            return Err("不支援的操作。");
        }
        let flag = |name: &str| r.get(name).and_then(Value::as_bool);
        if matches!(op, "key" | "focus" | "secure") && flag("secure").is_none() {
            return Err("缺少安全輸入狀態。");
        }
        for field in ["secure", "private", "activated", "repeat", "capsLock", "inlinePreedit", "panel"]
            .into_iter()
            .chain(QUICK.iter().copied().filter(|_| op == "quick"))
        {
            if r.contains_key(field) && flag(field).is_none() {
                return Err("輸入狀態格式無效。");
            }
        }
        let mut key = None;
        if op == "key" {
            let code = integer(r.get("key"), 0.0, f64::from(0x01ff_ffff));
            let modifiers = integer(r.get("modifiers"), 0.0, f64::from(0x7fff_ffff));
            let side = match r.get("shiftSide") {
                None => Some(ShiftSide::Unknown),
                value => integer(value, 0.0, 2.0).and_then(|side| ShiftSide::from_wire(side as i64)),
            };
            let (Some(code), Some(modifiers), Some(release), Some(shift_side)) =
                (code, modifiers, flag("release"), side)
            else {
                return Err("按鍵格式無效。");
            };
            key = Some((
                Key {
                    code: code as i32,
                    modifiers: modifiers as u32,
                    repeat: flag("repeat") == Some(true),
                    shift_side,
                    ..Key::default()
                },
                release,
            ));
        }
        if op == "mode" && flag("english").is_none() {
            return Err("中英模式格式無效。");
        }
        let action = if op == "action" {
            let value = || integer(r.get("value"), -1000.0, 1000.0).map(|n| n as i32);
            Some(match r.get("action").and_then(Value::as_str) {
                Some("pick") => Action::Pick(value().filter(|row| *row >= 0).ok_or("動作格式無效。")? as usize),
                Some("page") => Action::Page(value().filter(|d| d.abs() == 1).ok_or("動作格式無效。")?),
                Some("single") => Action::Single,
                Some("corrections") => Action::Corrections,
                Some("zhuyin") => Action::Zhuyin,
                Some("undo") => Action::Undo,
                // An index in the picker's whole list, which can be longer than a page's ±1000.
                Some(name @ ("symbol" | "symbolTab")) => {
                    let n = integer(r.get("value"), 0.0, 1_000_000.0).ok_or("動作格式無效。")? as usize;
                    if name == "symbol" { Action::Symbol(n) } else { Action::SymbolTab(n) }
                }
                Some(name @ ("block" | "forget" | "addPhrase")) => {
                    let action = match name {
                        "block" => CandidateAction::Block,
                        "forget" => CandidateAction::Forget,
                        _ => CandidateAction::AddPhrase,
                    };
                    Action::Word(action, value().filter(|row| *row >= 0).ok_or("動作格式無效。")? as usize)
                }
                _ => return Err("動作格式無效。"),
            })
        } else {
            None
        };
        let selection = if op == "selection" {
            match r.get("text") {
                Some(Value::String(text)) if utf16_len(text) <= crate::phrase_draft::MAX_SELECTION => {
                    Some(text.clone())
                }
                _ => return Err("選取的文字格式無效。"),
            }
        } else {
            None
        };
        // 前文: the app and the text before the insertion point, when the platform reports them (IPC.md §前文).
        let text = |name: &str, limit: usize| match r.get(name) {
            None => Ok(None),
            Some(Value::String(text)) if utf16_len(text) <= limit => Ok(Some(text.clone())),
            Some(_) => Err("前文格式無效。"),
        };
        let app = text("app", 256)?;
        let before = text("before", 1024)?;
        let mut rect = self.anchor;
        if let Some(value) = r.get("rect") {
            let scale = match r.get("scale") {
                None => 1.0,
                Some(scale) => scale.as_f64().ok_or("游標位置無效。")?,
            };
            let numbers: Vec<f64> = value
                .as_array()
                .filter(|a| a.len() == 4)
                .ok_or("游標位置無效。")?
                .iter()
                .map(|v| v.as_f64().filter(|n| n.is_finite() && n.abs() <= 1_000_000.0).ok_or("游標位置無效。"))
                .collect::<Result<_, _>>()?;
            if !scale.is_finite() || !(0.25..=8.0).contains(&scale) {
                return Err("游標位置無效。");
            }
            if numbers[2] < 0.0 || numbers[3] < 0.0 {
                return Err("游標範圍無效。");
            }
            rect = Rect::new(numbers[0] / scale, numbers[1] / scale, numbers[2] / scale, numbers[3] / scale);
        }
        if !controller.session().ready() {
            return Err("輸入引擎尚未就緒。");
        }
        let same = self.active == Some(connection) && self.context == context;
        if op == "quick" && !same {
            // Another connection (a menu outside the field): the switches are the service's, so they apply, but the
            // reply carries nothing of the focused field.
            self.quick(controller, &flag);
            let effects = self.drain(controller);
            return Ok((self.foreign_quick_reply(controller), effects));
        }
        if op == "focus" {
            if !same {
                self.discard(controller);
                self.active = Some(connection);
                context.clone_into(&mut self.context);
            }
        } else if !same {
            return Err("輸入欄位已失去焦點。");
        }
        self.anchor = rect;
        if op == "focus" {
            if flag("activated") == Some(true) {
                controller.activated();
            }
            self.inline_preedit = flag("inlinePreedit").unwrap_or(true);
            let asked = flag("panel").unwrap_or(false);
            self.panel_for_screen_reader =
                !asked && cfg!(target_os = "linux") && crate::system_look::screen_reader_if_known();
            self.draws_panel = asked || self.panel_for_screen_reader;
        }
        if let Some(secure) = flag("secure") {
            self.set_secure(controller, secure);
        }
        if matches!(op, "focus" | "secure") || flag("private").is_some() {
            controller.set_private(flag("private").unwrap_or(false));
        }
        if let Some(caps_lock) = flag("capsLock") {
            controller.synchronize_caps_lock(caps_lock);
        }
        if !self.secure && !controller.unlearned() && (op == "focus" || app.is_some() || before.is_some()) {
            if op == "focus" || app.is_some() {
                self.client_app = app.unwrap_or_default();
            }
            controller.set_client(&self.client_app, before);
        }
        let mut handled = false;
        let mut found = Vec::new();
        match op {
            "key" if !self.secure => {
                let (key, release) = key.expect("validated");
                if release {
                    controller.release(&key);
                } else {
                    handled = controller.press(&key);
                }
            }
            "mode" if !self.secure => controller.set_english(flag("english") == Some(true)),
            "quick" => self.quick(controller, &flag),
            // Never from a secure or private field.
            "selection" if !self.secure && !controller.private() => {
                found.push(Effect::AddSelectedText(selection.expect("validated")));
            }
            "action" if !self.secure => match action.expect("validated") {
                Action::Pick(row) => self.pick(controller, row),
                Action::Page(delta) => self.turn_page(controller, delta),
                Action::Single => self.toggle_single(controller),
                Action::Corrections => self.toggle_corrections(controller),
                Action::Zhuyin => self.type_reading(controller),
                Action::Word(action, row) => self.word_action(controller, action, row),
                Action::Undo => self.undo_word_action(controller),
                Action::Symbol(index) => {
                    if self.visible(controller) {
                        controller.pick_symbol(index);
                    }
                }
                Action::SymbolTab(index) => controller.select_symbol_tab(index),
            },
            "reset" => {
                self.commit.clear();
                controller.close_picker();
                let _ = controller.session_mut().reset();
                controller.take_events();
            }
            "blur" => controller.deactivate(),
            _ => {}
        }
        let mut effects = self.drain(controller);
        effects.extend(found);
        let mut reply = self.snapshot(controller, handled);
        if effects.contains(&Effect::QuickPanel) {
            reply.insert("quickPanel".into(), true.into());
            reply.insert("handled".into(), true.into());
        }
        if op == "quick" {
            reply.insert("quick".into(), self.quick_state(controller));
            reply.insert("simplified".into(), self.simplified(controller).into());
        }
        // The keys the adapter claims with nothing composed: at focus, and when they change.
        let claims = self.claims(controller);
        if op != "blur" && (op == "focus" || self.sent_claims.as_ref() != Some(&claims)) {
            let list: Vec<Value> = claims.iter().map(|&(key, modifiers)| json!([key, modifiers])).collect();
            reply.insert("claims".into(), list.into());
            self.sent_claims = Some(claims);
        }
        if op == "blur" {
            self.active = None;
            self.context.clear();
            self.flush = true;
        }
        Ok((reply, effects))
    }

    /// `connection`'s field `context` has focus: its requests may operate the composer.
    #[must_use]
    pub fn owns(&self, connection: ConnectionId, context: &str) -> bool {
        self.active == Some(connection) && self.context == context
    }

    /// 繁／簡 that `op: "quick"` asked for, for the service to set now that its lock is released.
    pub fn take_pending_script(&mut self) -> Option<bool> {
        self.pending_script.take()
    }

    /// 簡體, counting a change waiting to be set.
    fn simplified(&self, controller: &InputController) -> bool {
        self.pending_script.unwrap_or_else(|| controller.simplified())
    }

    /// The reply to `op: "quick"` from a connection whose field does not have focus: the switches, no text.
    fn foreign_quick_reply(&self, controller: &InputController) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("ok".into(), true.into());
        object.insert("handled".into(), false.into());
        object.insert("commit".into(), "".into());
        object.insert("preedit".into(), "".into());
        object.insert("cursor".into(), 0.into());
        object.insert("candidates".into(), false.into());
        object.insert("english".into(), controller.english().into());
        object.insert("simplified".into(), self.simplified(controller).into());
        object.insert("learningPaused".into(), controller.learning_paused().into());
        object.insert("fullWidth".into(), controller.full_width().into());
        object.insert("composing".into(), false.into());
        object.insert("quick".into(), self.quick_state(controller));
        object
    }

    /// `op: "quick"`: applies the switches present, as their own controls do (中／英 not while composing or in a
    /// secure field; 暫停學習 is kept only in memory).  `simplified` is the `outputScript` setting: it waits in
    /// [`Self::take_pending_script`] for the service, since nothing sets a setting under the service's lock.
    fn quick(&mut self, controller: &mut InputController, flag: &dyn Fn(&str) -> Option<bool>) {
        if let Some(simplified) = flag("simplified")
            && simplified != controller.simplified()
        {
            self.pending_script = Some(simplified);
        }
        if let Some(english) = flag("english") {
            controller.set_english(english);
        }
        if let Some(full) = flag("fullWidth") {
            controller.set_full_width(full);
        }
        if let Some(paused) = flag("learningPaused") {
            controller.set_learning_paused(paused);
        }
    }

    /// The reply's `quick` object: the 快速設定 switches as they are now.
    #[must_use]
    pub fn quick_state(&self, controller: &InputController) -> Value {
        json!({
            "english": controller.english(),
            "fullWidth": controller.full_width(),
            "simplified": self.simplified(controller),
            "learningPaused": controller.learning_paused(),
            // A 符號 picker: none yet (the 「符號…」 item stays hidden).
            "symbols": false,
        })
    }

    /// Picks row `row` of the current page (a click in the candidate window).
    pub fn pick(&mut self, controller: &mut InputController, row: usize) {
        let p = controller.presentation();
        if self.visible(controller) && row < p.rows.len() {
            let index = p.rows[row].index;
            controller.choose(index);
        }
    }

    /// The window's page buttons (`delta` 1 or -1).
    pub fn turn_page(&mut self, controller: &mut InputController, delta: i32) {
        if self.visible(controller) && delta.abs() == 1 {
            controller.feed(&Key::new(if delta > 0 { code::PAGE_DOWN } else { code::PAGE_UP }, 0));
        }
    }

    /// Moves the highlight (`delta` 1 or -1).
    pub fn move_selection(&mut self, controller: &mut InputController, delta: i32) {
        if self.visible(controller) && delta.abs() == 1 {
            controller.feed(&Key::new(if delta > 0 { code::DOWN } else { code::UP }, 0));
        }
    }

    /// The 只列一個字 switch.
    pub fn toggle_single(&mut self, controller: &mut InputController) {
        if self.visible(controller) {
            controller.feed(&Key::new(code::DOWN, modifier::SHIFT));
        }
    }

    /// The 列出修正的字 switch.
    pub fn toggle_corrections(&mut self, controller: &mut InputController) {
        if self.visible(controller) {
            controller.toggle_corrections();
        }
    }

    /// A word action on row `row` of the current page (the candidate window's menu).
    pub fn word_action(&mut self, controller: &mut InputController, action: CandidateAction, row: usize) {
        let p = controller.presentation();
        if self.visible(controller) && row < p.rows.len() {
            let index = p.rows[row].index;
            controller.word_action(action, Some(index));
        }
    }

    /// The notice's 「復原」.
    pub fn undo_word_action(&mut self, controller: &mut InputController) {
        if self.visible(controller) {
            controller.undo_word_action();
        }
    }

    /// The footer's ` action: the highlighted span as Zhuyin.
    pub fn type_reading(&mut self, controller: &mut InputController) {
        let has_reading = controller.presentation().reading.as_deref().is_some_and(|r| !r.is_empty());
        if self.visible(controller) && has_reading && controller.settings().bool("feature.zhuyinRowClick") {
            controller.feed(&Key::new(code::QUOTE_LEFT, 0));
        }
    }
}
