//! Per-system window behaviour that GPUI does not provide.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(windows)]
pub mod windows;
