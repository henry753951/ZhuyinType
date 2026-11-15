//! The broker (`IPC.md`): newline-delimited JSON between the native adapters (Fcitx5 addon, TSF text service) and
//! this process, over a per-user Unix socket (Linux) or named pipe (Windows).

/// The endpoint for this user: `ZT_IME_SOCKET` when set; Linux `$XDG_RUNTIME_DIR/zhuyintype/ime-v1` (or
/// `/tmp/zhuyintype-<uid>/ime-v1` when that is unset or not absolute); Windows `\\.\pipe\ZhuyinType-ime-v1-<SID>`.
#[must_use]
pub fn endpoint() -> Option<String> {
    todo!()
}
