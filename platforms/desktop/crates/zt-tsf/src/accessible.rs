//! The candidate window and the 中/英 badge for screen readers (Narrator, NVDA, JAWS): a UI Automation provider for
//! each window (the list: a List control named after the highlighted candidate; the badge: Text) and notifications
//! when a list opens, its page turns, its highlight moves, a notice appears or the mode changes (`announce` has the
//! words).
//!
//! UIAutomationCore.dll is loaded only when it can matter, so an application that loads the text service loads
//! nothing more: when a client asked one of our windows for its provider (WM_GETOBJECT), when a screen reader says it
//! is running (SPI_GETSCREENREADER), or when the application already has it.  Notifications are raised only while
//! a client listens (UiaClientsAreListening), so typing costs one cheap check otherwise.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::OnceLock;

use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, NotificationKind_ActionCompleted,
    NotificationProcessing_MostRecent, ProviderOptions, ProviderOptions_ServerSideProvider,
    ProviderOptions_UseComThreading, UIA_AutomationIdPropertyId, UIA_CONTROLTYPE_ID, UIA_ControlTypePropertyId,
    UIA_IsContentElementPropertyId, UIA_IsControlElementPropertyId, UIA_IsKeyboardFocusablePropertyId,
    UIA_ListControlTypeId, UIA_NamePropertyId, UIA_PATTERN_ID, UIA_PROPERTY_ID, UIA_ProviderDescriptionPropertyId,
    UIA_TextControlTypeId, UiaRootObjectId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETSCREENREADER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{BOOL, BSTR, ComObject, HRESULT, IUnknown, Interface, PCSTR, Result, implement, s, w};

/// UIAutomationCore's functions this module uses, looked up when first needed.
struct Uia {
    clients_are_listening: unsafe extern "system" fn() -> BOOL,
    host_provider_from_hwnd: unsafe extern "system" fn(HWND, *mut *mut c_void) -> HRESULT,
    raise_notification_event: unsafe extern "system" fn(*mut c_void, i32, i32, *const u16, *const u16) -> HRESULT,
    return_raw_element_provider: unsafe extern "system" fn(HWND, WPARAM, LPARAM, *mut c_void) -> LRESULT,
    disconnect_provider: unsafe extern "system" fn(*mut c_void) -> HRESULT,
}

static UIA: OnceLock<Option<Uia>> = OnceLock::new();

impl Uia {
    fn load(library: HMODULE) -> Option<Self> {
        type Export = unsafe extern "system" fn() -> isize;
        // SAFETY: a lookup in a loaded module.
        let find = |name: PCSTR| unsafe { GetProcAddress(library, name) };
        // SAFETY: each export cast to its documented signature.
        unsafe {
            Some(Self {
                clients_are_listening: std::mem::transmute::<Export, unsafe extern "system" fn() -> BOOL>(find(s!(
                    "UiaClientsAreListening"
                ))?),
                host_provider_from_hwnd: std::mem::transmute::<
                    Export,
                    unsafe extern "system" fn(HWND, *mut *mut c_void) -> HRESULT,
                >(find(s!("UiaHostProviderFromHwnd"))?),
                raise_notification_event: std::mem::transmute::<
                    Export,
                    unsafe extern "system" fn(*mut c_void, i32, i32, *const u16, *const u16) -> HRESULT,
                >(find(s!("UiaRaiseNotificationEvent"))?),
                return_raw_element_provider: std::mem::transmute::<
                    Export,
                    unsafe extern "system" fn(HWND, WPARAM, LPARAM, *mut c_void) -> LRESULT,
                >(find(s!("UiaReturnRawElementProvider"))?),
                disconnect_provider: std::mem::transmute::<Export, unsafe extern "system" fn(*mut c_void) -> HRESULT>(
                    find(s!("UiaDisconnectProvider"))?,
                ),
            })
        }
    }

    /// UIAutomationCore, loaded now (`load`) or only if the application already has it.
    fn get(load: bool) -> Option<&'static Self> {
        if let Some(uia) = UIA.get() {
            return uia.as_ref();
        }
        // SAFETY: plain module lookups; the library stays loaded for the life of the process.
        let library = unsafe {
            match GetModuleHandleW(w!("UIAutomationCore.dll")) {
                Ok(library) => library,
                Err(_) if load => {
                    match LoadLibraryExW(w!("UIAutomationCore.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) {
                        Ok(library) => library,
                        Err(_) => return UIA.get_or_init(|| None).as_ref(),
                    }
                }
                Err(_) => return None,
            }
        };
        UIA.get_or_init(|| Self::load(library)).as_ref()
    }
}

/// Whether a screen reader says it is running.
fn screen_reader() -> bool {
    let mut on = BOOL(0);
    // SAFETY: SPI_GETSCREENREADER writes one BOOL through the pointer, which outlives the call.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETSCREENREADER,
            0,
            Some((&raw mut on).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_ok() && on.as_bool()
}

/// What a window is for a screen reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// The candidate window: a list.
    List,
    /// The 中/英 badge: text.
    Badge,
}

/// One window's provider.
#[implement(IRawElementProviderSimple)]
struct Provider {
    window: Cell<HWND>,
    role: Role,
    name: RefCell<String>,
}

