//! Per-system window behaviour that GPUI does not provide.

pub mod backdrop;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(windows)]
pub mod windows;

/// Something is full screen (a game, a film, a presentation), or the system asks for quiet: no notification now.
pub fn full_screen() -> bool {
    #[cfg(windows)]
    return windows::notifications_unwelcome();
    #[cfg(target_os = "linux")]
    return linux::full_screen();
    #[cfg(not(any(windows, target_os = "linux")))]
    false
}
