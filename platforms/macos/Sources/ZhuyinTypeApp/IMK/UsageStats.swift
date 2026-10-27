// 使用統計 (docs/product/DESIGN.md §2.8): counts and timings of this Mac's typing for the 分析 page.  Only numbers are
// kept: never text, readings, keys pressed or app identifiers.  Nothing is counted under secure input.
//
// `UsageStatsObserver` watches a `ControllerCore` the way the correction recorder does and turns each key, click and
// commit into a few counter increments; `UsageStatsStore` keeps the totals and the last 60 days in memory and writes
// them to stats.json off the main thread, a few seconds after they change.  Per-key latency goes into a fixed
// histogram, so measuring costs two clock reads and an array increment.
import Darwin
import Foundation
import ZhuyinCore

/// What was counted, for one day or in total.
struct UsageCounters: Codable, Equatable {
    /// Characters sent to apps, punctuation included.
    var characters = 0
    /// Compositions sent.
    var compositions = 0
    /// Keys the input method handled while composing.
    var keys = 0
    /// Picks from the candidate list, by list.
    var picksNormal = 0
    var picksCorrections = 0
    var picksSingle = 0
    var picksPunctuation = 0
    /// ⌫ or ⌥⌫ in a composition.
    var backspaces = 0
    /// Esc that cleared Zhuyin or the composition.
    var escapes = 0
    /// ⌫ on just-sent text within 30 seconds, once per sending.
    var deleteAfterCommit = 0
    /// ` turning the text into its Zhuyin, and ` or Esc right after it restoring it.
    var literalZhuyin = 0
    var undoLiteral = 0
    /// Compositions in which the shown sentence had a typing correction, and those sent with one.
    var correctedShown = 0
    var correctedCommitted = 0
    /// Custom phrases added while typing (⇧← ⇧→ and Return).
    var phrasesAdded = 0

    init() {}

    // Decoding tolerates fields added later.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func value(_ key: CodingKeys) -> Int { (try? c.decodeIfPresent(Int.self, forKey: key)) ?? 0 }
        characters = value(.characters); compositions = value(.compositions); keys = value(.keys)
        picksNormal = value(.picksNormal); picksCorrections = value(.picksCorrections); picksSingle = value(.picksSingle)
        picksPunctuation = value(.picksPunctuation); backspaces = value(.backspaces); escapes = value(.escapes)
        deleteAfterCommit = value(.deleteAfterCommit); literalZhuyin = value(.literalZhuyin)
        undoLiteral = value(.undoLiteral); correctedShown = value(.correctedShown)
        correctedCommitted = value(.correctedCommitted); phrasesAdded = value(.phrasesAdded)
    }

    var picks: Int { picksNormal + picksCorrections + picksSingle + picksPunctuation }
    /// Every correction the user made: picks, deletions, Esc and `.
    var corrections: Int { picks + backspaces + escapes + deleteAfterCommit + literalZhuyin }
    var isZero: Bool { self == UsageCounters() }

    static func + (a: UsageCounters, b: UsageCounters) -> UsageCounters {
        var r = a
        r.characters += b.characters; r.compositions += b.compositions; r.keys += b.keys
        r.picksNormal += b.picksNormal; r.picksCorrections += b.picksCorrections; r.picksSingle += b.picksSingle
        r.picksPunctuation += b.picksPunctuation; r.backspaces += b.backspaces; r.escapes += b.escapes
        r.deleteAfterCommit += b.deleteAfterCommit; r.literalZhuyin += b.literalZhuyin; r.undoLiteral += b.undoLiteral
        r.correctedShown += b.correctedShown; r.correctedCommitted += b.correctedCommitted
        r.phrasesAdded += b.phrasesAdded
        return r
    }
}

/// Per-key latency, from the key reaching the input method to the composition and candidate window being updated.
struct LatencyHistogram: Codable, Equatable {
    /// Upper bounds of the buckets, in milliseconds; the last bucket is everything slower.
    static let bounds: [Double] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 25, 30, 40, 50, 75, 100, 150, 250, 500, 1000]
    var counts = [Int](repeating: 0, count: LatencyHistogram.bounds.count + 1)
    var maximum: Double = 0
    var total: Int { counts.reduce(0, +) }

    mutating func add(milliseconds: Double) {
        let index = Self.bounds.firstIndex { milliseconds <= $0 } ?? Self.bounds.count
        if counts.count != Self.bounds.count + 1 { counts = [Int](repeating: 0, count: Self.bounds.count + 1) }
        counts[index] += 1
        maximum = max(maximum, milliseconds)
    }

    /// The `fraction` percentile, interpolated inside its bucket; nil with no keys.
    func percentile(_ fraction: Double) -> Double? {
        let total = self.total
        guard total > 0 else { return nil }
        let target = fraction * Double(total)
        var seen = 0.0
        for (index, count) in counts.enumerated() where count > 0 {
            if seen + Double(count) >= target {
                let low = index == 0 ? 0 : Self.bounds[index - 1]
                let high = index < Self.bounds.count ? Self.bounds[index] : max(maximum, low)
                return min(maximum, low + (high - low) * (target - seen) / Double(count))
            }
            seen += Double(count)
        }
        return maximum
    }
}

