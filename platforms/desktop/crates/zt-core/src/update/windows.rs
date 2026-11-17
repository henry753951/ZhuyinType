//! The Windows side of installing and updating: the demand-start service that installs updates (`ZhuyinTypeUpdate`,
//! created by the installer with a security descriptor that lets any signed-in user start it, and nothing else),
//! starting it from the user's app, starting the app again in the signed-in sessions after the installer replaced it,
//! and adding the input method to the list of a user the installer could not reach.
#![allow(unsafe_code)] // Service control, registry, session and process creation calls.

use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SUCCESS, GetLastError, HANDLE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, GetAce, OWNER_SECURITY_INFORMATION, PSID,
};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};
use windows_sys::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_SESSION_INFOW, WTSActive, WTSDisconnected, WTSEnumerateSessionsW, WTSFreeMemory,
    WTSQueryUserToken,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, RegisterServiceCtrlHandlerExW, SC_MANAGER_CONNECT,
    SERVICE_ACCEPT_STOP, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus, StartServiceCtrlDispatcherW, StartServiceW,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, PROCESS_INFORMATION, STARTUPINFOW,
};

use super::WINDOWS_SERVICE;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// The installer's key (`HKLM\Software\ZhuyinType`; only administrators can write it).
const PRODUCT_KEY: &str = r"Software\ZhuyinType";
/// The installer's requests to [`relaunch_sessions`], removed once read: the sessions to start the app again in, in
/// the background (`1,3`, or `all`: every signed-in session), and the session to open its settings in.
const RELAUNCH_VALUE: &str = "RelaunchSessions";
const OPEN_VALUE: &str = "OpenSession";
/// The service's start argument that runs [`relaunch_sessions`] instead of an update (`sc start ZhuyinTypeUpdate
/// relaunch`, installer.nsi).  It only carries out what the installer wrote under [`PRODUCT_KEY`].
const RELAUNCH_ARGUMENT: &str = "relaunch";

fn read_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (wide(key), wide(value));
    let mut size = 0u32;
    // SAFETY: valid strings; the first call only measures, the second writes at most `size` bytes into the buffer.
    unsafe {
        if RegGetValueW(root, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut size) != 0 {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2).max(1)];
        if RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        ) != 0
        {
            return None;
        }
        let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..length]))
    }
}

fn write_string(root: HKEY, key: &str, value: &str, data: &str) -> bool {
    let (key, value, data) = (wide(key), wide(value), wide(data));
    let bytes = u32::try_from(data.len() * 2).unwrap_or(u32::MAX);
    // SAFETY: valid strings; `data` is NUL-terminated and `bytes` long.
    unsafe { RegSetKeyValueW(root, key.as_ptr(), value.as_ptr(), REG_SZ, data.as_ptr().cast(), bytes) == 0 }
}

/// Reads and removes one of the installer's requests.
fn take_request(value: &str) -> Option<String> {
    let text = read_string(HKEY_LOCAL_MACHINE, PRODUCT_KEY, value)?;
    let (key, name) = (wide(PRODUCT_KEY), wide(value));
    // SAFETY: valid strings.
    unsafe { RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr()) };
    Some(text)
}

/// Which sessions to start the app in: `(session, open its settings)`, from the installer's requests.
#[derive(Debug, Default, PartialEq, Eq)]
struct Relaunch {
    all: bool,
    background: Vec<u32>,
    open: Option<u32>,
}

impl Relaunch {
    fn parse(background: Option<&str>, open: Option<&str>) -> Self {
        let background = background.unwrap_or_default().trim();
        Self {
            all: background.eq_ignore_ascii_case("all"),
            // Session 0 runs services, never a signed-in user's desktop.
            background: background.split(',').filter_map(|id| id.trim().parse().ok()).filter(|&id| id != 0).collect(),
            open: open.and_then(|id| id.trim().parse().ok()).filter(|&id| id != 0),
        }
    }

    /// The app's arguments in `session`, or None to leave it alone.
    fn arguments(&self, session: u32) -> Option<&'static str> {
        if self.open == Some(session) {
            Some("")
        } else if self.all || self.background.contains(&session) {
            Some(" --background")
        } else {
            None
        }
    }
}

