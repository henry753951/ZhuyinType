//! The text input processor: follows TSF focus, sends keys to the broker inside synchronous edit sessions, applies
//! the replies, and owns the taskbar's 中/英 button.
//!
//! - Every TSF context gets its own broker context id.  Switching documents first sends `blur` for the old one and
//!   finishes its composition there, then focuses the new one; replies of an older focus (`generation`) are dropped.
//! - The broker is called only after TSF grants a synchronous write lock; if the lock fails, the key goes back to the
//!   application and the engine never saw it.
//! - `OnTestKeyDown`/`OnTestKeyUp` only predict locally (`keys::host_wants_key`): TSF may call them repeatedly, and a
//!   key they claim is taken from the application even if `OnKeyDown` then declines it.
//! - While a request is in flight (`BrokerClient::busy`) re-entrant calls defer focus changes and resets, and give
//!   keys back.
//! - Secure fields send only their secure state: no key identity or text leaves them.  Leaving one, or focusing a
//!   field, syncs Caps Lock with the host.
//! - Nothing runs between keys while the field is idle.  The host never writes unasked and clicks in the candidate
//!   window arrive as messages, so the only thing that changes on its own is an open candidate list (a setting
//!   changed meanwhile): the text service polls only while it shows one (`follow_list`).  A host that ended is
//!   noticed before the next key is sent (`BrokerClient::check`).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use serde_json::{Map, Value};
use windows::Win32::Foundation::{
    E_INVALIDARG, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, RECT, S_OK, WPARAM,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateGuid, CoCreateInstance};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, VK_CAPITAL};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, IEnumTfDisplayAttributeInfo, ITfComposition,
    ITfCompositionSink, ITfCompositionSink_Impl, ITfContext, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfInputProcessorProfileMgr, ITfKeyEventSink,
    ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfLangBarItemButton, ITfLangBarItemMgr, ITfSource,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfThreadFocusSink,
    ITfThreadFocusSink_Impl, ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, TF_INPUTPROCESSORPROFILE,
    TF_INVALID_COOKIE, TF_PROFILETYPE_INPUTPROCESSOR, TF_TMAE_COMLESS, TF_TMAE_SECUREMODE,
    TF_TMAE_UIELEMENTENABLEDONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CREATESTRUCTW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GWLP_USERDATA, GetMessageTime, GetWindowLongPtrW, HWND_MESSAGE, KillTimer, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    PostMessageW, RegisterClassW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, TPM_BOTTOMALIGN, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, UnregisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_NCCREATE, WM_NULL, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, ComObject, IUnknown, IUnknownImpl, Interface, PCWSTR, Ref, Result, implement, w};

use crate::broker::BrokerClient;
use crate::composition::{self, Slot, load, store};
use crate::display_attributes;
use crate::keyboard::{caps_lock_on, potential_key, translate_key, typed_character};
use crate::keys::{host_observes_key, host_wants_key};
use crate::mode_button::{self, ModeButton};
use crate::module::{ObjectCount, SERVICE_ID, guard, instance};
use crate::panel_window::{PanelWindow, WM_PANEL_ACTION, action_name};
use crate::protocol::{PanelUpdate, Reply};
use crate::registration::guid_string;

/// Adds 前文 to a request: the application's name and the text before the caret (IPC.md §前文).
fn put_context(fields: &mut Map<String, Value>, context: &ITfContext, cookie: u32) {
    fields.insert("app".into(), Value::from(composition::app_name()));
    if let Some(before) = composition::text_before(context, cookie) {
        fields.insert("before".into(), Value::from(before));
    }
}

/// The user picked another input method in this application since ZhuyinType was last activated here, so the next
/// activation is the user coming back to it rather than the application starting or opening another thread.  Set
/// when ZhuyinType deactivates because another keyboard input method became active (`switched_away`), not by every
/// deactivation: a thread that ends deactivates its text service too.  Taken by the next activation.
static SWITCHED_AWAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether this thread's keyboard input method is now another one (or a keyboard layout).  TSF reports the new
/// profile as active by the time it deactivates the old one; when a thread ends, ZhuyinType is still the active one.
/// Unknown (COM no longer usable, say) counts as not switched.
fn switched_away() -> bool {
    // SAFETY: COM is initialised on a TSF thread; the profile is written by the call.
    let active = unsafe {
        CoCreateInstance::<_, ITfInputProcessorProfileMgr>(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
            .and_then(|profiles| {
                let mut profile = TF_INPUTPROCESSORPROFILE::default();
                profiles.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile).map(|()| profile)
            })
    };
    active.is_ok_and(|profile| !(profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR && profile.clsid == SERVICE_ID))
}

const POLL_TIMER: usize = 1;
const POLL_MS: u32 = 20;
/// Posted to the message window after a field got focus: `PanelWindow::warm`, once the focus is handled.
const WM_WARM: u32 = WM_APP + 0x32;

fn same<T: Interface>(a: Option<&T>, b: Option<&T>) -> bool {
    a.map(Interface::as_raw) == b.map(Interface::as_raw)
}

fn hresult(result: Result<()>) -> windows::core::HRESULT {
    result.map_or_else(|error| error.code(), |()| S_OK)
}