impl IRawElementProviderSimple_Impl for Provider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions(ProviderOptions_ServerSideProvider.0 | ProviderOptions_UseComThreading.0))
    }

    fn GetPatternProvider(&self, _pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        // No patterns: S_OK with nothing.
        Err(windows::core::Error::empty())
    }

    // The property ids are the SDK's constants, named as there.
    #[allow(non_upper_case_globals)]
    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let control: UIA_CONTROLTYPE_ID = match self.role {
            Role::List => UIA_ListControlTypeId,
            Role::Badge => UIA_TextControlTypeId,
        };
        Ok(match property {
            UIA_ControlTypePropertyId => VARIANT::from(control.0),
            UIA_NamePropertyId => VARIANT::from(BSTR::from(self.name.borrow().as_str())),
            UIA_AutomationIdPropertyId => VARIANT::from(BSTR::from(match self.role {
                Role::List => "ZhuyinType.Candidates",
                Role::Badge => "ZhuyinType.Mode",
            })),
            UIA_ProviderDescriptionPropertyId => VARIANT::from(BSTR::from("ZhuyinType")),
            UIA_IsKeyboardFocusablePropertyId => VARIANT::from(false),
            UIA_IsControlElementPropertyId | UIA_IsContentElementPropertyId => VARIANT::from(true),
            _ => VARIANT::default(),
        })
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        let window = self.window.get();
        let uia = Uia::get(false).ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        if window.is_invalid() {
            return Err(windows::core::Error::empty());
        }
        let mut host: *mut c_void = std::ptr::null_mut();
        // SAFETY: our own window; the out pointer receives an owned reference.
        unsafe { (uia.host_provider_from_hwnd)(window, &raw mut host) }.ok()?;
        if host.is_null() {
            return Err(windows::core::Error::empty());
        }
        // SAFETY: a provider UIAutomationCore just returned, whose reference is ours now.
        Ok(unsafe { IRawElementProviderSimple::from_raw(host) })
    }
}

/// A window's accessibility: its provider, made when a client first asks or there is something to announce.
pub(crate) struct Accessible {
    role: Role,
    provider: RefCell<Option<ComObject<Provider>>>,
    /// A client asked this window for its provider.
    asked: Cell<bool>,
}

impl Accessible {
    pub(crate) fn new(role: Role) -> Self {
        Self { role, provider: RefCell::new(None), asked: Cell::new(false) }
    }

    fn provider(&self, window: HWND) -> ComObject<Provider> {
        let mut provider = self.provider.borrow_mut();
        let object = provider.get_or_insert_with(|| {
            ComObject::new(Provider { window: Cell::new(window), role: self.role, name: RefCell::new(String::new()) })
        });
        object.window.set(window);
        object.clone()
    }

    /// The window's name for clients that read it (the highlighted candidate, 中文 or 英文).
    /// Built only when a client has the provider.
    pub(crate) fn set_name(&self, name: impl FnOnce() -> String) {
        if let Some(provider) = self.provider.borrow().as_ref() {
            *provider.name.borrow_mut() = name();
        }
    }

    /// Whether announcing is worth building the words: a client is listening.  One cheap check when no screen reader
    /// runs; nothing loaded unless one does or a client already asked.
    pub(crate) fn listening(&self) -> bool {
        let load = self.asked.get() || screen_reader();
        // SAFETY: a function without arguments.
        Uia::get(load).is_some_and(|uia| unsafe { (uia.clients_are_listening)() }.as_bool())
    }

    /// Says `text` (a notification from `window`), replacing anything said before with the same `activity`.
    pub(crate) fn announce(&self, window: HWND, text: &str, activity: &str) {
        if window.is_invalid() || text.is_empty() {
            return;
        }
        let Some(uia) = Uia::get(true) else { return };
        let provider = self.provider(window);
        if provider.name.borrow().is_empty() {
            text.clone_into(&mut provider.name.borrow_mut());
        }
        let interface: IRawElementProviderSimple = provider.to_interface();
        let (text, activity) = (BSTR::from(text), BSTR::from(activity));
        // SAFETY: a live provider and two BSTRs that outlive the call.
        let _ = unsafe {
            (uia.raise_notification_event)(
                interface.as_raw(),
                NotificationKind_ActionCompleted.0,
                NotificationProcessing_MostRecent.0,
                text.as_ptr(),
                activity.as_ptr(),
            )
        };
    }

    /// WM_GETOBJECT: the provider for UI Automation's root object id, else None (the default handling).
    pub(crate) fn get_object(&self, window: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if lparam.0 as i32 != UiaRootObjectId {
            return None;
        }
        self.asked.set(true);
        let uia = Uia::get(true)?;
        let interface: IRawElementProviderSimple = self.provider(window).to_interface();
        // SAFETY: our own window and a live provider (UIAutomationCore takes its own reference).
        Some(unsafe { (uia.return_raw_element_provider)(window, wparam, lparam, interface.as_raw()) })
    }

    /// The window is going away: clients let go of its provider.
    pub(crate) fn disconnect(&self, window: HWND) {
        let Some(provider) = self.provider.borrow_mut().take() else { return };
        provider.window.set(HWND::default());
        let Some(uia) = Uia::get(false) else { return };
        // SAFETY: as UI Automation asks of a window being destroyed: no more provider for it, then disconnect ours.
        unsafe {
            if !window.is_invalid() && self.asked.get() {
                (uia.return_raw_element_provider)(window, WPARAM(0), LPARAM(0), std::ptr::null_mut());
            }
            let interface: IRawElementProviderSimple = provider.to_interface();
            let _ = (uia.disconnect_provider)(interface.as_raw());
        }
        self.asked.set(false);
    }
}
