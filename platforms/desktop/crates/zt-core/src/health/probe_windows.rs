//! Windows: reads the facts `windows::checks` judges, as the signed-in user and without administrator rights: the
//! registry (the text service's COM class in both views, this user's input method list, the installer's entries,
//! Smart App Control), the service manager (the update service), `SystemParametersInfo` (per-app input) and
//! `wevtutil` (Code Integrity's log, when this user may read it).  Off the UI thread; reads only.
#![allow(unsafe_code)] // Registry, service manager and SystemParametersInfo calls.

use std::os::windows::process::CommandExt as _;
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_MULTI_SZ,
    RRF_RT_REG_SZ, RRF_SUBKEY_WOW6432KEY, RRF_SUBKEY_WOW6464KEY, RegCloseKey, RegEnumValueW, RegGetValueW,
    RegOpenKeyExW,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETTHREADLOCALINPUTSETTINGS, SystemParametersInfoW};

use super::report::System;
use super::windows::{SmartAppControl, WindowsFacts, ZHUYINTYPE};
use crate::update::WINDOWS_SERVICE;

const CLASS_KEY: &str = r"Software\Classes\CLSID\{25BC3548-B611-4F92-91D0-C9291D427EA8}\InprocServer32";
const USER_PROFILE: &str = r"Control Panel\International\User Profile";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const ACTIVE_SETUP_KEY: &str =
    r"Software\Microsoft\Active Setup\Installed Components\{25BC3548-B611-4F92-91D0-C9291D427EA8}";
const CI_POLICY: &str = r"SYSTEM\CurrentControlSet\Control\CI\Policy";
const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// A value's raw UTF-16 data (`flags` picks the type and the registry view).
fn read_wide(root: HKEY, key: &str, value: Option<&str>, flags: u32) -> Option<Vec<u16>> {
    let key = wide(key);
    let value = value.map(wide);
    let value_ptr = value.as_ref().map_or(null(), |value| value.as_ptr());
    let mut size = 0u32;
    // SAFETY: valid strings; the first call only measures, the second writes at most `size` bytes into the buffer.
    unsafe {
        if RegGetValueW(root, key.as_ptr(), value_ptr, flags, null_mut(), null_mut(), &mut size) != ERROR_SUCCESS {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2).max(1)];
        if RegGetValueW(root, key.as_ptr(), value_ptr, flags, null_mut(), buffer.as_mut_ptr().cast(), &mut size)
            != ERROR_SUCCESS
        {
            return None;
        }
        buffer.truncate((size as usize) / 2);
        Some(buffer)
    }
}

fn read_string(root: HKEY, key: &str, value: Option<&str>, view: u32) -> Option<String> {
    let data = read_wide(root, key, value, RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | view)?;
    let end = data.iter().position(|&unit| unit == 0).unwrap_or(data.len());
    Some(String::from_utf16_lossy(&data[..end]))
}

fn read_multi(root: HKEY, key: &str, value: &str) -> Vec<String> {
    let Some(data) = read_wide(root, key, Some(value), RRF_RT_REG_MULTI_SZ) else { return Vec::new() };
    data.split(|&unit| unit == 0).filter(|part| !part.is_empty()).map(String::from_utf16_lossy).collect()
}

fn read_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (wide(key), wide(value));
    let mut data = 0u32;
    let mut size = 4u32;
    // SAFETY: valid strings; a 4-byte buffer for a REG_DWORD.
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

/// The names and DWORD data of every value of `key` (the input methods of one language: `0404:{…}{…}` = order).
fn dword_values(root: HKEY, key: &str) -> Option<Vec<(String, u32)>> {
    let key = wide(key);
    let mut handle: HKEY = null_mut();
    // SAFETY: valid string; the handle is closed below.
    if unsafe { RegOpenKeyExW(root, key.as_ptr(), 0, KEY_READ, &mut handle) } != ERROR_SUCCESS {
        return None;
    }
    let mut values = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut name_len = name.len() as u32;
        let mut data = 0u32;
        let mut data_len = 4u32;
        let mut kind = 0u32;
        // SAFETY: an open key; buffers of the sizes given.
        let status = unsafe {
            RegEnumValueW(
                handle,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                null(),
                &mut kind,
                (&raw mut data).cast(),
                &mut data_len,
            )
        };
        if status != ERROR_SUCCESS {
            break; // ERROR_NO_MORE_ITEMS, or a value too long to be an input method's
        }
        if kind == windows_sys::Win32::System::Registry::REG_DWORD {
            values.push((String::from_utf16_lossy(&name[..name_len as usize]), data));
        }
    }
    // SAFETY: opened above.
    unsafe { RegCloseKey(handle) };
    Some(values)
}