#[implement(
    ITfTextInputProcessorEx,
    ITfThreadMgrEventSink,
    ITfThreadFocusSink,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider
)]
pub(crate) struct TextService {
    manager: RefCell<Option<ITfThreadMgr>>,
    source: RefCell<Option<ITfSource>>,
    keys: RefCell<Option<ITfKeystrokeMgr>>,
    manager_cookie: Cell<u32>,
    focus_cookie: Cell<u32>,
    client: Cell<u32>,
    context: RefCell<Option<ITfContext>>,
    deferred_context: RefCell<Option<ITfContext>>,
    composition: Slot,
    broker: BrokerClient,
    context_name: RefCell<String>,
    /// NUL-terminated name of this instance's window class (unique per thread and instance).
    window_class: RefCell<Vec<u16>>,
    /// Message-only window for the poll timer, the warm-up and the candidate window's clicks.
    window: Cell<HWND>,
    /// The poll timer runs (only while a candidate list is shown).
    polling: Cell<bool>,
    /// The last reply applied to the document (a poll whose reply changes nothing leaves the document alone).
    last_reply: RefCell<Option<Reply>>,
    /// Owner of the 中/英 button's menu (a menu needs a real window; it is never shown).
    menu_owner: Cell<HWND>,
    generation: Cell<u64>,
    thread_focused: Cell<bool>,
    host_focused: Cell<bool>,
    secure: Cell<bool>,
    /// Secure-mode, COM-less or UI-less activation: no desktop candidate window, so every key passes through.
    restricted: Cell<bool>,
    pending: Cell<bool>,
    /// The text service itself is changing the composition (its terminations are not resets).
    applying: Cell<bool>,
    focus_deferred: Cell<bool>,
    reset_deferred: Cell<bool>,
    english: Cell<bool>,
    composing: Cell<bool>,
    caps_at_reply: Cell<bool>,
    lang_bar: RefCell<Option<ITfLangBarItemMgr>>,
    mode_button: RefCell<Option<(ITfLangBarItemButton, ComObject<ModeButton>)>>,
    rect: Cell<RECT>,
    scale: Cell<f64>,
    panel: PanelWindow,
    /// The display attributes' atoms (display_attributes.rs), from activation.
    atoms: Cell<Option<[u32; 4]>>,
    /// The last key `observe` sent (virtual key, flags, message time).
    observed: Cell<(usize, isize, i32)>,
    /// The field is private (`composition::is_private`): no 前文 is sent.
    private: Cell<bool>,
    /// The user selected ZhuyinType again after another input method in this application: the next focus says so,
    /// and 中/英 starts as 「開始輸入時使用英文」 says.
    activated: Cell<bool>,
    /// A `WM_WARM` is on its way.
    warm_posted: Cell<bool>,
    _count: ObjectCount,
}

impl TextService {
    pub(crate) fn create() -> IUnknown {
        Self {
            manager: RefCell::new(None),
            source: RefCell::new(None),
            keys: RefCell::new(None),
            manager_cookie: Cell::new(TF_INVALID_COOKIE),
            focus_cookie: Cell::new(TF_INVALID_COOKIE),
            client: Cell::new(0),
            context: RefCell::new(None),
            deferred_context: RefCell::new(None),
            composition: RefCell::new(None),
            broker: BrokerClient::new(),
            context_name: RefCell::new(String::new()),
            window_class: RefCell::new(Vec::new()),
            window: Cell::new(HWND::default()),
            polling: Cell::new(false),
            last_reply: RefCell::new(None),
            menu_owner: Cell::new(HWND::default()),
            generation: Cell::new(0),
            thread_focused: Cell::new(false),
            host_focused: Cell::new(false),
            secure: Cell::new(true),
            restricted: Cell::new(false),
            pending: Cell::new(false),
            applying: Cell::new(false),
            focus_deferred: Cell::new(false),
            reset_deferred: Cell::new(false),
            english: Cell::new(false),
            composing: Cell::new(false),
            caps_at_reply: Cell::new(false),
            lang_bar: RefCell::new(None),
            mode_button: RefCell::new(None),
            rect: Cell::new(RECT::default()),
            scale: Cell::new(1.0),
            panel: PanelWindow::new(),
            atoms: Cell::new(None),
            observed: Cell::new((0, 0, 0)),
            private: Cell::new(false),
            activated: Cell::new(false),
            warm_posted: Cell::new(false),
            _count: ObjectCount::new(),
        }
        .into()
    }

    fn name(&self) -> String {
        self.context_name.try_borrow().map(|name| name.clone()).unwrap_or_default()
    }

    fn set_name(&self, name: String) {
        if let Ok(mut slot) = self.context_name.try_borrow_mut() {
            *slot = name;
        }
    }

    fn bump(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.forget_reply();
    }

    fn forget_reply(&self) {
        if let Ok(mut last) = self.last_reply.try_borrow_mut() {
            *last = None;
        }
    }

    fn kill_timer(&self) {
        let window = self.window.get();
        if self.polling.replace(false) && !window.is_invalid() {
            // SAFETY: our own window.
            let _ = unsafe { KillTimer(Some(window), POLL_TIMER) };
        }
    }

    fn button(&self) -> Option<ComObject<ModeButton>> {
        self.mode_button.try_borrow().ok().and_then(|button| button.as_ref().map(|(_, object)| object.clone()))
    }

    /// The local prediction of whether the host takes this key.  Caps Lock is not claimed when idle; if it changed
    /// since the host's last reply its 英 may be stale, so the host decides (a declined character is then typed by
    /// `OnKeyDown`, never lost).
    fn claims(&self, vk: WPARAM, flags: LPARAM, release: bool) -> bool {
        let caps_changed = caps_lock_on() != self.caps_at_reply.get();
        let composing = self.composing.get() || load(&self.composition).is_some();
        host_wants_key(&translate_key(vk, flags, release), composing, self.english.get() && !caps_changed)
    }

