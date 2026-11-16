//! The Windows side of updates: the demand-start service that installs them (`ZhuyinTypeUpdate`, created by the
//! installer with a security descriptor that lets any signed-in user start it, and nothing else), starting it from
//! the user's app, and starting the app again in every signed-in session after the installer replaced it.
#![allow(unsafe_code)] // Service control, session and process creation calls.

use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SERVICE_ALREADY_RUNNING, GetLastError, HANDLE};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
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

unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let name = wide(WINDOWS_SERVICE);
    // SAFETY: a valid name and handler.
    let handle = unsafe { RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), null_mut()) };
    if handle.is_null() {
        return;
    }
    let _ = STATUS.set(handle as usize);
    report(SERVICE_RUNNING);
    if let Some(work) = WORK.get() {
        work();
    }
    report(SERVICE_STOPPED);
}

/// Runs `work` as the service (`zhuyintype.exe --update-service`, started by the service control manager).
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

/// Starts `<exe> --background` in every session a user is signed in to (as that user, on their desktop).  Run as
/// SYSTEM by the installer after an update (`--relaunch-sessions`).  Returns how many it started.
pub fn relaunch_sessions() -> Result<usize> {
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
            let mut command = wide(&format!("\"{}\" --background", exe.to_string_lossy()));
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
