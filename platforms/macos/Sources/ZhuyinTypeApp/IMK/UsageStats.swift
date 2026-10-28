// 使用統計 (docs/product/DESIGN.md §2.8): counts and timings of this Mac's typing for the 分析 page.  Only numbers are
// kept: never text, readings, keys pressed or app identifiers.  Nothing is counted under secure input.
//
// `UsageStatsObserver` watches a `ControllerCore` the way the correction recorder does and turns each key, click and
// commit into a few counter increments; `UsageStatsStore` keeps the totals and the last 60 days in memory and writes
// them to stats.json off the main thread at most every 10 minutes (and when the input method is switched away, the
// settings window opens or the process exits).  Per-key latency goes into a fixed histogram, so measuring costs two
// clock reads and an array increment.
import Darwin
import Foundation
import ZhuyinEngine

/// What was counted, for one day or in total.
struct UsageCounters: Codable, Equatable {
    /// Characters sent to apps, punctuation included.
    var characters = 0
    /// Compositions sent.
    var compositions = 0
    /// Compositions sent without a pick, ⌫, Esc or ` in them: the first choice was right.
    var cleanCompositions = 0
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
    /// Time between keys within a burst of typing, and pauses between bursts (thinking), in milliseconds.
    var typingMilliseconds = 0
    var thinkingMilliseconds = 0

    init() {}

    // Decoding tolerates fields added later.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func value(_ key: CodingKeys) -> Int { (try? c.decodeIfPresent(Int.self, forKey: key)) ?? 0 }
        characters = value(.characters); compositions = value(.compositions); cleanCompositions = value(.cleanCompositions)
        keys = value(.keys); picksNormal = value(.picksNormal); picksCorrections = value(.picksCorrections)
        picksSingle = value(.picksSingle); picksPunctuation = value(.picksPunctuation); backspaces = value(.backspaces)
        escapes = value(.escapes); deleteAfterCommit = value(.deleteAfterCommit); literalZhuyin = value(.literalZhuyin)
        undoLiteral = value(.undoLiteral); correctedShown = value(.correctedShown)
        correctedCommitted = value(.correctedCommitted); phrasesAdded = value(.phrasesAdded)
        typingMilliseconds = value(.typingMilliseconds); thinkingMilliseconds = value(.thinkingMilliseconds)
    }

    var picks: Int { picksNormal + picksCorrections + picksSingle + picksPunctuation }
    /// Every correction the user made: picks, deletions, Esc and `.
    var corrections: Int { picks + backspaces + escapes + deleteAfterCommit + literalZhuyin }
    var isZero: Bool { self == UsageCounters() }
    /// Characters a minute while typing, pauses left out; nil without typing time.
    var activeSpeed: Double? {
        typingMilliseconds >= 1000 ? Double(characters) * 60_000 / Double(typingMilliseconds) : nil
    }
    /// The share of compositions sent as first shown, 0–1.
    var firstTimeRight: Double? { compositions > 0 ? Double(cleanCompositions) / Double(compositions) : nil }

    static func += (a: inout UsageCounters, b: UsageCounters) {
        a.characters &+= b.characters; a.compositions &+= b.compositions; a.cleanCompositions &+= b.cleanCompositions
        a.keys &+= b.keys; a.picksNormal &+= b.picksNormal; a.picksCorrections &+= b.picksCorrections
        a.picksSingle &+= b.picksSingle; a.picksPunctuation &+= b.picksPunctuation; a.backspaces &+= b.backspaces
        a.escapes &+= b.escapes; a.deleteAfterCommit &+= b.deleteAfterCommit; a.literalZhuyin &+= b.literalZhuyin
        a.undoLiteral &+= b.undoLiteral; a.correctedShown &+= b.correctedShown
        a.correctedCommitted &+= b.correctedCommitted; a.phrasesAdded &+= b.phrasesAdded
        a.typingMilliseconds &+= b.typingMilliseconds; a.thinkingMilliseconds &+= b.thinkingMilliseconds
    }

    static func + (a: UsageCounters, b: UsageCounters) -> UsageCounters {
        var r = a
        r += b
        return r
    }
}

/// A fixed set of buckets with upper bounds, for latencies, key intervals and pauses.  Adding is O(1) in a fixed
/// array.
struct Buckets: Codable, Equatable {
    var bounds: [Double]
    var counts: [Int]
    var maximum: Double = 0

    init(bounds: [Double]) {
        self.bounds = bounds
        counts = [Int](repeating: 0, count: bounds.count + 1)
    }

    var total: Int { counts.reduce(0, +) }

    mutating func add(_ value: Double) {
        var index = 0
        while index < bounds.count, value > bounds[index] { index += 1 }
        counts[index] &+= 1
        if value > maximum { maximum = value }
    }