    fn destroy_windows(&self) {
        for window in [self.window.replace(HWND::default()), self.menu_owner.replace(HWND::default())] {
            if !window.is_invalid() {
                // SAFETY: our own windows; the back pointer is cleared before they go.
                unsafe {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(window);
                }
            }
        }
        let class = self.window_class.try_borrow_mut().map(|mut class| std::mem::take(&mut *class)).unwrap_or_default();
        if !class.is_empty() {
            // SAFETY: the class registered by this instance.
            let _ = unsafe { UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance())) };
        }
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        self.destroy_windows();
    }
}

impl TextService_Impl {
    fn create_window(&self) -> Result<()> {
        let unique = format!("ZhuyinType.Tsf.{}.{:p}", unsafe { GetCurrentThreadId() }, std::ptr::from_ref(self));
        let class: Vec<u16> = unique.encode_utf16().chain([0]).collect();
        let klass = WNDCLASSW {
            hInstance: instance(),
            lpfnWndProc: Some(window_proc),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        // SAFETY: the class name outlives the class (kept in window_class until unregistered).
        if unsafe { RegisterClassW(&klass) } == 0 {
            // A class of this name left by an earlier instance at the same address on a thread with the same id is
            // this module's own class, with this window procedure: use it rather than fail the activation (TSF would
            // then switch the thread to another input method).
            let error = windows::core::Error::from_thread();
            if error.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
                return Err(error);
            }
        }
        if let Ok(mut slot) = self.window_class.try_borrow_mut() {
            *slot = class;
        }
        let window = self.new_window(WINDOW_EX_STYLE(0), WINDOW_STYLE(0), Some(HWND_MESSAGE))?;
        self.window.set(window);
        Ok(())
    }

    fn new_window(&self, ex_style: WINDOW_EX_STYLE, style: WINDOW_STYLE, parent: Option<HWND>) -> Result<HWND> {
        let class = self.window_class.try_borrow().map(|class| class.clone()).unwrap_or_default();
        if class.is_empty() {
            return Err(E_INVALIDARG.into());
        }
        let this: *const c_void = std::ptr::from_ref(self).cast();
        // SAFETY: a registered class; the back pointer is cleared before the window is destroyed.
        unsafe {
            CreateWindowExW(
                ex_style,
                PCWSTR(class.as_ptr()),
                w!(""),
                style,
                0,
                0,
                0,
                0,
                parent,
                None,
                Some(instance()),
                Some(this),
            )
        }
    }

    fn add_mode_button(&self, manager: &ITfThreadMgr) {
        // The input method works without the indicator if the language bar refuses it.
        let Ok(lang_bar) = manager.cast::<ITfLangBarItemMgr>() else { return };
        let this = self.to_object();
        let (item, object) = mode_button::create(Rc::new(move |right_click, point| {
            if right_click {
                // The menu's hidden owner takes the activation; the field gets it back afterwards.
                let focus = crate::focus::Remembered::now();
                this.show_menu(point);
                focus.restore(this.menu_owner.get());
            } else {
                this.toggle_mode();
            }
        }));
        // SAFETY: a valid item.
        if unsafe { lang_bar.AddItem(&item) }.is_ok() {
            store(&self.lang_bar, Some(lang_bar));
            store(&self.mode_button, Some((item, object)));
        } else {
            object.clear_action();
        }
    }

