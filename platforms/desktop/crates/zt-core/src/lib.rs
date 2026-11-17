//! The part of ZhuyinType for Linux and Windows that has no UI.  The GPUI app (`zhuyintype`) draws it; the native
//! input method adapters (Fcitx5, TSF) reach it through the broker (`docs`: platforms/desktop/IPC.md).

pub mod appearance;
pub mod broker;
pub mod controller;
pub mod corrections;
pub mod engine;
pub mod health;
pub mod keys;
pub mod models;
pub mod panel;
pub mod paths;
pub mod practice;
pub mod presentation;
pub mod service;
pub mod settings;
pub mod shortcuts;
pub mod stats;
pub mod update;
