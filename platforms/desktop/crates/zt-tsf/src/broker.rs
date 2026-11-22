//! The broker connection: one named pipe per text service, one request at a time, bounded waits.
//!
//! - A transaction (write the request, read the reply) has 200 ms.  On timeout the I/O is cancelled, the pipe closed
//!   and the key given back to the application; a request is never sent again, because its reply may have carried a
//!   commit.
//! - Connecting waits at most 100 ms in all for a busy pipe (every instance taken: many applications connecting at
//!   once, as at sign-in), trying again whenever one frees; after a failure the next attempt is at least 250 ms
//!   later, so a missing app costs a field nothing.
//! - The broker never writes unasked, so nothing is read between requests.  Before a key goes out the idle pipe is
//!   checked (`check`): if the app ended or restarted, the text service reconnects at once instead of sending the
//!   key into a broken pipe and giving it back to the application as a plain letter.
//! - The app's broker runs on its own thread, never on the GUI thread, so even the app's own text fields (whose TSF
//!   calls arrive on the GUI thread) wait without pumping messages.

use crate::trace::trace;
use std::cell::Cell;

use serde_json::{Map, Value};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
    LocalFree, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING, ReadFile, SECURITY_IDENTIFICATION,
    SECURITY_SQOS_PRESENT, WriteFile,
};
use windows::Win32::System::Environment::GetEnvironmentVariableW;
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Pipes::{PeekNamedPipe, WaitNamedPipeW};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{CreateEventW, GetCurrentProcess, OpenProcessToken, WaitForSingleObject};
use windows::core::{HSTRING, PWSTR, w};

use crate::protocol::{self, Frame, Reply};

const TRANSACTION_MS: u64 = 200;
const BUSY_WAIT_MS: u64 = 100;
const RETRY_MS: u64 = 250;

fn now() -> u64 {
    // SAFETY: no preconditions.
    unsafe { GetTickCount64() }
}

/// `\\.\pipe\ZhuyinType-ime-v1-<user SID>`, or `ZT_IME_SOCKET` (tests).
pub(crate) fn endpoint() -> Option<String> {
    let mut buffer = vec![0u16; 32768];
    // SAFETY: the buffer is writable.
    let length = unsafe { GetEnvironmentVariableW(w!("ZT_IME_SOCKET"), Some(&mut buffer)) } as usize;
    if length > 0 && length < buffer.len() {
        return Some(String::from_utf16_lossy(&buffer[..length]));
    }
    Some(format!("\\\\.\\pipe\\ZhuyinType-ime-v1-{}", user_sid()?))
}

fn user_sid() -> Option<String> {
    let mut token = HANDLE::default();
    // SAFETY: the current process's pseudo handle; token is closed below.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.ok()?;
    let mut bytes = 0u32;
    // SAFETY: a size query.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut bytes) };
    // u64 storage keeps TOKEN_USER aligned.
    let mut storage = vec![0u64; (bytes as usize).div_ceil(8).max(1)];
    // SAFETY: the buffer holds `bytes` bytes.
    let ok = unsafe { GetTokenInformation(token, TokenUser, Some(storage.as_mut_ptr().cast()), bytes, &mut bytes) };
    // SAFETY: opened above.
    let _ = unsafe { CloseHandle(token) };
    ok.ok()?;
    // SAFETY: GetTokenInformation filled a TOKEN_USER.
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = PWSTR::null();
    // SAFETY: a valid SID; the string is freed with LocalFree.
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }.ok()?;
    // SAFETY: a NUL-terminated string from the system.
    let sid = unsafe { text.to_string() }.ok();
    // SAFETY: allocated by ConvertSidToStringSidW.
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    sid
}

/// One outstanding request at a time.  Every method takes `&self`: the text service's COM methods do.
pub(crate) struct BrokerClient {
    pipe: Cell<HANDLE>,
    retry_at: Cell<u64>,
    serial: Cell<u64>,
    busy: Cell<bool>,
}

impl Drop for BrokerClient {
    fn drop(&mut self) {
        self.disconnect(false);
    }
}

impl BrokerClient {
    pub(crate) fn new() -> Self {
        Self {
            pipe: Cell::new(INVALID_HANDLE_VALUE),
            retry_at: Cell::new(0),
            serial: Cell::new(0),
            busy: Cell::new(false),
        }
    }

    pub(crate) fn connected(&self) -> bool {
        self.pipe.get() != INVALID_HANDLE_VALUE
    }

    /// A request is in flight (the caller re-entered during its wait).
    pub(crate) fn busy(&self) -> bool {
        self.busy.get()
    }

    pub(crate) fn connect(&self) -> bool {
        if self.connected() {
            return true;
        }
        if now() < self.retry_at.get() {
            trace!("connect: waiting to retry");
            return false;
        }
        // Input stays on this computer: only local pipes.
        if let Some(path) = endpoint().filter(|path| path.starts_with("\\\\.\\pipe\\")) {
            let path = HSTRING::from(path);
            let open = || {
                // SAFETY: a valid path; the handle is owned by self.pipe.
                unsafe {
                    CreateFileW(
                        &path,
                        (GENERIC_READ | GENERIC_WRITE).0,
                        FILE_SHARE_NONE,
                        None,
                        OPEN_EXISTING,
                        FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                        None,
                    )
                }
            };
            // A free instance may be taken by another client between the wait and the open: wait again, until the
            // deadline.
            let deadline = now() + BUSY_WAIT_MS;
            let pipe = loop {
                match open() {
                    Ok(pipe) => break Some(pipe),
                    Err(error) if error.code() == ERROR_PIPE_BUSY.to_hresult() => {
                        let remaining = u32::try_from(deadline.saturating_sub(now())).unwrap_or(0);
                        // SAFETY: a valid path.
                        if remaining == 0 || !unsafe { WaitNamedPipeW(&path, remaining) }.as_bool() {
                            break None;
                        }
                    }
                    Err(_) => break None,
                }
            };
            if let Some(pipe) = pipe.filter(|pipe| !pipe.is_invalid()) {
                self.pipe.set(pipe);
                trace!("connect: ok");
                return true;
            }
            trace!("connect: the pipe did not open");
        } else {
            trace!("connect: no endpoint");
        }
        self.retry_at.set(now() + RETRY_MS);
        false
    }