    fn deactivate(&self) {
        if load(&self.manager).is_some() && switched_away() {
            SWITCHED_AWAY.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.switch_context(None);
        let button = self.mode_button.try_borrow_mut().ok().and_then(|mut button| button.take());
        if let Some((item, object)) = button {
            object.clear_action();
            if let Some(lang_bar) = load(&self.lang_bar) {
                // SAFETY: the item added in activate.
                let _ = unsafe { lang_bar.RemoveItem(&item) };
            }
        }
        store(&self.lang_bar, None);
        self.panel.destroy();
        self.destroy_windows();
        let keys = self.keys.try_borrow_mut().ok().and_then(|mut keys| keys.take());
        if let Some(keys) = keys {
            // SAFETY: our client id.
            let _ = unsafe { keys.UnadviseKeyEventSink(self.client.get()) };
        }
        let source = self.source.try_borrow_mut().ok().and_then(|mut source| source.take());
        if let Some(source) = source {
            for cookie in [self.manager_cookie.get(), self.focus_cookie.get()] {
                if cookie != TF_INVALID_COOKIE {
                    // SAFETY: cookies from AdviseSink.
                    let _ = unsafe { source.UnadviseSink(cookie) };
                }
            }
        }
        self.manager_cookie.set(TF_INVALID_COOKIE);
        self.focus_cookie.set(TF_INVALID_COOKIE);
        store(&self.manager, None);
        self.client.set(0);
        self.broker.disconnect(false);
    }

    fn refresh_focus(&self) {
        let Some(manager) = load(&self.manager) else { return };
        if !self.thread_focused.get() {
            return;
        }
        // SAFETY: a valid thread manager.
        let next = unsafe { manager.GetFocus() }.ok().and_then(|document| unsafe { document.GetTop() }.ok());
        self.switch_context(next);
    }

    fn switch_context(&self, next: Option<ITfContext>) {
        if self.broker.busy() {
            store(&self.deferred_context, next);
            self.focus_deferred.set(true);
            self.bump();
            self.kill_timer();
            return;
        }
        let mut next = next;
        let current = load(&self.context);
        if same(next.as_ref(), current.as_ref()) {
            return;
        }
        self.bump();
        self.kill_timer();
        self.panel.hide();
        let old_composition = self.composition.try_borrow_mut().ok().and_then(|mut slot| slot.take());
        if let Some(old) = current {
            let reply = if self.broker.connected() && self.host_focused.get() {
                let reply = self.broker.request(&self.name(), "blur", Map::new());
                if reply.is_none() {
                    // A late blur (the app was saving the personal data) is no reason to keep the next field from
                    // connecting at once.  The composition's text stays in the document below.
                    self.broker.disconnect(false);
                }
                reply
            } else {
                None
            };
            let secure = self.secure.get();
            let atoms = self.atoms.get();
            let sink: ITfCompositionSink = self.to_interface();
            let document = old.clone();
            // Blur is bound to the old document, even if its edit lock is deferred.  With no broker reply, the
            // visible composition text stays in that document.
            let _ = composition::edit(&old, self.client.get(), false, move |cookie| {
                let slot = RefCell::new(old_composition);
                hresult(if secure || composition::is_secure(&document, cookie) {
                    composition::finish(cookie, &slot, true)
                } else if let Some(reply) = &reply {
                    composition::apply_reply(&document, cookie, &sink, &slot, reply, atoms)
                } else {
                    composition::finish(cookie, &slot, false)
                })
            });
        }
        if self.focus_deferred.replace(false) {
            next = self.deferred_context.try_borrow_mut().ok().and_then(|mut deferred| deferred.take());
        }
        store(&self.context, next.clone());
        self.host_focused.set(false);
        self.secure.set(true);
        self.rect.set(RECT::default());
        self.scale.set(1.0);
        self.set_name(String::new());
        if next.is_none() {
            // Nothing to type into here (the desktop, a window without a text field): the taskbar shows that
            // ZhuyinType has no field, not the 中/英 of the last one.
            if let Some(button) = self.button() {
                button.set_available(false);
            }
            self.broker.disconnect(false);
            return;
        }
        // SAFETY: no preconditions.
        let Ok(id) = (unsafe { CoCreateGuid() }) else { return };
        self.set_name(guid_string(&id));
        self.schedule();
    }

    /// Focuses or updates the host for the current context inside an edit session: secure state, caret, Caps Lock.
    /// False when the key (or poll) cannot go to the host now.
    fn prepare(&self, cookie: u32, caps_key: bool) -> bool {
        let generation = self.generation.get();
        let Some(context) = load(&self.context) else { return false };
        // An app that ended or restarted since the last key: reconnect now rather than lose this key.
        self.broker.check();
        let secure = self.restricted.get() || composition::is_secure(&context, cookie);
        if let Some(button) = self.button() {
            button.set_available(!secure);
        }
        if secure {
            self.applying.set(true);
            let result = composition::finish(cookie, &self.composition, true);
            self.applying.set(false);
            if result.is_err() {
                return false;
            }
        }
        let (rect, scale, owner) = composition::caret(&context, cookie);
        self.rect.set(rect);
        self.scale.set(scale);
        self.panel.set_owner(owner);
        self.panel.set_notify(self.window.get());
        if !self.broker.connected() {
            self.host_focused.set(false);
            // A reconnect starts a new host session; keep the earlier composition's text locally first.
            self.applying.set(true);
            let _ = composition::finish(cookie, &self.composition, secure);
            self.applying.set(false);
            if !self.broker.connect() {
                // Keys go to the application as they are until the app answers.
                if let Some(button) = self.button() {
                    button.set_available(false);
                }
                return false;
            }
        }
        let mut fields = Map::new();
        fields.insert("secure".into(), Value::from(secure));
        let private = !secure && composition::is_private(&context, cookie);
        self.private.set(private);
        fields.insert("private".into(), Value::from(private));
        composition::put_rect(&mut fields, rect, scale);
        if !self.host_focused.get() {
            if !caps_key {
                fields.insert("capsLock".into(), Value::from(caps_lock_on()));
            }
            // This text service draws the candidate window itself, inside the application (panel_window).
            fields.insert("panel".into(), Value::from(true));
            if self.activated.replace(false) {
                fields.insert("activated".into(), Value::from(true));
            }
            if !secure && !private {
                put_context(&mut fields, &context, cookie);
            }
            let reply = self.broker.request(&self.name(), "focus", fields);
            let Some(reply) = reply.filter(|_| generation == self.generation.get()) else { return false };
            self.host_focused.set(true);
            self.secure.set(secure);
            if !self.apply(cookie, &reply) {
                return false;
            }
        } else if secure != self.secure.get() {
            self.secure.set(secure);
            if !secure && !caps_key {
                fields.insert("capsLock".into(), Value::from(caps_lock_on()));
            }
            let reply = self.broker.request(&self.name(), "secure", fields);
            let Some(reply) = reply.filter(|_| generation == self.generation.get()) else { return false };
            if !self.apply(cookie, &reply) {
                return false;
            }
        }
        let window = self.window.get();
        if !window.is_invalid()
            && self.broker.connected()
            && !secure
            && self.panel.cold()
            && !self.warm_posted.replace(true)
        {
            // SAFETY: our own window.
            let _ = unsafe { PostMessageW(Some(window), WM_WARM, WPARAM(0), LPARAM(0)) };
        }
        true
    }

    /// Polls the host only while a candidate list is shown; an idle field (and the app) wakes for nothing.
    fn follow_list(&self) {
        let window = self.window.get();
        let wanted = !window.is_invalid()
            && self.broker.connected()
            && self.host_focused.get()
            && !self.secure.get()
            && self.panel.shows_list();
        if !wanted {
            self.kill_timer();
        } else if !self.polling.replace(true) {
            // SAFETY: our own window.
            unsafe { SetTimer(Some(window), POLL_TIMER, POLL_MS, None) };
        }
    }

    /// The first 中/英 badge in this application shows at once (`PanelWindow::warm`).
    fn warm(&self) {
        self.warm_posted.set(false);
        if self.thread_focused.get() && self.host_focused.get() && !self.secure.get() && load(&self.context).is_some() {
            self.panel.warm();
        }
    }

    /// The reply changes nothing that the last applied one showed: no text to commit, the same composition, list,
    /// mode and nothing to animate.
    fn unchanged(&self, reply: &Reply) -> bool {
        let last = self.last_reply.try_borrow().ok();
        let Some(Some(last)) = last.as_deref() else { return false };
        reply.commit.is_empty()
            && reply.panel == PanelUpdate::Unchanged
            && reply.toast.is_none()
            && reply.boundary.is_none()
            && (reply.english, reply.composing, &reply.preedit, reply.cursor, &reply.clauses, reply.focus, reply.target)
                == (last.english, last.composing, &last.preedit, last.cursor, &last.clauses, last.focus, last.target)
    }

    fn apply(&self, cookie: u32, reply: &Reply) -> bool {
        let Some(context) = load(&self.context) else { return false };
        self.applying.set(true);
        let result = if self.secure.get() {
            composition::finish(cookie, &self.composition, true)
        } else {
            let sink: ITfCompositionSink = self.to_interface();
            composition::apply_reply(&context, cookie, &sink, &self.composition, reply, self.atoms.get())
        };
        self.applying.set(false);
        if result.is_err() {
            self.broker.disconnect(true);
            self.host_focused.set(false);
            return false;
        }
        self.english.set(reply.english);
        self.composing.set(reply.composing);
        // The taskbar's 中/英 first: it costs nothing, the badge may have to make its window.
        if let Some(button) = self.button() {
            button.set_english(reply.english);
        }
        if self.secure.get() {
            self.panel.hide();
        } else {
            self.panel.apply(&reply.panel, reply.boundary, reply.toast.as_deref(), self.rect.get(), self.scale.get());
        }
        self.caps_at_reply.set(caps_lock_on());
        if let Ok(mut last) = self.last_reply.try_borrow_mut() {
            *last = Some(reply.clone());
        }
        self.follow_list();
        true
    }

    fn can_toggle(&self) -> bool {
        load(&self.context).is_some()
            && self.thread_focused.get()
            && self.host_focused.get()
            && !self.secure.get()
            && !self.broker.busy()
    }

    /// Opens the settings, or one place of them (`words`, `about`, `wizard`, `tour`; IPC `show-settings`).
    fn open_settings(&self, place: Option<&str>) {
        if self.broker.busy() {
            return;
        }
        if self.broker.connected() || self.broker.connect() {
            let name = self.name();
            let name = if name.is_empty() { "settings".to_owned() } else { name };
            let mut fields = Map::new();
            if let Some(place) = place {
                fields.insert("place".into(), Value::from(place));
            }
            let _ = self.broker.request(&name, "show-settings", fields);
        }
    }

    /// The right-click menu of the 中/英 button, the usual place for an input method's settings on Windows.
    fn show_menu(&self, point: POINT) {
        let _ = guard(|| {
            if self.menu_owner.get().is_invalid() {
                let owner = self.new_window(WS_EX_TOOLWINDOW, WS_POPUP, None)?;
                self.menu_owner.set(owner);
            }
            let owner = self.menu_owner.get();
            // SAFETY: a menu created, shown and destroyed here, owned by our window.
            unsafe {
                let menu = CreatePopupMenu()?;
                let toggle = if self.can_toggle() { MF_STRING } else { MF_STRING | MF_GRAYED };
                let label = if self.english.get() { w!("切換成中文") } else { w!("切換成英文") };
                let _ = AppendMenuW(menu, toggle, 1, label);
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                // The macOS input menu's items.
                let _ = AppendMenuW(menu, MF_STRING, 2, w!("ZhuyinType 設定…"));
                let _ = AppendMenuW(menu, MF_STRING, 3, w!("自訂詞與學到的詞…"));
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                let _ = AppendMenuW(menu, MF_STRING, 4, w!("設定嚮導…"));
                let _ = AppendMenuW(menu, MF_STRING, 5, w!("新功能導覽…"));
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                let _ = AppendMenuW(menu, MF_STRING, 6, w!("關於 ZhuyinType"));
                let _ = SetForegroundWindow(owner);
                let command = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RIGHTALIGN | TPM_NONOTIFY,
                    point.x,
                    point.y,
                    None,
                    owner,
                    None,
                );
                let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
                let _ = DestroyMenu(menu);
                match command.0 {
                    1 => self.toggle_mode(),
                    2 => self.open_settings(None),
                    3 => self.open_settings(Some("words")),
                    4 => self.open_settings(Some("wizard")),
                    5 => self.open_settings(Some("tour")),
                    6 => self.open_settings(Some("about")),
                    _ => {}
                }
            }
            Ok(())
        });
    }

