//! This process's resource use, for `--timing` (port of `Usage.swift`): CPU time, energy and instructions (macOS;
//! the kernel's per-process estimate, no privileges needed) and memory.

/// A snapshot of the process's resource use.
pub(crate) struct Usage {
    /// User + system CPU time, seconds.
    cpu: f64,
    /// Energy, joules; `None` where the system does not report it.
    energy: Option<f64>,
    instructions: Option<u64>,
    /// Current memory footprint and the largest it has been, bytes.
    footprint: u64,
    peak_footprint: u64,
}

#[cfg(target_os = "macos")]
impl Usage {
    pub(crate) fn now() -> Usage {
        // `struct mach_timebase_info { uint32_t numer, denom; }`.
        #[allow(unsafe_code)]
        unsafe extern "C" {
            fn mach_timebase_info(info: *mut [u32; 2]) -> libc::c_int;
        }
        /// `rusage_info_v6` (sys/resource.h): a 16-byte UUID, then 64-bit fields; read by index.
        const RUSAGE_INFO_V6: libc::c_int = 6;
        const USER_TIME: usize = 2;
        const SYSTEM_TIME: usize = 3;
        const PHYS_FOOTPRINT: usize = 9;
        const LIFETIME_MAX_PHYS_FOOTPRINT: usize = 30;
        const INSTRUCTIONS: usize = 31;
        const ENERGY_NJ: usize = 42;
        let mut info = [0u64; 96];
        let mut usage = Usage { cpu: 0.0, energy: None, instructions: None, footprint: 0, peak_footprint: 0 };
        // SAFETY: `info` is 768 bytes, larger than `struct rusage_info_v6` (464 bytes), and suitably aligned; the
        // kernel writes at most that struct into it.  `proc_pid_rusage` takes the buffer as `rusage_info_t *`.
        #[allow(unsafe_code)]
        let status = unsafe {
            libc::proc_pid_rusage(libc::getpid(), RUSAGE_INFO_V6, info.as_mut_ptr().cast::<libc::rusage_info_t>())
        };
        if status != 0 {
            return usage;
        }
        // ri_user_time and ri_system_time are in Mach absolute time units.
        let mut timebase = [0u32; 2];
        // SAFETY: `timebase` has the layout of `mach_timebase_info` and is writable.
        #[allow(unsafe_code)]
        unsafe {
            mach_timebase_info(&raw mut timebase);
        }
        let ticks = (info[USER_TIME] + info[SYSTEM_TIME]) as f64;
        usage.cpu = ticks * f64::from(timebase[0]) / f64::from(timebase[1]) / 1e9;
        usage.energy = Some(info[ENERGY_NJ] as f64 / 1e9);
        usage.instructions = Some(info[INSTRUCTIONS]);
        usage.footprint = info[PHYS_FOOTPRINT];
        usage.peak_footprint = info[LIFETIME_MAX_PHYS_FOOTPRINT];
        usage
    }
}

#[cfg(not(target_os = "macos"))]
impl Usage {
    pub(crate) fn now() -> Usage {
        let mut usage = Usage { cpu: 0.0, energy: None, instructions: None, footprint: 0, peak_footprint: 0 };
        // SAFETY: `rusage` is plain old data; zeroed is a valid value, and getrusage writes one into it.
        #[allow(unsafe_code)]
        let resources = unsafe {
            let mut resources: libc::rusage = std::mem::zeroed();
            libc::getrusage(libc::RUSAGE_SELF, &raw mut resources);
            resources
        };
        usage.cpu = (resources.ru_utime.tv_sec + resources.ru_stime.tv_sec) as f64
            + (resources.ru_utime.tv_usec + resources.ru_stime.tv_usec) as f64 / 1e6;
        usage.peak_footprint = u64::try_from(resources.ru_maxrss).unwrap_or(0) * 1024;
        if let Ok(status) = std::fs::read_to_string("/proc/self/status")
            && let Some(line) = status.lines().find(|l| l.starts_with("VmRSS:"))
            && let Some(kilobytes) =
                line.split(' ').filter(|p| !p.is_empty()).nth(1).and_then(|k| k.parse::<u64>().ok())
        {
            usage.footprint = kilobytes * 1024;
        }
        usage
    }
}

/// The `--timing` summary for a run of keys: latency per key and what the run cost.
pub(crate) fn summary(key_times: &[f64], start: &Usage, end: &Usage, wall: f64) -> String {
    if key_times.is_empty() {
        return String::new();
    }
    let mut sorted = key_times.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        // Swift: Int((Double(count) * p).rounded(.up)); non-negative and at most the count.
        #[allow(clippy::cast_sign_loss)]
        let rank = (sorted.len() as f64 * p).ceil() as usize;
        sorted[(sorted.len() - 1).min(rank.saturating_sub(1))]
    };
    let keys = key_times.len() as f64;
    let cpu = end.cpu - start.cpu;
    let mut lines = vec![
        format!(
            "  [keys {}: p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms]",
            key_times.len(),
            percentile(0.5),
            percentile(0.95),
            sorted[sorted.len() - 1]
        ),
        format!(
            "  [cpu {:.0} ms total, {:.1} ms per key, {:.0}% of {:.2} s wall]",
            cpu * 1000.0,
            cpu * 1000.0 / keys,
            if wall > 0.0 { cpu / wall * 100.0 } else { 0.0 },
            wall
        ),
    ];
    if let (Some(a), Some(b)) = (start.energy, end.energy) {
        lines.push(format!(
            "  [energy {:.1} mJ total, {:.2} mJ per key, {:.2} W while computing]",
            (b - a) * 1000.0,
            (b - a) * 1000.0 / keys,
            if wall > 0.0 { (b - a) / wall } else { 0.0 }
        ));
    }
    if let (Some(a), Some(b)) = (start.instructions, end.instructions) {
        lines.push(format!("  [instructions {:.1} M per key]", b.saturating_sub(a) as f64 / keys / 1e6));
    }
    lines.push(format!(
        "  [memory {:.0} MB now, {:.0} MB peak]",
        end.footprint as f64 / 1_048_576.0,
        end.peak_footprint as f64 / 1_048_576.0
    ));
    lines.iter().fold(String::new(), |mut text, line| {
        text.push_str(line);
        text.push('\n');
        text
    })
}
