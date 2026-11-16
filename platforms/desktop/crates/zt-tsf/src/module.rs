//! The DLL's identity and its COM entry points: the class factory, the unload check, and registration
//! (`regsvr32`, and `regsvr32 /n /i:user` for the signed-in user's input method list).

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER, HINSTANCE, HMODULE, S_FALSE,
    S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::GetModuleHandleExW;
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use crate::registration;
use crate::text_service::TextService;

/// The text service's COM class.  The same as the C++ text service it replaces, so an installed profile stays valid.
pub(crate) const SERVICE_ID: GUID = GUID::from_u128(0x25bc3548_b611_4f92_91d0_c9291d427ea8);
/// The language profile of the text service.
pub(crate) const PROFILE_ID: GUID = GUID::from_u128(0xe8207cf3_7ce3_42ae_9b18_453284a73901);
/// Traditional Chinese (Taiwan).
pub(crate) const LANGUAGE: u16 = 0x0404;
/// The name in Settings › Time & language and in the Win+Space list.
pub(crate) const NAME: &str = "ZhuyinType 注音";

static OBJECTS: AtomicIsize = AtomicIsize::new(0);

/// Held by every COM object this DLL hands out, so `DllCanUnloadNow` answers no while any is alive.
pub(crate) struct ObjectCount(());

impl ObjectCount {
    pub(crate) fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for ObjectCount {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Runs a COM method body; a panic becomes `E_FAIL` instead of unwinding into the application.
pub(crate) fn guard<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

fn guard_hresult(body: impl FnOnce() -> HRESULT) -> HRESULT {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(E_FAIL)
}

/// This DLL's module handle.
pub(crate) fn module() -> HMODULE {
    const FROM_ADDRESS: u32 = 0x4;
    const UNCHANGED_REFCOUNT: u32 = 0x2;
    static ANCHOR: u8 = 0;
    let mut handle = HMODULE::default();
    let address = std::ptr::addr_of!(ANCHOR).cast::<u16>();
    // SAFETY: the address is inside this module; the handle is not reference counted.
    let _ = unsafe { GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, PCWSTR(address), &mut handle) };
    handle
}

pub(crate) fn instance() -> HINSTANCE {
    HINSTANCE(module().0)
}

#[implement(IClassFactory)]
struct Factory {
    _count: ObjectCount,
}

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, iid: *const GUID, object: *mut *mut c_void) -> Result<()> {
        guard(|| {
            if object.is_null() || iid.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: checked for null; COM callers pass a writable out pointer.
            unsafe { *object = std::ptr::null_mut() };
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let service: IUnknown = TextService::create();
            // SAFETY: iid and object are valid pointers from the caller.
            unsafe { service.query(iid, object) }.ok()
        })
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            OBJECTS.fetch_add(1, Ordering::SeqCst);
        } else {
            OBJECTS.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// COM: the class factory of the text service.
///
/// # Safety
/// Called by COM with valid pointers (`out` writable).
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    guard_hresult(|| {
        if out.is_null() || clsid.is_null() || iid.is_null() {
            return E_POINTER;
        }
        // SAFETY: checked for null above.
        unsafe { *out = std::ptr::null_mut() };
        // SAFETY: as above.
        if unsafe { *clsid } != SERVICE_ID {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory { _count: ObjectCount::new() }.into();
        // SAFETY: iid and out come from the caller.
        unsafe { factory.query(iid, out) }
    })
}

/// COM: whether the DLL may be unloaded (no object of it is alive).
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) == 0 { S_OK } else { S_FALSE }
}

/// `regsvr32`: registers the COM class for the machine, the TSF profile and its categories.  Needs an administrator.
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    guard_hresult(|| registration::register().map_or_else(|error| error.code(), |()| S_OK))
}

/// `regsvr32 /u`: removes what `DllRegisterServer` added.
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    guard_hresult(|| registration::unregister().map_or_else(|error| error.code(), |()| S_OK))
}

/// `regsvr32 /n /i:user DLL` adds ZhuyinType to the signed-in user's input methods (the Win+Space list);
/// `regsvr32 /u /n /i:user DLL` removes it.  The installer runs these for the user who installs, and Active Setup
/// runs the first for every other user at their next sign-in.
///
/// # Safety
/// `command` is null or a NUL-terminated string, as regsvr32 passes it.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllInstall(install: BOOL, command: PCWSTR) -> HRESULT {
    guard_hresult(|| {
        // SAFETY: regsvr32 passes a NUL-terminated string or null.
        let command =
            if command.is_null() { String::new() } else { unsafe { command.to_string() }.unwrap_or_default() };
        if !command.eq_ignore_ascii_case("user") {
            return E_INVALIDARG;
        }
        let result = if install.as_bool() { registration::enable_for_user() } else { registration::disable_for_user() };
        result.map_or_else(|error| error.code(), |()| S_OK)
    })
}