    /// A click on the 中/英 button: switches the focused field's mode (IPC `mode`), not while composing or secure.
    fn toggle_mode(&self) {
        if self.broker.busy() || !self.thread_focused.get() || !self.host_focused.get() || self.secure.get() {
            return;
        }
        let Some(context) = load(&self.context) else { return };
        let generation = self.generation.get();
        let this = self.to_object();
        let _ = composition::edit(&context, self.client.get(), false, move |cookie| {
            if generation != this.generation.get() || !this.host_focused.get() || this.secure.get() {
                return S_OK;
            }
            // Where the caret is now (the badge goes there, or nowhere), not where it was at the last key; and the
            // app again if it restarted since.
            if !this.prepare(cookie, false) || generation != this.generation.get() || this.secure.get() {
                this.recover_deferred();
                return S_OK;
            }
            let mut fields = Map::new();
            fields.insert("english".into(), Value::from(!this.english.get()));
            let reply = this.broker.request(&this.name(), "mode", fields);
            if generation == this.generation.get()
                && let Some(reply) = reply
            {
                this.apply(cookie, &reply);
            }
            this.recover_deferred();
            S_OK
        });
    }

    fn recover_deferred(&self) {
        if self.broker.busy() {
            return;
        }
        if self.focus_deferred.replace(false) {
            let next = self.deferred_context.try_borrow_mut().ok().and_then(|mut deferred| deferred.take());
            self.switch_context(next);
        } else if self.reset_deferred.replace(false) && self.broker.connected() && self.host_focused.get() {
            let _ = self.broker.request(&self.name(), "reset", Map::new());
        }
    }