    /// The `fraction` percentile, interpolated inside its bucket; nil when empty.
    func percentile(_ fraction: Double) -> Double? {
        let total = self.total
        guard total > 0 else { return nil }
        let target = fraction * Double(total)
        var seen = 0.0
        for (index, count) in counts.enumerated() where count > 0 {
            if seen + Double(count) >= target {
                let low = index == 0 ? 0 : bounds[index - 1]
                let high = index < bounds.count ? bounds[index] : max(maximum, low)
                return min(maximum, low + (high - low) * (target - seen) / Double(count))
            }
            seen += Double(count)
        }
        return maximum
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
        var index = 0
        while index < Self.bounds.count, milliseconds > Self.bounds[index] { index += 1 }
        if counts.count != Self.bounds.count + 1 { counts = [Int](repeating: 0, count: Self.bounds.count + 1) }
        counts[index] &+= 1
        if milliseconds > maximum { maximum = milliseconds }
    }

    /// The `fraction` percentile, interpolated inside its bucket; nil with no keys.
    func percentile(_ fraction: Double) -> Double? {
        var buckets = Buckets(bounds: Self.bounds)
        buckets.counts = counts
        buckets.maximum = maximum
        return buckets.percentile(fraction)
    }
}

/// How the user types, beyond the counts: key intervals (for the pause threshold), think pauses, the longest runs,
/// and when in the week the typing happens.
struct TypingRhythm: Codable, Equatable {
    /// Seconds between keys within bursts.
    var intervals = Buckets(bounds: [0.04, 0.06, 0.08, 0.1, 0.12, 0.15, 0.18, 0.22, 0.26, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.3,
                                     1.6, 2.0, 3.0, 4.0, 6.0])
    /// Seconds of pauses between bursts (up to `UsageStatsObserver.awayAfter`).
    var pauses = Buckets(bounds: [2, 3, 4, 5, 6, 8, 10, 13, 16, 20, 25, 30, 40, 50, 60])
    /// The most characters sent in one burst, and without a correction.
    var longestRun = 0
    var longestCleanRun = 0
    /// Characters by weekday (1 Sunday … 7 Saturday, as `Calendar`) and hour: index (weekday − 1) × 24 + hour.
    var hours = [Int](repeating: 0, count: 7 * 24)

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let empty = TypingRhythm()
        intervals = (try? c.decodeIfPresent(Buckets.self, forKey: .intervals)) ?? empty.intervals
        pauses = (try? c.decodeIfPresent(Buckets.self, forKey: .pauses)) ?? empty.pauses
        if intervals.counts.count != intervals.bounds.count + 1 { intervals = empty.intervals }
        if pauses.counts.count != pauses.bounds.count + 1 { pauses = empty.pauses }
        longestRun = (try? c.decodeIfPresent(Int.self, forKey: .longestRun)) ?? 0
        longestCleanRun = (try? c.decodeIfPresent(Int.self, forKey: .longestCleanRun)) ?? 0
        let stored = (try? c.decodeIfPresent([Int].self, forKey: .hours)) ?? []
        hours = stored.count == 7 * 24 ? stored : empty.hours
    }

    /// The gap that ends a burst: four times the usual key interval, at least 2 seconds.
    var pauseThreshold: Double { max(2, 4 * (intervals.percentile(0.5) ?? 0.25)) }
}

/// The contents of stats.json.
struct UsageData: Codable, Equatable {
    static let schemaVersion = 2
    var schema = UsageData.schemaVersion
    /// When counting started (or was last cleared).
    var since: Date
    var totals = UsageCounters()
    /// By local day, yyyy-MM-dd; at most `UsageStatsStore.keptDays`.
    var days: [String: UsageCounters] = [:]
    var latency = LatencyHistogram()
    var rhythm = TypingRhythm()

    init(since: Date) {
        self.since = since
    }

    // Reads version 1 too: the fields it lacks start empty.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        since = try c.decode(Date.self, forKey: .since)
        totals = (try? c.decodeIfPresent(UsageCounters.self, forKey: .totals)) ?? UsageCounters()
        days = (try? c.decodeIfPresent([String: UsageCounters].self, forKey: .days)) ?? [:]
        latency = (try? c.decodeIfPresent(LatencyHistogram.self, forKey: .latency)) ?? LatencyHistogram()
        if latency.counts.count != LatencyHistogram.bounds.count + 1 { latency = LatencyHistogram() }
        rhythm = (try? c.decodeIfPresent(TypingRhythm.self, forKey: .rhythm)) ?? TypingRhythm()
    }
}

/// The counters, kept in memory and written to stats.json at most every `saveInterval`, when the input method is
/// switched away, when the settings window opens and before the process exits, and only after a change.  Used on
/// the main thread; the file is written on its own queue.  Adding is O(1): today's counters and the totals are plain
/// structs, the rhythm uses fixed arrays, and the day only changes at midnight.
final class UsageStatsStore {
    static var shared = UsageStatsStore(directory: EngineHost.userDirectory)
    static let fileName = "stats.json"
    /// Days kept for the history.
    static let keptDays = 60
    /// The longest time between writes while typing.
    static let saveInterval: TimeInterval = 600

