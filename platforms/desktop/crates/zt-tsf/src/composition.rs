//! Edit sessions and what runs inside them: applying a broker reply to the document (commit, composition, caret),
//! ending a composition, telling a secure field, and finding the caret on screen.
//!
//! Every text or range operation runs under the context's edit cookie.  No `RefCell` borrow is held across a TSF
//! call: TSF may call back into the text service from inside one (`OnCompositionTerminated`, a nested edit session).

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;

use serde_json::{Map, Value};
use windows::Win32::Foundation::{E_FAIL, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MapWindowPoints, MonitorFromRect,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::System::Variant::{VARIANT, VT_I4, VT_UNKNOWN, VariantClear};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, LogicalToPhysicalPointForPerMonitorDPI,
    SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::Shell::GetScaleFactorForMonitor;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, GUID_PROP_INPUTSCOPE, IS_ALPHANUMERIC_PIN,
    IS_ALPHANUMERIC_PIN_SET, IS_NUMERIC_PASSWORD, IS_NUMERIC_PIN, IS_PASSWORD, IS_PRIVATE, ITfCompartmentMgr,
    ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition, ITfEditSession, ITfEditSession_Impl,
    ITfInputScope, ITfRange, InputScope, TF_AE_NONE, TF_ANCHOR_START, TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    TF_DEFAULT_SELECTION, TF_ES_ASYNCDONTCARE, TF_ES_READWRITE, TF_ES_SYNC, TF_SELECTION, TF_SELECTIONSTYLE,
    TS_SD_READONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GWL_STYLE, GetClassNameW, GetGUIThreadInfo, GetWindowLongPtrW,
};
use windows::core::{BOOL, GUID, HRESULT, Interface, Result, implement};

use crate::coordinates::logical_rect;
use crate::module::{ObjectCount, guard};
use crate::protocol::Reply;

/// The text service's composition, if it has one.
pub(crate) type Slot = RefCell<Option<ITfComposition>>;

pub(crate) fn load<T: Clone>(cell: &RefCell<Option<T>>) -> Option<T> {
    cell.try_borrow().ok().and_then(|value| value.clone())
}

/// Replaces the value; the old one is released after the borrow ends.
pub(crate) fn store<T>(cell: &RefCell<Option<T>>, value: Option<T>) {
    let old = match cell.try_borrow_mut() {
        Ok(mut slot) => std::mem::replace(&mut *slot, value),
        Err(_) => value,
    };
    drop(old);
}

/// What an edit session runs, with its edit cookie.
type Action = Box<dyn FnOnce(u32) -> HRESULT>;

#[implement(ITfEditSession)]
struct EditSession {
    action: Cell<Option<Action>>,
    _count: ObjectCount,
}

impl ITfEditSession_Impl for EditSession_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guard(|| match self.action.take() {
            Some(action) => action(cookie).ok(),
            None => Ok(()),
        })
    }
}

/// Requests a read/write edit session that runs `action`; synchronous sessions run before this returns or not at
/// all.  The session's own result when it ran, otherwise the request's error.
pub(crate) fn edit(
    context: &ITfContext,
    client: u32,
    sync: bool,
    action: impl FnOnce(u32) -> HRESULT + 'static,
) -> HRESULT {
    let session: ITfEditSession =
        EditSession { action: Cell::new(Some(Box::new(action))), _count: ObjectCount::new() }.into();
    let mode = if sync { TF_ES_SYNC } else { TF_ES_ASYNCDONTCARE };
    let flags = TF_CONTEXT_EDIT_CONTEXT_FLAGS(mode.0 | TF_ES_READWRITE.0);
    // SAFETY: a valid context and session.
    match unsafe { context.RequestEditSession(client, &session, flags) } {
        Ok(result) => result,
        Err(error) => error.code(),
    }
}

fn compartment_set(context: &ITfContext, id: &GUID) -> bool {
    let Ok(manager) = context.cast::<ITfCompartmentMgr>() else { return false };
    // SAFETY: a valid GUID; the VARIANT is cleared below.
    unsafe {
        let Ok(compartment) = manager.GetCompartment(id) else { return false };
        let Ok(mut value) = compartment.GetValue() else { return false };
        let set = value.Anonymous.Anonymous.vt == VT_I4 && value.Anonymous.Anonymous.Anonymous.lVal != 0;
        let _ = VariantClear(&mut value);
        set
    }
}

/// The default selection's range.
pub(crate) fn selection(context: &ITfContext, cookie: u32) -> Result<ITfRange> {
    let mut selection = [TF_SELECTION::default()];
    let mut fetched = 0u32;
    // SAFETY: a writable one-element array; the range it returns is owned here.
    unsafe {
        context.GetSelection(cookie, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)?;
        let range = ManuallyDrop::take(&mut selection[0].range);
        if fetched == 0 { Err(E_FAIL.into()) } else { range.ok_or_else(|| E_FAIL.into()) }
    }
}

