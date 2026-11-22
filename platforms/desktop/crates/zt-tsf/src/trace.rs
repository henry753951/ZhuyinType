//! A trace of the text service's decisions, for finding a fault on a test machine: built only with the `trace`
//! feature (never in a release), appended to %TEMP%\zt-tsf-trace.log with the process, thread and time.  It records
//! activation, focus, connections and how each key was handled, never the text typed.

#[cfg(feature = "trace")]
pub(crate) fn write(line: std::fmt::Arguments<'_>) {
    use std::io::Write as _;
    let path = std::env::temp_dir().join("zt-tsf-trace.log");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        // SAFETY: no preconditions.
        let thread = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        let _ = writeln!(file, "{} {}:{} {line}", now.as_millis(), std::process::id(), thread);
    }
}

/// `trace!("…", args)`: a line in the trace (feature `trace`), nothing otherwise.
macro_rules! trace {
    ($($arg:tt)*) => {{
        #[cfg(feature = "trace")]
        $crate::trace::write(format_args!($($arg)*));
    }};
}
pub(crate) use trace;