/// Asks the service to run (from the signed-in user's app).  Already running counts as started.
pub fn start_service() -> Result<()> {
    let name = wide(WINDOWS_SERVICE);
    // SAFETY: valid strings; every handle opened is closed.
    unsafe {
        let manager = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            bail!("OpenSCManager: {}", GetLastError());
        }
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_START);
        if service.is_null() {
            let error = GetLastError();
            CloseServiceHandle(manager);
            bail!("找不到更新服務（{error}）；請重新安裝 ZhuyinType");
        }
        let started = StartServiceW(service, 0, null());
        let error = GetLastError();
        CloseServiceHandle(service);
        CloseServiceHandle(manager);
        if started == 0 && error != ERROR_SERVICE_ALREADY_RUNNING {
            bail!("StartService: {error}");
        }
    }
    Ok(())
}

static WORK: OnceLock<fn()> = OnceLock::new();
static STATUS: OnceLock<usize> = OnceLock::new();

fn report(state: u32) {
    let Some(&handle) = STATUS.get() else { return };
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING { SERVICE_ACCEPT_STOP } else { 0 },
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    // SAFETY: the handle from RegisterServiceCtrlHandlerExW.
    unsafe { SetServiceStatus(handle as SERVICE_STATUS_HANDLE, &status) };
}

/// Every control (stop, interrogate) is acknowledged; the work is short and ends on its own.
unsafe extern "system" fn control(_: u32, _: u32, _: *mut core::ffi::c_void, _: *mut core::ffi::c_void) -> u32 {
    0
}

/// The start arguments after the service's own name (`StartService`), as text.
///
/// # Safety
/// `argv` holds `argc` NUL-terminated UTF-16 strings, as the service control manager passes them.
unsafe fn arguments(argc: u32, argv: *mut *mut u16) -> Vec<String> {
    if argv.is_null() {
        return Vec::new();
    }
    // SAFETY: per the contract above.
    unsafe { std::slice::from_raw_parts(argv, argc as usize) }
        .iter()
        .skip(1)
        .filter(|arg| !arg.is_null())
        .map(|&arg| {
            // SAFETY: a NUL-terminated string.
            let length = (0..).take_while(|&i| unsafe { *arg.add(i) } != 0).count();
            // SAFETY: `length` units precede the NUL.
            String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(arg, length) })
        })
        .collect()
}

unsafe extern "system" fn service_main(argc: u32, argv: *mut *mut u16) {
    let name = wide(WINDOWS_SERVICE);
    // SAFETY: a valid name and handler.
    let handle = unsafe { RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), null_mut()) };
    if handle.is_null() {
        return;
    }
    let _ = STATUS.set(handle as usize);
    report(SERVICE_RUNNING);
    // SAFETY: the arguments the service control manager passed.
    if unsafe { arguments(argc, argv) }.first().is_some_and(|arg| arg == RELAUNCH_ARGUMENT) {
        if let Err(error) = relaunch_sessions() {
            log::error!("relaunch: {error:#}");
        }
    } else if let Some(work) = WORK.get() {
        work();
    }
    report(SERVICE_STOPPED);
}

/// Runs `work` as the service (`zhuyintype.exe --update-service`, started by the service control manager).  Started
/// with the argument `relaunch` (by the installer), it runs [`relaunch_sessions`] instead.
pub fn run_service(work: fn()) -> Result<()> {
    let _ = WORK.set(work);
    let mut name = wide(WINDOWS_SERVICE);
    let table = [
        SERVICE_TABLE_ENTRYW { lpServiceName: name.as_mut_ptr(), lpServiceProc: Some(service_main) },
        SERVICE_TABLE_ENTRYW { lpServiceName: null_mut(), lpServiceProc: None },
    ];
    // SAFETY: a null-terminated table that outlives the call.
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        // SAFETY: no preconditions.
        bail!("StartServiceCtrlDispatcher: {}", unsafe { GetLastError() });
    }
    Ok(())
}