    /// Nil: in memory only (self-tests, previews).
    let url: URL?
    /// Everything but today, which is `today` until the day changes.
    private var stored: UsageData
    private var today = UsageCounters()
    private var todayKey = ""
    private var todayStart = 0.0
    private var todayEnd = -1.0
    private var todayWeekday = 1
    private(set) var rhythm: TypingRhythm
    var now: () -> Date = Date.init
    /// Seconds since boot, for the write interval.
    var uptime: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    private var dirty = false
    private var revision = 0
    private var writing = false
    /// When the file was last written (or counting began in this process), in `uptime` seconds.
    private var lastSave: TimeInterval?
    /// Writes so far, for the self-test.
    private(set) var writes = 0
    private let queue = DispatchQueue(label: "ZhuyinType.stats", qos: .utility)
    private let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()

    init(directory: URL?, data: UsageData? = nil) {
        url = directory?.appendingPathComponent(Self.fileName)
        var loaded = UsageData(since: Date())
        if let data {
            loaded = data
        } else if let url, let bytes = try? Data(contentsOf: url),
                  let decoded = try? Self.decoder.decode(UsageData.self, from: bytes) {
            loaded = decoded
        }
        rhythm = loaded.rhythm
        stored = loaded
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

    /// Everything, today included: for the 分析 page and for writing.
    var data: UsageData {
        var data = stored
        data.rhythm = rhythm
        if !todayKey.isEmpty, !today.isZero { data.days[todayKey] = (data.days[todayKey] ?? UsageCounters()) + today }
        return data
    }

    /// The local day `date` falls in.
    func dayKey(_ date: Date) -> String {
        let t = date.timeIntervalSinceReferenceDate
        if t >= todayStart, t < todayEnd { return todayKey }
        return dayFormatter.string(from: date)
    }

    func date(ofDay key: String) -> Date? { dayFormatter.date(from: key) }

    /// Moves to the day `t` falls in: today's counters join the history.  Once a day.
    private func roll(to date: Date) {
        if !todayKey.isEmpty, !today.isZero {
            stored.days[todayKey] = (stored.days[todayKey] ?? UsageCounters()) + today
        }
        today = UsageCounters()
        let calendar = Calendar.current
        let start = calendar.startOfDay(for: date)
        todayStart = start.timeIntervalSinceReferenceDate
        todayEnd = (calendar.date(byAdding: .day, value: 1, to: start) ?? start.addingTimeInterval(86_400)).timeIntervalSinceReferenceDate
        todayKey = dayFormatter.string(from: date)
        todayWeekday = calendar.component(.weekday, from: date)
        if stored.days.count > Self.keptDays {
            for old in stored.days.keys.sorted().prefix(stored.days.count - Self.keptDays) { stored.days[old] = nil }
        }
    }

    /// Adds to today and the totals; O(1) except once a day.
    func add(_ delta: UsageCounters) {
        guard !delta.isZero else { return }
        let date = now()
        let t = date.timeIntervalSinceReferenceDate
        if t < todayStart || t >= todayEnd { roll(to: date) }
        today += delta
        stored.totals += delta
        if delta.characters > 0 {
            let hour = min(23, max(0, Int((t - todayStart) / 3600)))
            rhythm.hours[(todayWeekday - 1) * 24 + hour] &+= delta.characters
        }
        changed()
    }

    func addLatency(milliseconds: Double) {
        stored.latency.add(milliseconds: milliseconds)
        changed()
    }

    func addInterval(seconds: Double) { rhythm.intervals.add(seconds) }
    func addPause(seconds: Double) { rhythm.pauses.add(seconds) }
    func noteRuns(burst: Int, clean: Int) {
        if burst > rhythm.longestRun { rhythm.longestRun = burst }
        if clean > rhythm.longestCleanRun { rhythm.longestCleanRun = clean }
    }

    /// Marks a change; writes only when the last write is `saveInterval` old.
    private func changed() {
        dirty = true
        revision &+= 1
        let now = uptime()
        guard let last = lastSave else { lastSave = now; return }
        if now - last >= Self.saveInterval { saveIfNeeded() }
    }

    /// Writes in the background if anything changed since the last write.
    func saveIfNeeded() {
        guard dirty, url != nil, !writing else { return }
        writing = true
        lastSave = uptime()
        writes += 1
        let snapshot = data
        let savedRevision = revision
        queue.async {
            let success = self.write(snapshot)
            DispatchQueue.main.async {
                self.writing = false
                if success && self.revision == savedRevision { self.dirty = false }
            }
        }
    }

    /// Starts over: the counters, the rhythm, the history and the file.
    func clear() throws {
        if let url {
            try queue.sync {
                if FileManager.default.fileExists(atPath: url.path) { try FileManager.default.removeItem(at: url) }
            }
        }
        revision &+= 1
        stored = UsageData(since: now())
        rhythm = TypingRhythm()
        today = UsageCounters()
        todayKey = ""
        todayEnd = -1
        dirty = false
    }

    /// Writes now if anything changed, and waits: before the process exits, and in self-tests.
    func flush() {
        queue.sync {}
        guard dirty, url != nil else { return }
        let snapshot = data
        writes += 1
        lastSave = uptime()
        if queue.sync(execute: { write(snapshot) }) { dirty = false }
    }

    private func write(_ snapshot: UsageData) -> Bool {
        guard let url, let encoded = Self.encode(snapshot) else { return false }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try encoded.write(to: url, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
            return true
        } catch {
            EngineHost.logger.error("Stats write failed: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }
}

/// Watches one `ControllerCore` and counts.  It never changes what the core does.  Each key costs a few comparisons,
/// one clock read and integer additions; bursts are told apart by the time since the last key only.
final class UsageStatsObserver {
    let store: UsageStatsStore
    /// Seconds since the system started, as key-down times.
    var clock: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    /// Seconds after sending in which ⌫ on the sent text counts (as the engine's own rule).
    static let deleteWindow: TimeInterval = 30
    /// A gap longer than this is time away, not thinking: it counts as neither.
    static let awayAfter: TimeInterval = 60

    private var lastCommit: TimeInterval = -.infinity
    private var deletedSinceCommit = false
    /// The text before the last `, while ` or Esc can still restore it.
    private var literalBefore: String?
    /// The shown sentence had a typing correction at some point of this composition.
    private var sawCorrected = false
    /// The composition had a pick, ⌫, Esc or `.
    private var corrected = false
    private var lastKey: TimeInterval?
    /// The gap that ends a burst, fixed for this observer (one session) so its numbers are stable.
    let pauseThreshold: TimeInterval
    private var burst = 0
    private var cleanRun = 0

    init(store: UsageStatsStore) {
        self.store = store
        pauseThreshold = store.rhythm.pauseThreshold
    }

    /// `corrected`: the sentence now shown has a typing correction.  `latency`: nanoseconds the key took.  `time`: when
    /// the key went down, if known.
    func observe(_ input: CorrectionRecorder.Input, before: Presentation, after: Presentation, commit: String?,
                 handled: Bool, options: ComposerOptions, secure: Bool, corrected shownCorrected: Bool, latency: UInt64?) {
        guard !secure else {
            literalBefore = nil
            sawCorrected = false
            corrected = false
            lastCommit = -.infinity
            lastKey = nil
            burst = 0
            cleanRun = 0
            return
        }
        var count = UsageCounters()
        let composing = !before.isEmpty || !after.isEmpty
        if case .key(_, let time) = input {
            // Rhythm: the gap since the last key decides typing, thinking or away.
            let now = time ?? clock()
            if let last = lastKey {
                let gap = now - last
                if gap >= 0, gap <= pauseThreshold {
                    count.typingMilliseconds = Int(gap * 1000)
                    store.addInterval(seconds: gap)
                } else if gap > pauseThreshold, gap <= Self.awayAfter {
                    count.thinkingMilliseconds = Int(gap * 1000)
                    store.addPause(seconds: gap)
                    burst = 0
                    cleanRun = 0
                } else {
                    burst = 0
                    cleanRun = 0
                }
            }
            lastKey = now
            if handled, composing {
                count.keys = 1
                if let latency { store.addLatency(milliseconds: Double(latency) / 1_000_000) }
            }
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
        if count.picks > 0 || count.backspaces > 0 || count.escapes > 0 || count.literalZhuyin > 0 || count.deleteAfterCommit > 0 {
            corrected = true
            cleanRun = 0
        }
        if let notice = after.notice, notice.hasPrefix("已加入自訂詞"), notice != before.notice { count.phrasesAdded = 1 }
        if shownCorrected, !after.isEmpty { sawCorrected = true }
        if let commit, !commit.isEmpty {
            let characters = commit.count
            count.characters = characters
            burst += characters
            cleanRun += characters
            store.noteRuns(burst: burst, clean: cleanRun)
            if after.isEmpty {
                count.compositions = 1
                if !corrected { count.cleanCompositions = 1 }
                if sawCorrected { count.correctedShown = 1 }
                if shownCorrected { count.correctedCommitted = 1 }
                sawCorrected = false
                corrected = false
                lastCommit = clock()
                deletedSinceCommit = false
            }
        } else if after.isEmpty {
            // Cleared without sending (⌫ or Esc): what is typed next is the same attempt, still corrected.
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