    /// Closes an idle pipe whose other end went away (the app ended or restarted), so the next `connect` opens a
    /// new one at once.  Unasked bytes break the protocol and close it too.
    pub(crate) fn check(&self) {
        if !self.connected() || self.busy.get() {
            return;
        }
        let mut available = 0u32;
        // SAFETY: the pipe opened by connect; only the count of waiting bytes is asked for.
        let alive = unsafe { PeekNamedPipe(self.pipe.get(), None, 0, None, Some(&mut available), None) }.is_ok();
        if !alive || available > 0 {
            self.disconnect(false);
        }
    }

    /// Closes the pipe; after a failure, the next connection waits the retry interval.
    pub(crate) fn disconnect(&self, failed: bool) {
        let pipe = self.pipe.replace(INVALID_HANDLE_VALUE);
        if pipe != INVALID_HANDLE_VALUE {
            // SAFETY: the handle was opened by connect.
            let _ = unsafe { CloseHandle(pipe) };
        }
        self.retry_at.set(if failed { now() + RETRY_MS } else { 0 });
    }

    /// Sends `op` for `context` and returns the checked reply; `None` (and a closed pipe) on any failure.
    pub(crate) fn request(&self, context: &str, op: &str, fields: Map<String, Value>) -> Option<Reply> {
        if !self.connected() || self.busy.get() {
            return None;
        }
        // Cleared on the way out even if the transaction panics (the COM method's guard catches it): a `busy` left
        // set would hand every later key to the application and keep any window on screen, the picker's included.
        let busy = Busy::set(&self.busy);
        let reply = self.transaction(context, op, fields);
        drop(busy);
        trace!("request {op}: {}", if reply.is_some() { "ok" } else { "failed" });
        if reply.is_none() {
            self.disconnect(true);
        }
        reply
    }

    fn transaction(&self, context: &str, op: &str, fields: Map<String, Value>) -> Option<Reply> {
        let id = self.serial.get() + 1;
        if id >= 1 << 53 {
            return None;
        }
        self.serial.set(id);
        let outgoing = protocol::request_line(id, context, op, fields)?;
        let deadline = now() + TRANSACTION_MS;
        let mut offset = 0;
        while offset < outgoing.len() {
            offset += self.transfer(Io::Write(&outgoing[offset..]), deadline)?;
        }
        let mut incoming = Vec::new();
        let mut block = [0u8; 4096];
        loop {
            let read = self.transfer(Io::Read(&mut block), deadline)?;
            incoming.extend_from_slice(&block[..read]);
            match protocol::frame(&incoming) {
                Frame::Complete => break,
                Frame::Invalid => return None,
                Frame::Incomplete => {}
            }
        }
        protocol::parse_reply(&incoming, id, context)
    }

    /// One overlapped read or write, waited for until `deadline`; the byte count, or `None` (cancelled, failed or
    /// nothing transferred).
    fn transfer(&self, io: Io<'_>, deadline: u64) -> Option<usize> {
        if now() >= deadline {
            return None;
        }
        let pipe = self.pipe.get();
        // SAFETY: an unnamed manual-reset event, closed below.
        let event = unsafe { CreateEventW(None, true, false, None) }.ok()?;
        let mut overlapped = OVERLAPPED { hEvent: event, ..Default::default() };
        let mut done = 0u32;
        // SAFETY: the buffer and `overlapped` outlive the I/O: it is waited for (or cancelled and waited for) below.
        let started = unsafe {
            match io {
                Io::Write(bytes) => WriteFile(pipe, Some(bytes), None, Some(&mut overlapped)),
                Io::Read(bytes) => ReadFile(pipe, Some(bytes), None, Some(&mut overlapped)),
            }
        };
        let finished = match started {
            // SAFETY: the I/O has completed; the count comes from its OVERLAPPED.
            Ok(()) => unsafe { GetOverlappedResult(pipe, &overlapped, &mut done, false) }.is_ok(),
            Err(error) if error.code() == ERROR_IO_PENDING.to_hresult() => {
                let remaining = u32::try_from(deadline.saturating_sub(now())).unwrap_or(u32::MAX);
                // SAFETY: the event is valid; the I/O is ours.
                unsafe {
                    if WaitForSingleObject(event, remaining) == WAIT_OBJECT_0 {
                        GetOverlappedResult(pipe, &overlapped, &mut done, false).is_ok()
                    } else {
                        let _ = CancelIoEx(pipe, Some(&overlapped));
                        // Keep the buffer and OVERLAPPED alive until the cancellation has completed.
                        let _ = GetOverlappedResult(pipe, &overlapped, &mut done, true);
                        false
                    }
                }
            }
            Err(_) => false,
        };
        // SAFETY: created above.
        let _ = unsafe { CloseHandle(event) };
        (finished && done > 0).then_some(done as usize)
    }
}

/// `BrokerClient::busy` while a request is in flight; cleared when dropped, unwinding included.
struct Busy<'a>(&'a Cell<bool>);

impl<'a> Busy<'a> {
    fn set(flag: &'a Cell<bool>) -> Self {
        flag.set(true);
        Self(flag)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

enum Io<'a> {
    Write(&'a [u8]),
    Read(&'a mut [u8]),
}