/// Starts the app again after the installer replaced it, as the user signed in to each session and on their desktop:
/// in the background in the sessions the installer listed (where the app ran before it was stopped, or `all`), and
/// with its settings in the session whose installer offered to open it.  Run as SYSTEM (it needs each session's user
/// token): by the installer itself when the update service ran it (`--relaunch-sessions`), otherwise by the service
/// at the installer's request (the `relaunch` start argument), so that a standard user who typed an administrator's
/// password gets the app as themselves, not as that administrator.  Only acts on what the installer wrote under
/// `HKLM\Software\ZhuyinType`; anyone may start the service, so nothing else is taken from its caller.  Returns how
/// many it started.
pub fn relaunch_sessions() -> Result<usize> {
    let request = Relaunch::parse(take_request(RELAUNCH_VALUE).as_deref(), take_request(OPEN_VALUE).as_deref());
    let exe = std::env::current_exe()?;
    let directory = wide(&exe.parent().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_default());
    let application = wide(&exe.to_string_lossy());
    let mut started = 0;
    // SAFETY: the session list is freed below; every token, environment block and process handle is released.
    unsafe {
        let mut sessions: *mut WTS_SESSION_INFOW = null_mut();
        let mut count = 0u32;
        if WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut sessions, &mut count) == 0 {
            bail!("WTSEnumerateSessions: {}", GetLastError());
        }
        for session in std::slice::from_raw_parts(sessions, count as usize) {
            if session.State != WTSActive && session.State != WTSDisconnected {
                continue;
            }
            let Some(arguments) = request.arguments(session.SessionId) else { continue };
            let mut token: HANDLE = null_mut();
            if WTSQueryUserToken(session.SessionId, &mut token) == 0 {
                continue;
            }
            let mut environment: *mut core::ffi::c_void = null_mut();
            let have_environment = CreateEnvironmentBlock(&mut environment, token, 0) != 0;
            let mut desktop = wide("winsta0\\default");
            let startup = STARTUPINFOW {
                cb: size_of::<STARTUPINFOW>() as u32,
                lpDesktop: desktop.as_mut_ptr(),
                ..std::mem::zeroed()
            };
            let mut command = wide(&format!("\"{}\"{arguments}", exe.to_string_lossy()));
            let mut process: PROCESS_INFORMATION = std::mem::zeroed();
            let flags = CREATE_NO_WINDOW | if have_environment { CREATE_UNICODE_ENVIRONMENT } else { 0 };
            if CreateProcessAsUserW(
                token,
                application.as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                0,
                flags,
                if have_environment { environment } else { null() },
                directory.as_ptr(),
                &startup,
                &mut process,
            ) != 0
            {
                started += 1;
                CloseHandle(process.hThread);
                CloseHandle(process.hProcess);
            }
            if have_environment {
                DestroyEnvironmentBlock(environment);
            }
            CloseHandle(token);
        }
        WTSFreeMemory(sessions.cast());
    }
    Ok(started)
}

/// SYSTEM, Administrators and TrustedInstaller: the only accounts that may own or change what the service runs.
const TRUSTED: [&str; 3] =
    ["S-1-5-18", "S-1-5-32-544", "S-1-5-80-956008885-3425870976-2464367581-2108071245-1764063519"];
/// Rights that let a holder change a file or folder or what it contains: write and append data (add files), delete
/// children, delete, change the permissions or the owner, and the generic forms.
const WRITE_RIGHTS: u32 = 0x2 | 0x4 | 0x40 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x1000_0000 | 0x4000_0000;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const INHERIT_ONLY_ACE: u8 = 0x8;

fn sid_string(sid: PSID) -> Option<String> {
    let mut text: *mut u16 = null_mut();
    // SAFETY: a valid SID; the string is freed below.
    unsafe {
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return None;
        }
        let length = (0..).take_while(|&i| *text.add(i) != 0).count();
        let string = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        Some(string)
    }
}

/// Whether only SYSTEM, Administrators and TrustedInstaller own `path` and can change it: true for
/// `C:\Program Files\ZhuyinType`, false for a folder made under `C:\` that inherits the right of every signed-in
/// user to change it.
fn only_administrators_can_change(path: &Path) -> Result<bool> {
    let name = wide(&path.to_string_lossy());
    let mut owner: PSID = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: *mut core::ffi::c_void = null_mut();
    // SAFETY: a valid path; the descriptor is freed below and the owner and DACL point into it.
    unsafe {
        let status = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        );
        if status != ERROR_SUCCESS {
            bail!("GetNamedSecurityInfo {}: {status}", path.display());
        }
        let trusted = |sid: PSID| sid_string(sid).is_some_and(|sid| TRUSTED.contains(&sid.as_str()));
        // A missing DACL gives everyone every right.
        let mut protected = !dacl.is_null() && trusted(owner);
        if protected {
            for index in 0..u32::from((*dacl).AceCount) {
                let mut ace: *mut core::ffi::c_void = null_mut();
                if GetAce(dacl, index, &mut ace) == 0 {
                    protected = false;
                    break;
                }
                let header = &*ace.cast::<ACE_HEADER>();
                if header.AceType != ACCESS_ALLOWED_ACE_TYPE || header.AceFlags & INHERIT_ONLY_ACE != 0 {
                    continue;
                }
                let allowed = &*ace.cast::<ACCESS_ALLOWED_ACE>();
                let sid: PSID = std::ptr::addr_of!(allowed.SidStart).cast_mut().cast();
                if allowed.Mask & WRITE_RIGHTS != 0 && !trusted(sid) {
                    protected = false;
                    break;
                }
            }
        }
        LocalFree(descriptor);
        Ok(protected)
    }
}

