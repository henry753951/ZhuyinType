//! The broker's named pipe `\\.\pipe\ZhuyinType-ime-v1-<SID>`: a security descriptor that lets only the current user
//! in, remote clients rejected, and the first instance created with `FILE_FLAG_FIRST_PIPE_INSTANCE` so no other
//! process can serve the name first.  A lock file in `%LOCALAPPDATA%\ZhuyinType` keeps a second instance out.
//!
//! The TSF text service in this process (the app's own text fields) talks to this pipe too; the broker's threads
//! never wait for the UI thread, so that cannot deadlock.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{FromRawHandle as _, RawHandle};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::{Handler, MAX_FRAME, ReserveError};

pub(super) type Stream = File;

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect()
}

/// A string the system allocated with `LocalAlloc`, as UTF-16 up to its NUL.
///
/// # Safety
/// `pointer` is NULL or a NUL-terminated UTF-16 string from `LocalAlloc`; it is freed here.
unsafe fn take_local_string(pointer: *mut u16) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0;
    // SAFETY: the string is NUL-terminated per the contract.
    while unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: `length` units are readable.
    let text = String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, length) }).ok();
    // SAFETY: allocated by LocalAlloc per the contract.
    unsafe { LocalFree(pointer.cast()) };
    text
}

/// The current user's SID in string form (`S-1-5-21-…`).
fn user_sid() -> Option<String> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: the pseudo-handle of this process is valid; `token` is a live out pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let mut size = 0u32;
    // SAFETY: a size query with no buffer.
    unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
    // u64 storage keeps TOKEN_USER's pointers aligned.
    let mut storage = vec![0u64; (size as usize).div_ceil(8).max(1)];
    // SAFETY: `storage` holds at least `size` bytes.
    let ok = unsafe { GetTokenInformation(token, TokenUser, storage.as_mut_ptr().cast(), size, &mut size) } != 0;
    // SAFETY: `token` was opened above.
    unsafe { CloseHandle(token) };
    if !ok {
        return None;
    }
    // SAFETY: on success the buffer starts with a TOKEN_USER whose SID points into the buffer.
    let sid = unsafe { (*storage.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `sid` is valid while `storage` lives; `text` is a live out pointer.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return None;
    }
    // SAFETY: ConvertSidToStringSidW allocates with LocalAlloc.
    unsafe { take_local_string(text) }
}

pub(super) fn default_endpoint() -> Option<String> {
    user_sid().map(|sid| format!(r"\\.\pipe\ZhuyinType-ime-v1-{sid}"))
}

pub(super) fn connect(endpoint: &str) -> std::io::Result<File> {
    if !endpoint.starts_with(r"\\.\pipe\") {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    OpenOptions::new().read(true).write(true).open(endpoint)
}

pub(super) fn clone_stream(stream: &File) -> std::io::Result<File> {
    stream.try_clone()
}

fn failed(message: &str) -> ReserveError {
    ReserveError::Failed(anyhow!(message.to_owned()))
}

/// FNV-1a, for a lock file name that is the same in every build.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

pub(super) struct Reservation {
    endpoint: String,
    lock: File,
}

impl Reservation {
    pub(super) fn new(endpoint: &str) -> std::result::Result<Self, ReserveError> {
        if !endpoint.starts_with(r"\\.\pipe\") {
            return Err(failed("無法取得使用者通訊端。"));
        }
        let directory: PathBuf = dirs::data_local_dir().ok_or_else(|| failed("無法建立服務鎖定目錄。"))?.join("ZhuyinType");
        std::fs::create_dir_all(&directory).map_err(|_| failed("無法建立服務鎖定目錄。"))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join(format!("{:016x}.lock", fnv(endpoint))))
            .map_err(|_| failed("無法建立服務鎖定檔。"))?;
        if lock.try_lock().is_err() {
            return Err(ReserveError::AlreadyRunning);
        }
        match connect(endpoint) {
            Ok(_) => return Err(ReserveError::AlreadyRunning),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                return Err(ReserveError::AlreadyRunning);
            }
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => {}
            Err(error) => return Err(ReserveError::Failed(error.into())),
        }
        Ok(Self { endpoint: endpoint.to_owned(), lock })
    }

    pub(super) fn listen(self, handler: Arc<dyn Handler>, stop: Arc<AtomicBool>) -> Result<Listener> {
        let sid = user_sid().ok_or_else(|| anyhow!("無法取得使用者識別碼。"))?;
        let descriptor = Descriptor::current_user(&sid)?;
        let name = wide(&self.endpoint);
        let first = descriptor.create(&name, true)?;
        std::thread::Builder::new().name("zt-broker".into()).spawn(move || {
            let mut pipe = Some(first);
            loop {
                let instance = match pipe.take() {
                    Some(instance) => instance,
                    None => match descriptor.create(&name, false) {
                        Ok(instance) => instance,
                        Err(error) => {
                            log::warn!("broker: {error}");
                            break;
                        }
                    },
                };
                // SAFETY: `instance` is a pipe handle this thread owns; a blocking connect.
                let connected = unsafe { ConnectNamedPipe(instance.0, std::ptr::null_mut()) } != 0
                    // SAFETY: reads the calling thread's last error.
                    || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if !connected {
                    continue;
                }
                let file = instance.into_file();
                let handler = handler.clone();
                let spawned = std::thread::Builder::new().name("zt-broker-connection".into()).spawn(move || {
                    let Ok(writer) = file.try_clone() else { return };
                    super::serve(&handler, file, writer);
                });
                if let Err(error) = spawned {
                    log::warn!("broker: {error}");
                }
            }
        })?;
        Ok(Listener { _lock: self.lock })
    }
}

pub(super) struct Listener {
    _lock: File,
}

/// A pipe instance not yet handed to a connection thread.
struct Pipe(HANDLE);

// SAFETY: a pipe handle may be used from any thread; each instance is used by one thread at a time.
unsafe impl Send for Pipe {}

impl Pipe {
    fn into_file(self) -> File {
        let handle = self.0;
        std::mem::forget(self);
        // SAFETY: the handle is a valid pipe handle owned here; the File now owns and closes it.
        unsafe { File::from_raw_handle(handle as RawHandle) }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and not used after this.
        unsafe { CloseHandle(self.0) };
    }
}

/// A security descriptor that grants the current user (and nobody else) full access.
struct Descriptor(*mut c_void);

// SAFETY: the descriptor is immutable once made; it is only read by CreateNamedPipeW.
unsafe impl Send for Descriptor {}

impl Descriptor {
    fn current_user(sid: &str) -> Result<Self> {
        // D:P — a protected DACL with one entry: generic all for the user's SID.
        let sddl = wide(&format!("D:P(A;;GA;;;{sid})"));
        let mut descriptor: *mut c_void = std::ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated; `descriptor` is a live out pointer.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        } != 0;
        if !ok {
            anyhow::bail!("無法建立輸入法通訊的存取權限。");
        }
        Ok(Self(descriptor))
    }

    fn create(&self, name: &[u16], first: bool) -> Result<Pipe> {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        };
        let open = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        let buffer = MAX_FRAME as u32;
        // SAFETY: `name` is NUL-terminated; `attributes` and its descriptor live through the call.
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                open,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                buffer,
                buffer,
                0,
                &attributes,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            anyhow::bail!("無法建立輸入法通訊管道：{}", std::io::Error::last_os_error());
        }
        Ok(Pipe(handle))
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW with LocalAlloc.
        unsafe { LocalFree(self.0) };
    }
}