/// This user's input methods, language by language in the list's order, each language's in its own order.
fn user_list() -> Option<Vec<String>> {
    let languages = read_multi(HKEY_CURRENT_USER, USER_PROFILE, "Languages");
    if languages.is_empty() {
        return None;
    }
    let mut list = Vec::new();
    for language in languages {
        let Some(mut values) = dword_values(HKEY_CURRENT_USER, &format!(r"{USER_PROFILE}\{language}")) else {
            continue;
        };
        values.retain(|(name, _)| name.contains(':'));
        values.sort_by_key(|&(_, order)| order);
        list.extend(values.into_iter().map(|(name, _)| name));
    }
    Some(list)
}

/// The text service's COM class in one registry view, with its DLL on disk.
fn registered(view: u32) -> bool {
    read_string(HKEY_LOCAL_MACHINE, CLASS_KEY, None, view).is_some_and(|path| PathBuf::from(path).is_file())
}

fn service_exists() -> bool {
    let name = wide(WINDOWS_SERVICE);
    // SAFETY: valid strings; every handle opened is closed.
    unsafe {
        let manager = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return false;
        }
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_QUERY_STATUS);
        let exists = !service.is_null();
        if exists {
            CloseServiceHandle(service);
        }
        CloseServiceHandle(manager);
        exists
    }
}

fn per_app_input() -> Option<bool> {
    let mut value = 0i32;
    // SAFETY: SPI_GETTHREADLOCALINPUTSETTINGS writes a BOOL.
    let ok = unsafe { SystemParametersInfoW(SPI_GETTHREADLOCALINPUTSETTINGS, 0, (&raw mut value).cast(), 0) };
    (ok != 0).then_some(value != 0)
}

/// Our files in Code Integrity's recent block events (3033: did not meet the signing level, 3077: blocked by policy,
/// 3089: the signature details of one), or None when this user may not read the log.
fn blocked_files() -> Option<Vec<String>> {
    let system = std::env::var_os("SystemRoot").map_or_else(|| r"C:\Windows".into(), PathBuf::from);
    let output = std::process::Command::new(system.join(r"System32\wevtutil.exe"))
        .args([
            "qe",
            "Microsoft-Windows-CodeIntegrity/Operational",
            "/q:*[System[(EventID=3033 or EventID=3077)]]",
            "/c:50",
            "/rd:true",
            "/f:text",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(super::windows::blocked_files_in(&String::from_utf8_lossy(&output.stdout)))
}

/// The facts.
#[must_use]
pub fn probe() -> WindowsFacts {
    let user_list = user_list();
    let in_user_list = user_list.as_ref().map(|list| list.iter().any(|entry| entry.eq_ignore_ascii_case(ZHUYINTYPE)));
    let run = |root, view| read_string(root, RUN_KEY, Some("ZhuyinType"), view).is_some();
    let installed = [RRF_SUBKEY_WOW6464KEY, RRF_SUBKEY_WOW6432KEY]
        .into_iter()
        .any(|view| read_string(HKEY_LOCAL_MACHINE, ACTIVE_SETUP_KEY, Some("Version"), view).is_some());
    WindowsFacts {
        registered_64: registered(RRF_SUBKEY_WOW6464KEY),
        registered_32: registered(RRF_SUBKEY_WOW6432KEY),
        in_user_list,
        user_list: user_list.unwrap_or_default(),
        default_override: read_string(HKEY_CURRENT_USER, USER_PROFILE, Some("InputMethodOverride"), 0)
            .filter(|value| !value.is_empty()),
        per_app_input: per_app_input(),
        autostart: run(HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6464KEY)
            || run(HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6432KEY)
            || run(HKEY_CURRENT_USER, 0),
        update_service: service_exists(),
        installed,
        smart_app_control: match read_dword(HKEY_LOCAL_MACHINE, CI_POLICY, "VerifiedAndReputablePolicyState") {
            Some(1) => SmartAppControl::On,
            Some(2) => SmartAppControl::Evaluation,
            _ => SmartAppControl::Off,
        },
        blocked_files: blocked_files(),
    }
}

/// 「Windows 11 24H2（組建 26100）」.
#[must_use]
pub fn system() -> System {
    let value = |name| read_string(HKEY_LOCAL_MACHINE, CURRENT_VERSION, Some(name), RRF_SUBKEY_WOW6464KEY);
    let build = value("CurrentBuild").unwrap_or_default();
    let name = if build.parse::<u32>().is_ok_and(|build| build >= 22000) { "Windows 11" } else { "Windows 10" };
    let edition = value("EditionID").unwrap_or_default();
    let release = value("DisplayVersion").unwrap_or_default();
    let os =
        [name, edition.as_str(), release.as_str()].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>();
    System {
        os: format!("{}（組建 {build}）", os.join(" ")),
        desktop: String::new(),
        arch: std::env::consts::ARCH.to_owned(),
    }
}