    fn key_event(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM, release: bool) -> Result<BOOL> {
        let Some(context) = context.cloned() else { return Ok(false.into()) };
        if !self.thread_focused.get() || self.restricted.get() || self.broker.busy() || !potential_key(vk) {
            return Ok(false.into());
        }
        // Only keys the test claimed reach here; TSF has already taken them from the application.
        let claimed = self.claims(vk, flags, release);
        if !same(Some(&context), load(&self.context).as_ref()) {
            self.switch_context(Some(context.clone()));
        }
        let handled = Rc::new(Cell::new(false));
        let generation = self.generation.get();
        let this = self.to_object();
        let result = Rc::clone(&handled);
        let hr = composition::edit(&context, self.client.get(), true, move |cookie| {
            this.key_session(cookie, vk, flags, release, claimed, generation, &result);
            S_OK
        });
        self.recover_deferred();
        if !self.broker.connected() {
            self.kill_timer();
        }
        Ok((hr.is_ok() && handled.get()).into())
    }

    #[allow(clippy::too_many_arguments)]
    fn key_session(
        &self,
        cookie: u32,
        vk: WPARAM,
        flags: LPARAM,
        release: bool,
        claimed: bool,
        generation: u64,
        handled: &Cell<bool>,
    ) {
        let caps_key = vk.0 == usize::from(VK_CAPITAL.0);
        if !self.prepare(cookie, caps_key) {
            if !self.focus_deferred.get() {
                self.applying.set(true);
                let _ = composition::finish(cookie, &self.composition, false);
                self.applying.set(false);
            }
            return;
        }
        if self.secure.get() {
            return; // No key identity or text leaves a secure context.
        }
        let key = translate_key(vk, flags, release);
        if key.key == 0 {
            return;
        }
        let mut fields = Map::new();
        fields.insert("key".into(), Value::from(key.key));
        fields.insert("modifiers".into(), Value::from(key.modifiers));
        fields.insert("release".into(), Value::from(key.release));
        fields.insert("repeat".into(), Value::from(key.repeat));
        fields.insert("shiftSide".into(), Value::from(key.shift_side));
        fields.insert("secure".into(), Value::from(false));
        fields.insert("private".into(), Value::from(self.private.get()));
        if !caps_key {
            fields.insert("capsLock".into(), Value::from(caps_lock_on()));
        }
        composition::put_rect(&mut fields, self.rect.get(), self.scale.get());
        // 前文 when a composition may start (as the Fcitx 5 addon sends it).
        if !release
            && !self.private.get()
            && !self.composing.get()
            && load(&self.composition).is_none()
            && let Some(context) = load(&self.context)
        {
            put_context(&mut fields, &context, cookie);
        }
        let reply = self.broker.request(&self.name(), "key", fields);
        if generation != self.generation.get() {
            return;
        }
        match reply {
            Some(reply) if self.apply(cookie, &reply) => {
                handled.set(reply.handled);
                // A typed character the host declined (the mode changed in between): put it in the document here
                // rather than lose it.
                if !reply.handled
                    && claimed
                    && !release
                    && !self.composing.get()
                    && let Some(typed) = typed_character(vk, flags)
                    && self.apply(cookie, &Reply::commit_only(vec![typed], self.english.get()))
                {
                    handled.set(true);
                }
            }
            _ => {
                self.applying.set(true);
                let _ = composition::finish(cookie, &self.composition, false);
                self.applying.set(false);
                self.host_focused.set(false);
            }
        }
    }

    /// Polls the host (candidate-window clicks, a changed composition) in an asynchronous edit session.
    fn schedule(&self) {
        if self.pending.get() || !self.thread_focused.get() || self.broker.busy() {
            return;
        }
        let Some(context) = load(&self.context) else { return };
        self.pending.set(true);
        let generation = self.generation.get();
        let this = self.to_object();
        let hr = composition::edit(&context, self.client.get(), false, move |cookie| {
            this.poll_session(cookie, generation);
            S_OK
        });
        if hr.is_err() {
            self.pending.set(false);
        }
    }

