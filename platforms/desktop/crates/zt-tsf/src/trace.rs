//! A trace of the text service's decisions, for finding a fault on someone's computer: activation (and why it
//! failed), focus, connections, panics and how each key was handled, never the text typed.  Off unless the file
//! %LOCALAPPDATA%\ZhuyinType\tsf-trace exists (or the `trace` feature is built in, for test machines); then each
//! process appends to %LOCALAPPDATA%\ZhuyinType\tsf-trace.log, else to its temporary folder, with its executable's
//! name, process and thread.  AppContainer apps see a folder of their own, so they are traced only with the feature.
//!
//! Turning it on (PowerShell), reproducing, then turning it off:
//!
//!   New-Item "$env:LOCALAPPDATA\ZhuyinType\tsf-trace" -Force
//!   Remove-Item "$env:LOCALAPPDATA\ZhuyinType\tsf-trace"

use std::path::PathBuf;
use std::sync::OnceLock;

fn folder() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("ZhuyinType"))
}

/// Whether the trace is on in this process (read once: a process started before the file existed stays off).
pub(crate) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = cfg!(feature = "trace") || folder().is_some_and(|folder| folder.join("tsf-trace").exists());
        if on {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                write(format_args!("panic: {info}"));
                previous(info);
            }));
        }
        on
    })
}

pub(crate) fn write(line: std::fmt::Arguments<'_>) {
    use std::io::Write as _;
    if !enabled() {
        return;
    }
    let shared = folder().map(|folder| folder.join("tsf-trace.log"));
    let local = std::env::temp_dir().join("zt-tsf-trace.log");
    let open = |path: &PathBuf| std::fs::OpenOptions::new().create(true).append(true).open(path).ok();
    let Some(mut file) = shared.as_ref().and_then(open).or_else(|| open(&local)) else { return };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    // SAFETY: no preconditions.
    let thread = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    let app = crate::composition::app_name();
    let _ = writeln!(file, "{} {app} {}:{thread} {line}", now.as_millis(), std::process::id());
}

/// `trace!("…", args)`: a line in the trace when it is on ([`enabled`]).
macro_rules! trace {
    ($($arg:tt)*) => {{
        if $crate::trace::enabled() {
            $crate::trace::write(format_args!($($arg)*));
        }
    }};
}
pub(crate) use trace;