/// The contents of stats.json.
struct UsageData: Codable, Equatable {
    static let schemaVersion = 1
    var schema = UsageData.schemaVersion
    /// When counting started (or was last cleared).
    var since: Date
    var totals = UsageCounters()
    /// By local day, yyyy-MM-dd; at most `UsageStatsStore.keptDays`.
    var days: [String: UsageCounters] = [:]
    var latency = LatencyHistogram()
}

/// The counters in memory and in stats.json.  Used on the main thread; files are written on its own queue.
final class UsageStatsStore {
    static var shared = UsageStatsStore(directory: EngineHost.userDirectory)
    static let fileName = "stats.json"
    /// Days kept for the history.
    static let keptDays = 60
    /// Seconds between a change and writing the file.
    static let saveDelay: TimeInterval = 5

    /// Nil: in memory only (self-tests, previews).
    let url: URL?
    private(set) var data: UsageData
    var now: () -> Date = Date.init
    private let queue = DispatchQueue(label: "ZhuyinType.stats", qos: .utility)
    private var saveScheduled = false
    private let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()
    private var cachedDay: (start: Date, end: Date, key: String)?

    init(directory: URL?, data: UsageData? = nil) {
        url = directory?.appendingPathComponent(Self.fileName)
        if let data {
            self.data = data
        } else if let url, let stored = try? Data(contentsOf: url),
                  let decoded = try? Self.decoder.decode(UsageData.self, from: stored), decoded.schema == UsageData.schemaVersion {
            self.data = decoded
        } else {
            self.data = UsageData(since: Date())
        }
    }

    private static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }()

    static func encode(_ data: UsageData) -> Data? {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(data)
    }

    /// The local day `date` falls in.
    func dayKey(_ date: Date) -> String {
        if let cached = cachedDay, date >= cached.start, date < cached.end { return cached.key }
        let calendar = Calendar.current
        let start = calendar.startOfDay(for: date)
        let end = calendar.date(byAdding: .day, value: 1, to: start) ?? date.addingTimeInterval(86_400)
        let key = dayFormatter.string(from: date)
        cachedDay = (start, end, key)
        return key
    }

    func date(ofDay key: String) -> Date? { dayFormatter.date(from: key) }

    func add(_ delta: UsageCounters) {
        guard !delta.isZero else { return }
        data.totals = data.totals + delta
        let key = dayKey(now())
        data.days[key] = (data.days[key] ?? UsageCounters()) + delta
        if data.days.count > Self.keptDays {
            for old in data.days.keys.sorted().prefix(data.days.count - Self.keptDays) { data.days[old] = nil }
        }
        scheduleSave()
    }

    func addLatency(milliseconds: Double) {
        data.latency.add(milliseconds: milliseconds)
        scheduleSave()
    }

    /// Starts over: the counters, the history and the file.
    func clear() {
        data = UsageData(since: now())
        guard let url else { return }
        queue.async { try? FileManager.default.removeItem(at: url) }
    }

    /// Writes now and waits: before the process exits, and in self-tests.
    func flush() {
        let snapshot = data
        queue.sync { write(snapshot) }
    }

    private func scheduleSave() {
        guard url != nil, !saveScheduled else { return }
        saveScheduled = true
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.saveDelay) { [weak self] in
            guard let self else { return }
            self.saveScheduled = false
            let snapshot = self.data
            self.queue.async { self.write(snapshot) }
        }
    }

    private func write(_ snapshot: UsageData) {
        guard let url, let encoded = Self.encode(snapshot) else { return }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try encoded.write(to: url, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        } catch {
            EngineHost.logger.error("Stats write failed: \(error.localizedDescription, privacy: .public)")
        }
    }
}

/// Watches one `ControllerCore` and counts.  It never changes what the core does.
final class UsageStatsObserver {
    let store: UsageStatsStore
    /// Seconds since the system started, as key-down times.
    var clock: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    /// Seconds after sending in which ⌫ on the sent text counts (as the engine's own rule).
    static let deleteWindow: TimeInterval = 30

    private var lastCommit: TimeInterval = -.infinity
    private var deletedSinceCommit = false
    /// The text before the last `, while ` or Esc can still restore it.
    private var literalBefore: String?
    /// The shown sentence had a typing correction at some point of this composition.
    private var sawCorrected = false