    fn poll_session(&self, cookie: u32, generation: u64) {
        self.pending.set(false);
        if generation != self.generation.get() {
            self.schedule();
            return;
        }
        if load(&self.context).is_none() || !self.thread_focused.get() {
            return;
        }
        // The field's first session focuses it.  Later ones only ask whether an open list changed: no caret, input
        // scope or document text is read, and an unchanged reply is not written back into the document (each of
        // those makes an application such as Notepad lay out its text again).
        if !self.host_focused.get() || !self.broker.connected() || self.secure.get() {
            if !self.prepare(cookie, false) {
                self.kill_timer();
                self.recover_deferred();
                return;
            }
        } else {
            self.broker.check();
        }
        if !self.panel.shows_list() || !self.broker.connected() {
            self.kill_timer();
            return;
        }
        let reply = self.broker.request(&self.name(), "poll", Map::new());
        if generation != self.generation.get() {
            self.recover_deferred();
            return;
        }
        match reply {
            Some(reply) if self.unchanged(&reply) => {}
            Some(reply) => {
                self.apply(cookie, &reply);
            }
            None => {
                self.host_focused.set(false);
                self.kill_timer();
            }
        }
    }

    /// A click in the candidate window (posted by `panel_window`): sent to the host in an edit session, whose reply
    /// is applied like a key's.
    fn panel_action(&self, action: usize, value: isize) {
        let Some(name) = action_name(action) else { return };
        if !self.thread_focused.get() || self.broker.busy() {
            return;
        }
        let Some(context) = load(&self.context) else { return };
        let generation = self.generation.get();
        let this = self.to_object();
        let _ = composition::edit(&context, self.client.get(), false, move |cookie| {
            this.action_session(cookie, generation, name, value);
            S_OK
        });
    }

    fn action_session(&self, cookie: u32, generation: u64, name: &str, value: isize) {
        if generation != self.generation.get() || !self.prepare(cookie, false) {
            return;
        }
        let mut fields = Map::new();
        fields.insert("action".into(), Value::from(name));
        fields.insert("value".into(), Value::from(value as i64));
        composition::put_rect(&mut fields, self.rect.get(), self.scale.get());
        let reply = self.broker.request(&self.name(), "action", fields);
        if generation != self.generation.get() {
            return;
        }
        if let Some(reply) = reply {
            self.apply(cookie, &reply);
        }
    }

    fn on_timer(&self, window: HWND) {
        if self.broker.connected() && self.polling.get() && self.panel.shows_list() {
            self.schedule();
        } else {
            self.polling.set(false);
            // SAFETY: our own window.
            let _ = unsafe { KillTimer(Some(window), POLL_TIMER) };
        }
    }

    fn set_thread_focus(&self, focused: bool) {
        self.thread_focused.set(focused);
        if focused {
            self.refresh_focus();
        } else {
            self.switch_context(None);
        }
    }

    fn test_key(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM, release: bool) -> Result<BOOL> {
        guard(|| {
            let eligible = !context.is_null()
                && !self.restricted.get()
                && self.thread_focused.get()
                && !self.broker.busy()
                && potential_key(vk);
            if !eligible {
                return Ok(false.into());
            }
            if self.claims(vk, flags, release) {
                return Ok(true.into());
            }
            self.observe(vk, flags, release);
            Ok(false.into())
        })
    }

    /// Tells the host about a key the application gets (`host_observes_key`): ⌫, Space and Return when nothing is
    /// composed.  Once per key, though TSF may test a key more than once.
    fn observe(&self, vk: WPARAM, flags: LPARAM, release: bool) {
        let composing = self.composing.get() || load(&self.composition).is_some();
        let key = translate_key(vk, flags, release);
        if !host_observes_key(&key, composing, self.english.get())
            || !self.host_focused.get()
            || self.secure.get()
            || !self.broker.connected()
        {
            return;
        }
        // SAFETY: no preconditions.
        let stamp = (vk.0, flags.0, unsafe { GetMessageTime() });
        if self.observed.replace(stamp) == stamp {
            return;
        }
        let mut fields = Map::new();
        fields.insert("key".into(), Value::from(key.key));
        fields.insert("modifiers".into(), Value::from(key.modifiers));
        fields.insert("release".into(), Value::from(false));
        fields.insert("repeat".into(), Value::from(key.repeat));
        fields.insert("secure".into(), Value::from(false));
        if let Some(reply) = self.broker.request(&self.name(), "key", fields) {
            self.english.set(reply.english);
        }
    }
}