fn move_caret(context: &ITfContext, cookie: u32, range: &ITfRange, cursor: usize) -> Result<()> {
    let cursor = i32::try_from(cursor).map_err(|_| windows::core::Error::from(E_FAIL))?;
    // SAFETY: valid ranges under the edit cookie.
    unsafe {
        let caret = range.Clone()?;
        caret.Collapse(cookie, TF_ANCHOR_START)?;
        let mut moved = 0;
        caret.ShiftStart(cookie, cursor, &mut moved, std::ptr::null())?;
        if moved != cursor {
            return Err(E_FAIL.into());
        }
        caret.Collapse(cookie, TF_ANCHOR_START)?;
        let mut selection = [TF_SELECTION {
            range: ManuallyDrop::new(Some(caret)),
            style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: false.into() },
        }];
        let result = context.SetSelection(cookie, &selection);
        ManuallyDrop::drop(&mut selection[0].range);
        result
    }
}

/// Ends the composition in `slot`, first emptying it when `discard`; the visible text stays otherwise.
pub(crate) fn finish(cookie: u32, slot: &Slot, discard: bool) -> Result<()> {
    let Some(composition) = load(slot) else { return Ok(()) };
    // SAFETY: a valid composition under the write cookie.
    unsafe {
        if discard {
            composition.GetRange()?.SetText(cookie, 0, &[]).map_err(|_| windows::core::Error::from(E_FAIL))?;
        }
        composition.EndComposition(cookie)?;
    }
    store(slot, None);
    Ok(())
}

/// Puts a reply into the document: the commit replaces the composition (or the selection) and the caret goes after
/// it; then the composition shows `preedit` with the caret at `cursor`, or ends when it is empty.
pub(crate) fn apply_reply(
    context: &ITfContext,
    cookie: u32,
    sink: &ITfCompositionSink,
    slot: &Slot,
    reply: &Reply,
) -> Result<()> {
    // SAFETY: valid TSF objects under the write cookie.
    unsafe {
        if !reply.commit.is_empty() {
            let range = match load(slot) {
                Some(composition) => composition.GetRange()?,
                None => selection(context, cookie)?,
            };
            range.SetText(cookie, 0, &reply.commit)?;
            move_caret(context, cookie, &range, reply.commit.len())?;
            finish(cookie, slot, false)?;
        }
        if reply.preedit.is_empty() {
            return finish(cookie, slot, true);
        }
        let composition = match load(slot) {
            Some(composition) => composition,
            None => {
                let range = selection(context, cookie)?;
                let owner = context.cast::<ITfContextComposition>()?;
                let composition = owner.StartComposition(cookie, &range, sink)?;
                store(slot, Some(composition.clone()));
                composition
            }
        };
        let range = composition.GetRange()?;
        range.SetText(cookie, 0, &reply.preedit)?;
        move_caret(context, cookie, &range, reply.cursor)
    }
}

// GetWindowLongPtrW is GetWindowLongW (i32) on 32-bit Windows.
#[allow(clippy::unnecessary_cast)]
fn is_password_edit(window: HWND) -> bool {
    const ES_PASSWORD: isize = 0x20;
    let mut class = [0u16; 64];
    // SAFETY: a writable buffer.
    let length = usize::try_from(unsafe { GetClassNameW(window, &mut class) }).unwrap_or(0);
    let class = String::from_utf16_lossy(&class[..length.min(class.len())]);
    // ES_PASSWORD shares a bit with unrelated window styles: only edit controls count.
    let edit = class.eq_ignore_ascii_case("Edit")
        || class.get(..8).is_some_and(|start| start.eq_ignore_ascii_case("RichEdit"));
    // SAFETY: a window handle from the system.
    edit && (unsafe { GetWindowLongPtrW(window, GWL_STYLE) } as isize) & ES_PASSWORD != 0
}

fn gui_thread() -> Option<GUITHREADINFO> {
    let mut gui = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    // SAFETY: cbSize is set.
    unsafe { GetGUIThreadInfo(GetCurrentThreadId(), &mut gui) }.ok().map(|()| gui)
}