    init(store: UsageStatsStore) {
        self.store = store
    }

    /// `corrected`: the sentence now shown has a typing correction.  `latency`: nanoseconds the key took.
    func observe(_ input: CorrectionRecorder.Input, before: Presentation, after: Presentation, commit: String?,
                 handled: Bool, options: ComposerOptions, secure: Bool, corrected: Bool, latency: UInt64?) {
        guard !secure else {
            literalBefore = nil
            sawCorrected = false
            lastCommit = -.infinity
            return
        }
        var count = UsageCounters()
        let composing = !before.isEmpty || !after.isEmpty
        if case .key = input, handled, composing {
            count.keys = 1
            if let latency { store.addLatency(milliseconds: Double(latency) / 1_000_000) }
        }
        if let list = before.candidates, after.candidates == nil, picked(input, options: options) {
            if list.tolerant == true { count.picksCorrections = 1 }
            else if list.singleCharacter { count.picksSingle = 1 }
            else if before.zhuyinHint == nil { count.picksPunctuation = 1 }
            else { count.picksNormal = 1 }
        }
        if case .key(let key, _) = input {
            if let original = literalBefore, handled, key == .char("`") || key == .escape, after.text == original {
                count.undoLiteral = 1
                literalBefore = nil
            } else if key == .char("`"), handled, !before.isEmpty, after.text != before.text {
                count.literalZhuyin = 1
                literalBefore = before.text
            } else {
                literalBefore = nil
                switch key {
                case .backspace, .optionBackspace:
                    if handled, !before.isEmpty, before.candidates == nil {
                        count.backspaces = 1
                    } else if !handled, before.isEmpty, !deletedSinceCommit,
                              clock() - lastCommit <= Self.deleteWindow {
                        count.deleteAfterCommit = 1
                        deletedSinceCommit = true
                    }
                case .escape:
                    if handled, !before.isEmpty, before.candidates == nil, after.text != before.text { count.escapes = 1 }
                default: break
                }
            }
        } else if case .click = input {
            literalBefore = nil
        }
        if let notice = after.notice, notice.hasPrefix("已加入自訂詞"), notice != before.notice { count.phrasesAdded = 1 }
        if corrected, !after.isEmpty { sawCorrected = true }
        if let commit, !commit.isEmpty {
            count.characters = commit.count
            if after.isEmpty {
                count.compositions = 1
                if sawCorrected { count.correctedShown = 1 }
                if corrected { count.correctedCommitted = 1 }
                sawCorrected = false
                lastCommit = clock()
                deletedSinceCommit = false
            }
        } else if after.isEmpty {
            sawCorrected = false
        }
        store.add(count)
    }

    private func picked(_ input: CorrectionRecorder.Input, options: ComposerOptions) -> Bool {
        switch input {
        case .click: return true
        case .key(.enter, _): return true
        case .key(.char(let character), _): return options.selectionKeys.contains(character)
        default: return false
        }
    }
}

// MARK: - The process itself

/// How much this process uses, from the system: CPU time, memory and, where macOS reports it, energy.
struct ProcessMetrics {
    /// User and system CPU time since the process started.
    var cpuSeconds: Double
    var footprintBytes: UInt64?
    var peakFootprintBytes: UInt64?
    /// macOS's estimate of the energy used since the process started; nil when it is not reported.
    var energyJoules: Double?
    var running: TimeInterval

    /// When the process started.
    static let launched = Date()

    static func sample() -> ProcessMetrics {
        var usage = rusage()
        getrusage(RUSAGE_SELF, &usage)
        func seconds(_ value: timeval) -> Double { Double(value.tv_sec) + Double(value.tv_usec) / 1_000_000 }
        var metrics = ProcessMetrics(cpuSeconds: seconds(usage.ru_utime) + seconds(usage.ru_stime),
                                     running: Date().timeIntervalSince(launched))
        var info = rusage_info_v6()
        let result = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(getpid(), RUSAGE_INFO_V6, $0) }
        }
        if result == 0 {
            metrics.footprintBytes = info.ri_phys_footprint
            metrics.peakFootprintBytes = info.ri_lifetime_max_phys_footprint
            if info.ri_energy_nj > 0 { metrics.energyJoules = Double(info.ri_energy_nj) / 1_000_000_000 }
        } else {
            var v4 = rusage_info_v4()
            let fallback = withUnsafeMutablePointer(to: &v4) { pointer in
                pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(getpid(), RUSAGE_INFO_V4, $0) }
            }
            if fallback == 0 {
                metrics.footprintBytes = v4.ri_phys_footprint
                metrics.peakFootprintBytes = v4.ri_lifetime_max_phys_footprint
            }
        }
        return metrics
    }
}