/// The poll timer and the menu owner.  The window's user data points at the text service while the window exists.
unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: Windows calls this with valid arguments; the user data is cleared before the service goes away.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as _);
        }
        if message == WM_TIMER && wparam.0 == POLL_TIMER {
            let service = GetWindowLongPtrW(window, GWLP_USERDATA) as *const TextService_Impl;
            if !service.is_null() {
                let _ = catch_unwind(AssertUnwindSafe(|| (*service).on_timer(window)));
            }
            return LRESULT(0);
        }
        if message == WM_WARM {
            let service = GetWindowLongPtrW(window, GWLP_USERDATA) as *const TextService_Impl;
            if !service.is_null() {
                let _ = catch_unwind(AssertUnwindSafe(|| (*service).warm()));
            }
            return LRESULT(0);
        }
        if message == WM_PANEL_ACTION {
            let service = GetWindowLongPtrW(window, GWLP_USERDATA) as *const TextService_Impl;
            if !service.is_null() {
                let _ = catch_unwind(AssertUnwindSafe(|| (*service).panel_action(wparam.0, lparam.0)));
            }
            return LRESULT(0);
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, manager: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        ITfTextInputProcessorEx_Impl::ActivateEx(self, manager, client, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        guard(|| {
            self.deactivate();
            Ok(())
        })
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, manager: Ref<ITfThreadMgr>, client: u32, flags: u32) -> Result<()> {
        guard(|| {
            let manager = manager.ok()?.clone();
            if load(&self.manager).is_some() {
                return Err(E_INVALIDARG.into());
            }
            store(&self.manager, Some(manager.clone()));
            self.client.set(client);
            self.activated.set(SWITCHED_AWAY.swap(false, std::sync::atomic::Ordering::Relaxed));
            self.atoms.set(display_attributes::atoms());
            // TSF secure-mode and UI-less activation cannot use a desktop candidate window.
            self.restricted.set(flags & (TF_TMAE_SECUREMODE | TF_TMAE_COMLESS | TF_TMAE_UIELEMENTENABLEDONLY) != 0);
            // The message window and the 中/英 button come first: advising the key sink already reports the focus
            // (`OnSetFocus`), and handling it starts the poll timer on that window and shows the field's 中/英.
            let sinks = || -> Result<()> {
                self.create_window()?;
                self.add_mode_button(&manager);
                let source = manager.cast::<ITfSource>()?;
                store(&self.source, Some(source.clone()));
                let events: ITfThreadMgrEventSink = self.to_interface();
                let focus: ITfThreadFocusSink = self.to_interface();
                // SAFETY: valid sinks of this object.
                unsafe {
                    self.manager_cookie.set(source.AdviseSink(&ITfThreadMgrEventSink::IID, &events)?);
                    self.focus_cookie.set(source.AdviseSink(&ITfThreadFocusSink::IID, &focus)?);
                    let keys = manager.cast::<ITfKeystrokeMgr>()?;
                    store(&self.keys, Some(keys.clone()));
                    let sink: ITfKeyEventSink = self.to_interface();
                    keys.AdviseKeyEventSink(client, &sink, true)?;
                }
                Ok(())
            };
            if let Err(error) = sinks() {
                self.deactivate();
                return Err(error);
            }
            // SAFETY: no preconditions.
            self.thread_focused.set(!unsafe { GetFocus() }.is_invalid());
            self.refresh_focus();
            Ok(())
        })
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _document: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, document: Ref<ITfDocumentMgr>) -> Result<()> {
        guard(|| {
            if let Some(context) = load(&self.context) {
                // SAFETY: a valid context.
                let owner = unsafe { context.GetDocumentMgr() }.ok();
                if same(owner.as_ref(), document.as_ref()) {
                    self.switch_context(None);
                }
            }
            Ok(())
        })
    }

    fn OnSetFocus(&self, _focus: Ref<ITfDocumentMgr>, _previous: Ref<ITfDocumentMgr>) -> Result<()> {
        guard(|| {
            self.refresh_focus();
            Ok(())
        })
    }

    fn OnPushContext(&self, _context: Ref<ITfContext>) -> Result<()> {
        guard(|| {
            self.refresh_focus();
            Ok(())
        })
    }

    fn OnPopContext(&self, popped: Ref<ITfContext>) -> Result<()> {
        guard(|| {
            if same(load(&self.context).as_ref(), popped.as_ref()) {
                self.switch_context(None);
            }
            self.refresh_focus();
            Ok(())
        })
    }
}

impl ITfThreadFocusSink_Impl for TextService_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        guard(|| {
            self.set_thread_focus(true);
            Ok(())
        })
    }

    fn OnKillThreadFocus(&self) -> Result<()> {
        guard(|| {
            self.set_thread_focus(false);
            Ok(())
        })
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        guard(|| {
            self.set_thread_focus(foreground.as_bool());
            Ok(())
        })
    }

    fn OnTestKeyDown(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.test_key(context, vk, flags, false)
    }

    fn OnTestKeyUp(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.test_key(context, vk, flags, true)
    }

    fn OnKeyDown(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM) -> Result<BOOL> {
        guard(|| self.key_event(context, vk, flags, false)).or(Ok(false.into()))
    }

    fn OnKeyUp(&self, context: Ref<ITfContext>, vk: WPARAM, flags: LPARAM) -> Result<BOOL> {
        guard(|| self.key_event(context, vk, flags, true)).or(Ok(false.into()))
    }

    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const windows::core::GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(&self, _cookie: u32, ended: Ref<ITfComposition>) -> Result<()> {
        guard(|| {
            if !ended.is_null() && same(load(&self.composition).as_ref(), ended.as_ref()) {
                store(&self.composition, None);
                self.forget_reply();
                if !self.applying.get() && self.broker.connected() && !self.name().is_empty() {
                    if self.broker.busy() {
                        self.reset_deferred.set(true);
                        self.bump();
                    } else {
                        let _ = self.broker.request(&self.name(), "reset", Map::new());
                    }
                }
            }
            Ok(())
        })
    }
}

/// The composition's display attributes (display_attributes.rs).  TSF also asks a text service that was created
/// only for this, never activated.
impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        guard(|| Ok(display_attributes::Enumeration::create(0)))
    }

    fn GetDisplayAttributeInfo(&self, guid: *const windows::core::GUID) -> Result<ITfDisplayAttributeInfo> {
        guard(|| {
            if guid.is_null() {
                return Err(E_INVALIDARG.into());
            }
            // SAFETY: checked for null.
            display_attributes::info(unsafe { &*guid })
        })
    }
}