/// A field where nothing typed may leave the application: read-only or disabled contexts, password edit controls,
/// and input scopes for passwords, PINs and private text.  Unknown counts as secure.
pub(crate) fn is_secure(context: &ITfContext, cookie: u32) -> bool {
    // SAFETY: a valid context.
    let Ok(status) = (unsafe { context.GetStatus() }) else { return true };
    if status.dwDynamicFlags & TS_SD_READONLY != 0
        || compartment_set(context, &GUID_COMPARTMENT_KEYBOARD_DISABLED)
        || compartment_set(context, &GUID_COMPARTMENT_EMPTYCONTEXT)
    {
        return true;
    }
    if gui_thread().is_some_and(|gui| !gui.hwndFocus.is_invalid() && is_password_edit(gui.hwndFocus)) {
        return true;
    }
    let Ok(range) = selection(context, cookie) else { return true };
    // SAFETY: valid objects; the VARIANT is cleared and the scope array freed.
    unsafe {
        let Ok(property) = context.GetAppProperty(&GUID_PROP_INPUTSCOPE) else { return false };
        let Ok(mut value) = property.GetValue(cookie, &range) else { return false };
        let mut secure = false;
        if value.Anonymous.Anonymous.vt == VT_UNKNOWN
            && let Some(unknown) = value.Anonymous.Anonymous.Anonymous.punkVal.as_ref()
            && let Ok(scopes) = unknown.cast::<ITfInputScope>()
        {
            let mut values: *mut InputScope = std::ptr::null_mut();
            let mut count = 0u32;
            if scopes.GetInputScopes(&mut values, &mut count).is_ok() && !values.is_null() {
                let list = std::slice::from_raw_parts(values, count as usize);
                secure = list.iter().any(|scope| {
                    [
                        IS_PASSWORD,
                        IS_NUMERIC_PASSWORD,
                        IS_NUMERIC_PIN,
                        IS_ALPHANUMERIC_PIN,
                        IS_ALPHANUMERIC_PIN_SET,
                        IS_PRIVATE,
                    ]
                    .contains(scope)
                });
                CoTaskMemFree(Some(values.cast_const().cast()));
            }
        }
        let _ = VariantClear(&mut value as *mut VARIANT);
        secure
    }
}

/// Runs `body` with this thread per-monitor DPI aware, so monitor rectangles and scale factors are physical.
pub(crate) fn per_monitor<T>(body: impl FnOnce() -> T) -> T {
    // SAFETY: changes only this thread's awareness, restored below.
    let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let result = body();
    if previous != DPI_AWARENESS_CONTEXT::default() {
        // SAFETY: restores the context returned above.
        unsafe { SetThreadDpiAwarenessContext(previous) };
    }
    result
}

/// The caret (the selection's text extent, else the Win32 caret) in physical screen pixels, the scale of the monitor
/// it is on, and the window that shows the text (the candidate window's owner; the focus window when the view has
/// none).  Text stores report in their window's DPI space, even DPI-unaware ones; that is undone first.
pub(crate) fn caret(context: &ITfContext, cookie: u32) -> (RECT, f64, HWND) {
    let mut rect = RECT::default();
    // SAFETY: valid objects under the edit cookie.
    unsafe {
        let Ok(view) = context.GetActiveView() else { return (rect, 1.0, GetFocus()) };
        let mut window = view.GetWnd().unwrap_or_default();
        let mut clipped = BOOL::default();
        let extent =
            selection(context, cookie).and_then(|range| view.GetTextExt(cookie, &range, &mut rect, &mut clipped));
        if extent.is_err()
            && let Some(gui) = gui_thread()
            && !gui.hwndCaret.is_invalid()
        {
            let mut points = [
                POINT { x: gui.rcCaret.left, y: gui.rcCaret.top },
                POINT { x: gui.rcCaret.right, y: gui.rcCaret.bottom },
            ];
            MapWindowPoints(Some(gui.hwndCaret), None, &mut points);
            rect = RECT { left: points[0].x, top: points[0].y, right: points[1].x, bottom: points[1].y };
            window = gui.hwndCaret;
        }
        let mut a = POINT { x: rect.left, y: rect.top };
        let mut b = POINT { x: rect.right, y: rect.bottom };
        if !window.is_invalid()
            && LogicalToPhysicalPointForPerMonitorDPI(Some(window), &mut a).as_bool()
            && LogicalToPhysicalPointForPerMonitorDPI(Some(window), &mut b).as_bool()
        {
            rect = RECT { left: a.x, top: a.y, right: b.x, bottom: b.y };
        }
        let scale = per_monitor(|| {
            GetScaleFactorForMonitor(MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST))
                .map_or(1.0, |factor| f64::from(factor.0) / 100.0)
        });
        let owner = if window.is_invalid() { GetFocus() } else { window };
        (rect, scale, owner)
    }
}

/// Adds `rect` (logical pixels around the caret's monitor origin) and `scale: 1` to a request.
pub(crate) fn put_rect(fields: &mut Map<String, Value>, rect: RECT, scale: f64) {
    let origin = per_monitor(|| {
        let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: cbSize is set.
        let _ = unsafe { GetMonitorInfoW(MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST), &mut monitor) };
        (monitor.rcMonitor.left, monitor.rcMonitor.top)
    });
    let logical = logical_rect([rect.left, rect.top, rect.right, rect.bottom], origin, scale);
    fields.insert("rect".into(), Value::from(logical.to_vec()));
    fields.insert("scale".into(), Value::from(1));
}
