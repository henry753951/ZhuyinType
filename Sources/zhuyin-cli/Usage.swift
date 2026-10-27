import Foundation
#if canImport(Darwin)
import Darwin
#else
import Glibc
#endif

/// This process's resource use, for --timing: CPU time, energy (macOS only; the kernel's per-process estimate, no
/// privileges needed) and memory.
struct Usage {
    /// User + system CPU time, seconds.
    var cpu = 0.0
    /// Energy, joules; nil where the system does not report it.
    var energy: Double?
    var instructions: UInt64?
    /// Current memory footprint and the largest it has been, bytes.
    var footprint: UInt64 = 0
    var peakFootprint: UInt64 = 0

    static func now() -> Usage {
        var usage = Usage()
        #if canImport(Darwin)
        var info = rusage_info_v6()
        let status = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(getpid(), RUSAGE_INFO_V6, $0) }
        }
        guard status == 0 else { return usage }
        // ri_user_time and ri_system_time are in Mach absolute time units.
        var timebase = mach_timebase_info_data_t()
        mach_timebase_info(&timebase)
        let ticks = Double(info.ri_user_time + info.ri_system_time)
        usage.cpu = ticks * Double(timebase.numer) / Double(timebase.denom) / 1e9
        usage.energy = Double(info.ri_energy_nj) / 1e9
        usage.instructions = info.ri_instructions
        usage.footprint = info.ri_phys_footprint
        usage.peakFootprint = info.ri_lifetime_max_phys_footprint
        #else
        var resources = rusage()
        getrusage(__rusage_who_t(RUSAGE_SELF.rawValue), &resources)
        usage.cpu = Double(resources.ru_utime.tv_sec + resources.ru_stime.tv_sec)
            + Double(resources.ru_utime.tv_usec + resources.ru_stime.tv_usec) / 1e6
        usage.peakFootprint = UInt64(resources.ru_maxrss) * 1024
        if let status = try? String(contentsOfFile: "/proc/self/status", encoding: .utf8),
           let line = status.split(separator: "\n").first(where: { $0.hasPrefix("VmRSS:") }),
           let kilobytes = UInt64(line.split(separator: " ").dropFirst().first ?? "") {
            usage.footprint = kilobytes * 1024
        }
        #endif
        return usage
    }
}

/// The --timing summary for a run of keys: latency per key and what the run cost.
func usageSummary(keyTimes: [Double], from start: Usage, to end: Usage, wall: Double) -> String {
    guard !keyTimes.isEmpty else { return "" }
    var lines: [String] = []
    let sorted = keyTimes.sorted()
    func percentile(_ p: Double) -> Double { sorted[min(sorted.count - 1, Int((Double(sorted.count) * p).rounded(.up)) - 1)] }
    let keys = Double(keyTimes.count)
    let cpu = end.cpu - start.cpu
    lines.append(String(format: "  [keys %d: p50 %.1f ms, p95 %.1f ms, max %.1f ms]", keyTimes.count, percentile(0.5), percentile(0.95),
                 sorted.last!))
    lines.append(String(format: "  [cpu %.0f ms total, %.1f ms per key, %.0f%% of %.2f s wall]", cpu * 1000, cpu * 1000 / keys,
                 wall > 0 ? cpu / wall * 100 : 0, wall))
    if let a = start.energy, let b = end.energy {
        lines.append(String(format: "  [energy %.1f mJ total, %.2f mJ per key, %.2f W while computing]", (b - a) * 1000,
                     (b - a) * 1000 / keys, wall > 0 ? (b - a) / wall : 0))
    }
    if let a = start.instructions, let b = end.instructions {
        lines.append(String(format: "  [instructions %.1f M per key]", Double(b - a) / keys / 1e6))
    }
    lines.append(String(format: "  [memory %.0f MB now, %.0f MB peak]", Double(end.footprint) / 1_048_576,
                        Double(end.peakFootprint) / 1_048_576))
    return lines.map { $0 + "\n" }.joined()
}
