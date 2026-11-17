//! ZhuyinType's Windows text service: the TSF text input processor Windows loads into every application that takes
//! text (zhuyintype-tsf-x64.dll, and zhuyintype-tsf-x86.dll for 32-bit applications).
//!
//! The DLL handles keys, focus, the TSF composition and commits; the engine, the settings and the candidate window
//! live in the ZhuyinType app, reached through the broker's named pipe (platforms/desktop/IPC.md).  Every COM entry
//! point catches panics, and nothing here blocks an application for longer than one broker transaction (200 ms).
//!
//! The protocol, key and coordinate rules are plain Rust and tested on any system; the rest is Windows only.

#![cfg_attr(not(windows), allow(dead_code))]

mod coordinates;
mod keys;
mod protocol;
mod quick_menu;

#[cfg(windows)]
mod broker;
#[cfg(windows)]
mod composition;
#[cfg(windows)]
mod display_attributes;
#[cfg(windows)]
mod keyboard;
#[cfg(windows)]
mod mode_button;
#[cfg(windows)]
mod module;
#[cfg(windows)]
mod panel_draw;
#[cfg(windows)]
mod panel_window;
#[cfg(windows)]
mod registration;
#[cfg(windows)]
mod text_service;

#[cfg(windows)]
pub use module::{DllCanUnloadNow, DllGetClassObject, DllInstall, DllRegisterServer, DllUnregisterServer};
