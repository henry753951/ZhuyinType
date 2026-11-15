//! The part of ZhuyinType for Linux and Windows that has no UI.  The GPUI app (`zhuyintype`) draws it; the native
//! input method adapters (Fcitx5, TSF) reach it through the broker (`docs`: platforms/desktop/IPC.md).

pub mod paths;
pub mod settings;
