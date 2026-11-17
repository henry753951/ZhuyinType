//! The broker's Unix socket: in a directory owned by this user with mode 0700, the socket itself 0600, and every
//! peer's uid checked (`SO_PEERCRED`) against ours.  A lock file beside the socket keeps a second instance out.

use std::fs::{DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt as _, FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};

use super::{Handler, ReserveError};

pub(super) type Stream = UnixStream;

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

pub(super) fn default_endpoint() -> Option<String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|path| path.is_absolute());
    let base = match runtime {
        Some(runtime) => runtime.join("zhuyintype"),
        None => PathBuf::from(format!("/tmp/zhuyintype-{}", uid())),
    };
    base.join("ime-v1").into_os_string().into_string().ok()
}

pub(super) fn connect(endpoint: &str) -> std::io::Result<UnixStream> {
    UnixStream::connect(endpoint)
}

pub(super) fn clone_stream(stream: &UnixStream) -> std::io::Result<UnixStream> {
    stream.try_clone()
}

fn failed(message: &str) -> ReserveError {
    ReserveError::Failed(anyhow!(message.to_owned()))
}

/// The directory must be this user's, mode 0700 (made so when missing).
fn prepare_directory(endpoint: &Path) -> std::result::Result<(), ReserveError> {
    let directory = endpoint.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if let Err(error) = DirBuilder::new().mode(0o700).create(directory)
        && error.kind() != std::io::ErrorKind::AlreadyExists
    {
        return Err(failed("無法建立輸入法通訊目錄。"));
    }
    let owned = std::fs::symlink_metadata(directory)
        .is_ok_and(|meta| meta.is_dir() && meta.uid() == uid() && meta.mode() & 0o077 == 0);
    if !owned {
        return Err(failed("輸入法通訊目錄必須由目前使用者擁有，且權限為 0700。"));
    }
    Ok(())
}

/// Something other than our own socket is at the endpoint.
fn occupied(endpoint: &Path) -> bool {
    std::fs::symlink_metadata(endpoint).is_ok_and(|meta| !meta.file_type().is_socket() || meta.uid() != uid())
}

pub(super) struct Reservation {
    path: PathBuf,
    lock: File,
}

impl Reservation {
    pub(super) fn new(endpoint: &str) -> std::result::Result<Self, ReserveError> {
        if endpoint.is_empty() {
            return Err(failed("無法取得使用者通訊端。"));
        }
        let path = PathBuf::from(endpoint);
        prepare_directory(&path)?;
        let mut lock_path = path.clone().into_os_string();
        lock_path.push(".lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|_| failed("無法建立服務鎖定檔。"))?;
        if lock.try_lock().is_err() {
            return Err(ReserveError::AlreadyRunning);
        }
        if occupied(&path) {
            return Err(failed("通訊端路徑已被其他檔案佔用。"));
        }
        match UnixStream::connect(&path) {
            Ok(_) => return Err(ReserveError::AlreadyRunning),
            Err(error)
                if matches!(error.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound) => {}
            Err(error) => return Err(ReserveError::Failed(error.into())),
        }
        Ok(Self { path, lock })
    }

    pub(super) fn listen(self, handler: Arc<dyn Handler>, stop: Arc<AtomicBool>) -> Result<Listener> {
        if occupied(&self.path) {
            anyhow::bail!("通訊端路徑已被其他檔案佔用。");
        }
        // A socket left by an instance that ended without cleaning up (we hold the lock, so it is not serving).
        let _ = std::fs::remove_file(&self.path);
        let listener = UnixListener::bind(&self.path)?;
        std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
        let me = uid();
        std::thread::Builder::new().name("zt-broker".into()).spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else {
                    // Out of file descriptors, say: not a busy loop while it lasts.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                };
                // Only this user's processes may type through us.
                let peer = rustix::net::sockopt::socket_peercred(&stream).map(|cred| cred.uid.as_raw());
                if peer.ok() != Some(me) {
                    log::warn!("broker: refused a connection from another user");
                    continue;
                }
                let handler = handler.clone();
                let spawned = std::thread::Builder::new().name("zt-broker-connection".into()).spawn(move || {
                    let Ok(writer) = stream.try_clone() else { return };
                    super::serve(&handler, stream, writer);
                });
                if let Err(error) = spawned {
                    log::warn!("broker: {error}");
                }
            }
        })?;
        Ok(Listener { path: self.path, _lock: self.lock })
    }
}

pub(super) struct Listener {
    path: PathBuf,
    _lock: File,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
