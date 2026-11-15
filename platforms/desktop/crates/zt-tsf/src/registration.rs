//! Registration: the COM class (machine-wide, InprocServer32, Apartment), the TSF language profile and its
//! categories, and the signed-in user's input method list.
//!
//! regsvr32 of the 64-bit DLL writes the 64-bit registry view; the 32-bit regsvr32 (SysWOW64) registers the 32-bit
//! DLL in the WOW64 view, which 32-bit applications read.  Both register the same profile.

use std::path::PathBuf;

use windows::Win32::Foundation::{E_FAIL, ERROR_FILE_NOT_FOUND, FreeLibrary, HMODULE};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr, ITfInputProcessorProfileMgr,
    ITfInputProcessorProfiles,
};
use windows::core::{BOOL, GUID, HSTRING, PCWSTR, Result, s, w};

use crate::module::{LANGUAGE, NAME, PROFILE_ID, SERVICE_ID, module};

/// Keyboard text service; works in Store/immersive apps and in the system tray's input indicator; declares UI-less
/// support so applications that draw their own candidates still activate it (it then passes keys through, see
/// `TextService::restricted`).  No display attributes are provided.
const CATEGORIES: [GUID; 4] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
];

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
pub(crate) fn guid_string(guid: &GUID) -> String {
    let d = guid.data4;
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1, guid.data2, guid.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

/// The profile as Windows lists it: `0404:{CLSID}{PROFILE}`.
fn tip() -> String {
    format!("{LANGUAGE:04x}:{}{}", guid_string(&SERVICE_ID), guid_string(&PROFILE_ID))
}

fn class_key() -> String {
    format!("Software\\Classes\\CLSID\\{}", guid_string(&SERVICE_ID))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn module_path(module: HMODULE) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    // SAFETY: the buffer is writable and its length is passed.
    let length = unsafe { GetModuleFileNameW(Some(module), &mut buffer) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(E_FAIL.into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}

fn set_string(root: HKEY, path: &str, name: Option<&str>, value: &str) -> Result<()> {
    let mut key = HKEY::default();
    let path = HSTRING::from(path);
    // SAFETY: valid strings and out pointer.
    unsafe {
        RegCreateKeyExW(root, &path, None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None)
    }
    .ok()?;
    let data: Vec<u8> = value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    let name = name.map(HSTRING::from);
    let name = name.as_ref().map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()));
    // SAFETY: the key is open; data is a NUL-terminated UTF-16 string.
    let status = unsafe { RegSetValueExW(key, name, None, REG_SZ, Some(&data)) };
    // SAFETY: closes the key opened above.
    let _ = unsafe { RegCloseKey(key) };
    status.ok()
}

fn delete_tree(root: HKEY, path: &str) -> Result<()> {
    // SAFETY: a valid root and path.
    let status = unsafe { RegDeleteTreeW(root, &HSTRING::from(path)) };
    if status == ERROR_FILE_NOT_FOUND { Ok(()) } else { status.ok() }
}

pub(crate) fn register() -> Result<()> {
    let dll = module_path(module())?;
    // The language list shows logo.ico beside the DLL (installed with it); without it, the DLL's own icon.
    let icon = dll.with_file_name("logo.ico");
    let icon = if icon.is_file() { icon } else { dll.clone() };
    let key = class_key();
    set_string(HKEY_LOCAL_MACHINE, &key, None, NAME)?;
    set_string(HKEY_LOCAL_MACHINE, &format!("{key}\\InprocServer32"), None, &dll.display().to_string())?;
    set_string(HKEY_LOCAL_MACHINE, &format!("{key}\\InprocServer32"), Some("ThreadingModel"), "Apartment")?;
    // A per-user registration of the same class (an earlier developer install) would win over this one.
    let _ = delete_tree(HKEY_CURRENT_USER, &key);
    let profile = || -> Result<()> {
        // SAFETY: COM is initialised by regsvr32 (or the caller).
        let profiles: ITfInputProcessorProfileMgr =
            unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }?;
        let icon = wide(&icon.display().to_string());
        // SAFETY: valid GUIDs and strings.
        unsafe {
            profiles.RegisterProfile(
                &SERVICE_ID,
                LANGUAGE,
                &PROFILE_ID,
                &wide(NAME),
                &icon,
                0,
                HKL::default(),
                0,
                true,
                0,
            )
        }?;
        // SAFETY: as above.
        let categories: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }?;
        for category in &CATEGORIES {
            // SAFETY: valid GUIDs.
            unsafe { categories.RegisterCategory(&SERVICE_ID, category, &SERVICE_ID) }?;
        }
        Ok(())
    };
    profile().inspect_err(|_| {
        let _ = unregister();
    })
}

/// Removes the profile, the categories, the text service's TSF key and the COM class.  Only a failure to remove the
/// class is an error: the other steps may find their part gone already.
pub(crate) fn unregister() -> Result<()> {
    // SAFETY: COM is initialised by regsvr32 (or the caller); valid GUIDs.
    unsafe {
        if let Ok(categories) = CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        {
            for category in &CATEGORIES {
                let _ = categories.UnregisterCategory(&SERVICE_ID, category, &SERVICE_ID);
            }
        }
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.UnregisterProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID, 0);
        }
        // The calls above leave the empty TIP key behind; this removes it (in this DLL's registry view).
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.Unregister(&SERVICE_ID);
        }
    }
    let _ = delete_tree(HKEY_LOCAL_MACHINE, &format!("Software\\Microsoft\\CTF\\TIP\\{}", guid_string(&SERVICE_ID)));
    let key = class_key();
    let _ = delete_tree(HKEY_CURRENT_USER, &key);
    delete_tree(HKEY_LOCAL_MACHINE, &key)
}

const ILOT_UNINSTALL: u32 = 0x1;

/// `InstallLayoutOrTip` (input.dll): adds the profile to the current user's language list and enables it, or with
/// `ILOT_UNINSTALL` removes it.  Other input methods and the default input method are left as they are.
fn install_layout_or_tip(flags: u32) -> Result<()> {
    type InstallLayoutOrTip = unsafe extern "system" fn(PCWSTR, u32) -> BOOL;
    // SAFETY: loads a system DLL from System32 only.
    let library = unsafe { LoadLibraryExW(w!("input.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }?;
    // SAFETY: the library is loaded; the name is NUL-terminated.
    let result = match unsafe { GetProcAddress(library, s!("InstallLayoutOrTip")) } {
        Some(function) => {
            // SAFETY: the documented signature of InstallLayoutOrTip.
            let function: InstallLayoutOrTip = unsafe { std::mem::transmute(function) };
            let tip = HSTRING::from(tip());
            // SAFETY: a NUL-terminated profile string.
            if unsafe { function(PCWSTR(tip.as_ptr()), flags) }.as_bool() { Ok(()) } else { Err(E_FAIL.into()) }
        }
        None => Err(E_FAIL.into()),
    };
    // SAFETY: frees the library loaded above.
    let _ = unsafe { FreeLibrary(library) };
    result
}

pub(crate) fn enable_for_user() -> Result<()> {
    install_layout_or_tip(0)
}

pub(crate) fn disable_for_user() -> Result<()> {
    install_layout_or_tip(ILOT_UNINSTALL)
}
