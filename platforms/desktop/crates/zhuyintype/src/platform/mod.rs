//! Per-system window behaviour that GPUI does not provide.

pub mod backdrop;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(windows)]
pub mod windows;