/// The service runs `zhuyintype.exe` and the installers it downloads as SYSTEM, so it refuses to when anyone else
/// could have replaced them: when the installation folder or the executable can be changed by a non-administrator
/// (a folder chosen outside Program Files that kept the permissions of `C:\`; the installer locks it down since
/// 0.6.2, see installer.nsi).
pub fn ensure_installation_protected() -> Result<()> {
    let exe = std::env::current_exe()?;
    let directory = exe.parent().ok_or_else(|| anyhow::anyhow!("no install directory"))?;
    for path in [directory, exe.as_path()] {
        if !only_administrators_can_change(path)? {
            bail!("{} 可以由系統管理員以外的帳號修改，不從這裡安裝更新；請重新安裝 ZhuyinType", path.display());
        }
    }
    Ok(())
}

/// The installer's Active Setup entry (installer.nsi): its `Version` under HKCU marks a user whose input method list
/// was offered ZhuyinType once.
const ACTIVE_SETUP_KEY: &str =
    r"Software\Microsoft\Active Setup\Installed Components\{25BC3548-B611-4F92-91D0-C9291D427EA8}";

/// Adds ZhuyinType to the signed-in user's input method list once, as Active Setup does at sign-in
/// (`regsvr32 /n /i:user`, which runs the text service's `DllInstall`).  For the user the installer could not reach:
/// a standard user who installed by typing an administrator's password (the installer runs as that administrator and
/// adds it to the administrator's list), or a user already signed in when it was installed.  Someone who removed it
/// from the list keeps it removed: Active Setup's mark is set either way.  Run by the app as the signed-in user.
pub fn enable_input_method_for_user() -> Result<()> {
    let installed = read_string(HKEY_LOCAL_MACHINE, ACTIVE_SETUP_KEY, "Version");
    let Some(installed) = installed else { return Ok(()) }; // not installed by the installer
    if read_string(HKEY_CURRENT_USER, ACTIVE_SETUP_KEY, "Version").is_some() {
        return Ok(());
    }
    add_input_method_for_user()?;
    write_string(HKEY_CURRENT_USER, ACTIVE_SETUP_KEY, "Version", &installed);
    Ok(())
}

/// Adds ZhuyinType to the signed-in user's input method list now (`regsvr32 /n /i:user`, the text service's
/// `DllInstall`), whether or not it was offered before: 檢查輸入法's 「加入輸入法清單」.  Other input methods and the
/// default stay as they are.
pub fn add_input_method_for_user() -> Result<()> {
    let exe = std::env::current_exe()?;
    let dll = exe.with_file_name("zhuyintype-tsf-x64.dll");
    let system = std::env::var_os("SystemRoot").map_or_else(|| r"C:\Windows".into(), std::path::PathBuf::from);
    let status = std::process::Command::new(system.join(r"System32\regsvr32.exe"))
        .args(["/s", "/n", "/i:user"])
        .arg(&dll)
        .status()?;
    if !status.success() {
        bail!("regsvr32 /n /i:user {}: {status}", dll.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Relaunch;

    #[test]
    fn the_installers_request_names_the_sessions() {
        let request = Relaunch::parse(Some("1, 3,x,0"), Some("2"));
        assert_eq!(request.background, vec![1, 3]);
        assert_eq!(request.arguments(1), Some(" --background"));
        assert_eq!(request.arguments(2), Some(""));
        assert_eq!(request.arguments(4), None);
        assert_eq!(request.arguments(0), None);
        let all = Relaunch::parse(Some("all"), None);
        assert_eq!(all.arguments(7), Some(" --background"));
        assert_eq!(Relaunch::parse(None, None), Relaunch::default());
        assert_eq!(Relaunch::parse(None, None).arguments(1), None);
        assert_eq!(Relaunch::parse(None, Some("0")).open, None);
    }
}
