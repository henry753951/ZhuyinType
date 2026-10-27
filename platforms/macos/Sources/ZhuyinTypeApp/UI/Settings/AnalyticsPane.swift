import AppKit
import Charts
import SwiftUI
import ZhuyinCore

/// 分析: a recap of this Mac's typing (counts only, from `UsageStatsStore`), how the input method itself performs,
/// and the switch for counting.  Refreshes every two seconds only while the page is on screen; nothing runs otherwise.
struct AnalyticsPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    /// Fixed data and process numbers for the UI previews.
    var store: UsageStatsStore = .shared
    var metrics: ProcessMetrics?
    @State private var period: Period = .month
    @State private var confirmingClear = false
    @State private var words: (phrases: Int, learned: Int)?
    /// Advanced every two seconds while the page is shown.
    @State private var tick = 0

    enum Period: String, CaseIterable { case month, all }

    var body: some View {
        // Reading `tick` redraws the page (keeping its scroll position) every time it advances.
        let _ = tick
        content(UsageSummary(store: store, period: period), metrics ?? ProcessMetrics.sample())
            .task {
                // Cancelled when the page leaves the screen: no timer while it is hidden.
                while !Task.isCancelled {
                    try? await Task.sleep(nanoseconds: 2_000_000_000)
                    if !Task.isCancelled { tick &+= 1 }
                }
            }
        .onAppear(perform: countWords)
        .onChange(of: model.engine != nil) { _, _ in countWords() }
        .alert("要清除使用統計嗎？", isPresented: $confirmingClear) {
            Button("清除", role: .destructive) { store.clear() }
            Button("取消", role: .cancel) {}
        } message: {
            Text("所有計數與每天的紀錄都會刪除，無法復原。")
        }
    }

    private func countWords() {
        guard let engine = model.engine else { return }
        words = (engine.userPhrases().count, engine.learnedEntries().count)
    }

    @ViewBuilder
    private func content(_ summary: UsageSummary, _ metrics: ProcessMetrics) -> some View {
        let counting = preferences.recordsUsageStats
        SettingsForm {
            SettingsSection(card: false) {
                Picker("期間", selection: $period) {
                    Text("過去 30 天").tag(Period.month)
                    Text("全部").tag(Period.all)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                RecapHero(summary: summary, counting: counting)
            }
            if summary.counters.characters > 0 {
                SettingsSection("每天送出的字") {
                    DailyChart(days: summary.days)
                        .frame(height: 150)
                }
            }
            RhythmSection(summary: summary, rhythm: store.rhythm)
            SettingsSection("打字", card: false) {
                StatGrid(stats: [
                    ("送出的字", summary.counters.characters, "character"),
                    ("組字", summary.counters.compositions, "text.cursor"),
                    ("按鍵", summary.counters.keys, "keyboard"),
                    ("從清單選字", summary.counters.picks, "list.bullet"),
                    ("改字", summary.counters.corrections, "pencil"),
                    ("加入自訂詞", summary.counters.phrasesAdded, "plus.square.on.square"),
                ])
            }
            SettingsSection("從清單選字") {
                Breakdown(rows: [
                    ("一般清單", summary.counters.picksNormal), ("修正清單", summary.counters.picksCorrections),
                    ("單字清單", summary.counters.picksSingle), ("標點", summary.counters.picksPunctuation),
                ])
            }
            SettingsSection("改字") {
                Breakdown(rows: [
                    ("⌫ 刪掉重打", summary.counters.backspaces), ("Esc 清除", summary.counters.escapes),
                    ("送出後刪掉重打", summary.counters.deleteAfterCommit), ("打出注音本身", summary.counters.literalZhuyin),
                    ("恢復成原本的字", summary.counters.undoLiteral),
                ])
                SettingRow(title: "自動修正", subtitle: "整句中有按錯修正的字：送出的組字 / 出現過的組字",
                           value: "\(summary.counters.correctedCommitted.formatted()) / \(summary.counters.correctedShown.formatted())")
            }
            SettingsSection("你的字詞") {
                SettingRow(title: "自訂詞", value: words.map { "\($0.phrases.formatted()) 個" } ?? "—")
                SettingRow(title: "記住的字詞", value: words.map { "\($0.learned.formatted()) 個" } ?? "—")
            }
            SettingsSection("效能") {
                DrillRow(model: model, title: "效能", subtitle: "按鍵反應、CPU、記憶體、耗電與載入時間",
                         value: store.data.latency.total > 0
                            ? "p95 " + PerformanceFormat.ms(store.data.latency.percentile(0.95)) : "還沒有資料",
                         subpage: .performance)
            }
            SettingsSection("統計") {
                SettingRow(title: "記錄使用統計",
                           subtitle: counting ? "從 \(store.data.since.formatted(date: .long, time: .omitted)) 開始記錄" : "關閉時不計數",
                           accessory: .info(title: "使用統計", content: AnyView(UsageInfo()))) {
                    RowSwitch(title: "記錄使用統計", isOn: binding(preferences, \.recordsUsageStats))
                }
                SettingRow(title: "清除統計", subtitle: "刪除所有計數與每天的紀錄") {
                    Button("清除…") { confirmingClear = true }
                }
            } footer: {
                FooterText("只記錄次數與時間，不記錄打的字、讀音與 App；只存在這台 Mac。統計只在記憶體中累加，每 10 分鐘最多寫入一次，不影響打字。")
            }
        }
    }
}

/// The numbers for one period.
struct UsageSummary {
    var counters: UsageCounters
    /// The last 30 days, oldest first, with days without typing.
    var days: [(date: Date, counters: UsageCounters)]
    /// Consecutive days with typing, up to today (or yesterday).
    var streak: Int
    var bestDay: (date: Date, characters: Int)?
    var period: AnalyticsPane.Period

    init(store: UsageStatsStore, period: AnalyticsPane.Period) {
        self.period = period
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: store.now())
        days = (0..<30).reversed().compactMap { back in
            guard let date = calendar.date(byAdding: .day, value: -back, to: today) else { return nil }
            return (date, store.data.days[store.dayKey(date)] ?? UsageCounters())
        }
        counters = period == .all ? store.data.totals : days.reduce(UsageCounters()) { $0 + $1.counters }
        var streak = 0
        for day in days.reversed().drop(while: { $0.date == today && $0.counters.characters == 0 }) {
            guard day.counters.characters > 0 else { break }
            streak += 1
        }
        self.streak = streak
        let best = store.data.days.max { $0.value.characters < $1.value.characters }
        bestDay = best.flatMap { entry in
            guard entry.value.characters > 0, let date = store.date(ofDay: entry.key) else { return nil }
            return (date, entry.value.characters)
        }
    }
}

private struct RecapHero: View {
    let summary: UsageSummary
    let counting: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(summary.period == .month ? "過去 30 天，你打了" : "開始記錄以來，你打了")
                .font(.callout)
                .foregroundStyle(.secondary)
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(summary.counters.characters.formatted())
                    .font(.system(size: 44, weight: .semibold, design: .rounded))
                    .monospacedDigit()
                    .contentTransition(reduceMotion ? .identity : .numericText())
                Text("個字").font(.title3).foregroundStyle(.secondary)
            }
            TrailingFlow(spacing: 8, lineSpacing: 6, leading: true) {
                if summary.streak > 1 { Chip(symbol: "flame", text: "連續 \(summary.streak) 天") }
                if let best = summary.bestDay {
                    Chip(symbol: "star", text: "最多的一天 \(best.date.formatted(.dateTime.month().day()))・\(best.characters.formatted()) 字")
                }
                if let right = summary.counters.firstTimeRight {
                    Chip(symbol: "checkmark.seal", text: "一次就對 \(Int((right * 100).rounded()))%")
                }
                if summary.counters.characters > 0 {
                    let rate = Double(summary.counters.corrections) * 100 / Double(summary.counters.characters)
                    Chip(symbol: "pencil", text: "每 100 字改 \(rate.formatted(.number.precision(.fractionLength(1)))) 次")
                }
                if !counting { Chip(symbol: "pause.circle", text: "已暫停記錄") }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(RowMetrics.tilePadding)
        .background(
            RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous)
                .fill(LinearGradient(colors: [Color.accentColor.opacity(0.16), Color.accentColor.opacity(0.04)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing))
        )
        .accessibilityElement(children: .combine)
    }
}

private struct Chip: View {
    let symbol: String
    let text: String
    var body: some View {
        Label(text, systemImage: symbol)
            .font(.caption)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Capsule().fill(.background.opacity(0.7)))
            .foregroundStyle(.secondary)
    }
}

/// Characters per day: one series, so one colour and no legend; the best day carries its number.
private struct DailyChart: View {
    let days: [(date: Date, counters: UsageCounters)]
    @State private var selected: Date?

    var body: some View {
        let best = days.max { $0.counters.characters < $1.counters.characters }
        Chart {
            ForEach(days, id: \.date) { day in
                BarMark(x: .value("日期", day.date, unit: .day), y: .value("字數", day.counters.characters), width: .ratio(0.62))
                    .foregroundStyle(Color.accentColor.opacity(selected == nil || selected == day.date ? 0.85 : 0.35))
                    .cornerRadius(3)
            }
            if let best, best.counters.characters > 0, selected == nil {
                PointMark(x: .value("日期", best.date, unit: .day), y: .value("字數", best.counters.characters))
                    .opacity(0)
                    .annotation(position: .top, spacing: 2) {
                        Text(best.counters.characters.formatted()).font(.caption2).foregroundStyle(.secondary)
                    }
            }
            if let selected, let day = days.first(where: { $0.date == selected }) {
                RuleMark(x: .value("日期", selected, unit: .day))
                    .foregroundStyle(.secondary.opacity(0.3))
                    .annotation(position: .top, overflowResolution: .init(x: .fit, y: .disabled)) {
                        Text("\(selected.formatted(.dateTime.month().day()))・\(day.counters.characters.formatted()) 字")
                            .font(.caption)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(RoundedRectangle(cornerRadius: 5).fill(.background))
                    }
            }
        }
        .chartXSelection(value: $selected)
        .chartXAxis {
            AxisMarks(values: .stride(by: .day, count: 7)) { _ in
                AxisValueLabel(format: .dateTime.month(.defaultDigits).day())
            }
        }
        .chartYAxis {
            AxisMarks(position: .leading, values: .automatic(desiredCount: 3)) { _ in
                AxisGridLine().foregroundStyle(.quaternary)
                AxisValueLabel()
            }
        }
        .accessibilityLabel("過去 30 天每天送出的字數")
    }
}

private struct StatGrid: View {
    let stats: [(title: String, value: Int, symbol: String)]
    var body: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: RowMetrics.tileSpacing), count: 3),
                  spacing: RowMetrics.tileSpacing) {
            ForEach(stats, id: \.title) { stat in
                VStack(alignment: .leading, spacing: 4) {
                    Label(stat.title, systemImage: stat.symbol)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(stat.value.formatted())
                        .font(.system(size: 20, weight: .semibold, design: .rounded))
                        .monospacedDigit()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(RowMetrics.tilePadding)
                .background(RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous).fill(Color.primary.opacity(0.03)))
                .accessibilityElement(children: .combine)
            }
        }
    }
}

/// Parts of a whole as labelled bars of one colour, each with its number.
private struct Breakdown: View {
    let rows: [(title: String, value: Int)]
    var body: some View {
        let top = max(1, rows.map(\.value).max() ?? 1)
        ForEach(rows, id: \.title) { row in
            SettingRow(title: row.title, value: row.value.formatted()) {
                GeometryReader { proxy in
                    Capsule().fill(Color.accentColor.opacity(0.75))
                        .frame(width: max(row.value > 0 ? 3 : 0, proxy.size.width * CGFloat(row.value) / CGFloat(top)), height: 6)
                        .frame(maxHeight: .infinity)
                }
                .frame(width: 140, height: 14)
            }
        }
    }
}

/// 分析 › 效能: per-key latency from the counters, the rest from the system for this process.
struct PerformancePage: View {
    var store: UsageStatsStore = .shared
    @State private var tick = 0
    var body: some View {
        let _ = tick
        SettingsForm { PerformanceSection(latency: store.data.latency, metrics: ProcessMetrics.sample()) }
            .task {
                while !Task.isCancelled {
                    try? await Task.sleep(nanoseconds: 2_000_000_000)
                    if !Task.isCancelled { tick &+= 1 }
                }
            }
    }
}

enum PerformanceFormat {
    static func ms(_ value: Double?) -> String {
        guard let value else { return "—" }
        return value < 10 ? String(format: "%.1f ms", value) : String(format: "%.0f ms", value)
    }
}

/// How the input method performs: per-key latency from the counters, the rest from the system for this process.
private struct PerformanceSection: View {
    let latency: LatencyHistogram
    let metrics: ProcessMetrics

    var body: some View {
        SettingsSection {
            if latency.total > 0 {
                SettingRow(title: "按鍵反應", subtitle: "從按鍵到畫面更新；\(latency.total.formatted()) 個按鍵",
                           value: ["p50 \(ms(latency.percentile(0.5)))", "p95 \(ms(latency.percentile(0.95)))",
                                   "最慢 \(ms(latency.maximum))"].joined(separator: "・"))
                LatencyChart(latency: latency).frame(height: 110)
            } else {
                SettingRow(title: "按鍵反應", value: "還沒有資料")
            }
            SettingRow(title: "CPU 時間", subtitle: "自 \(ProcessMetrics.launched.formatted(date: .omitted, time: .shortened)) 啟動以來",
                       value: String(format: "%.1f 秒", metrics.cpuSeconds))
            SettingRow(title: "記憶體", value: [metrics.footprintBytes.map { "目前 \(bytes($0))" },
                                               metrics.peakFootprintBytes.map { "最高 \(bytes($0))" }]
                .compactMap { $0 }.joined(separator: "・").nonEmpty ?? "無法取得")
            SettingRow(title: "耗電", subtitle: "macOS 的估計，自啟動以來", value: energy)
            SettingRow(title: "載入時間", subtitle: "詞庫與語言模型、整句重排、逐字模型", value: loadTimes)
        }
    }

    private var energy: String {
        guard let joules = metrics.energyJoules else { return "macOS 沒有提供" }
        let watts = metrics.running > 0 ? joules / metrics.running : 0
        let average = watts >= 1 ? String(format: "%.2f 瓦", watts) : String(format: "%.0f 毫瓦", watts * 1000)
        return String(format: "%.1f 焦耳・平均 ", joules) + average
    }

    private var loadTimes: String {
        let times = EngineHost.loadDurations
        return [times.engine, times.reranker, times.characters].map { $0.map { ms($0 * 1000) } ?? "—" }.joined(separator: "・")
    }

    private func ms(_ value: Double?) -> String { PerformanceFormat.ms(value) }

    private func bytes(_ value: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .memory)
    }
}

/// Keys by latency bucket: one series, one colour.
private struct LatencyChart: View {
    let latency: LatencyHistogram
    var body: some View {
        let labels = LatencyHistogram.bounds.map { $0 < 1000 ? "\(Int($0))" : "1s" } + ["更慢"]
        let rows = Array(zip(labels, latency.counts)).enumerated().filter { $0.element.1 > 0 || $0.offset < 12 }
        Chart(rows, id: \.offset) { row in
            BarMark(x: .value("毫秒", row.element.0), y: .value("按鍵", row.element.1), width: .ratio(0.7))
                .foregroundStyle(Color.accentColor.opacity(0.8))
                .cornerRadius(2)
        }
        .chartYAxis {
            AxisMarks(position: .leading, values: .automatic(desiredCount: 3)) { _ in
                AxisGridLine().foregroundStyle(.quaternary)
                AxisValueLabel()
            }
        }
        .chartXAxisLabel("毫秒以內", alignment: .trailing)
        .accessibilityLabel("按鍵反應時間的分布")
    }
}

/// What 使用統計 counts (docs/product/DESIGN.md §2.8).
struct UsageInfo: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            FactRow(symbol: "number", text: "送出的字數、組字次數、按鍵數")
            FactRow(symbol: "list.bullet", text: "從各種清單選字的次數、各種改字的次數、加入自訂詞的次數")
            FactRow(symbol: "wand.and.stars", text: "整句有按錯修正的組字數")
            FactRow(symbol: "timer", text: "每個按鍵的反應時間，只記分布與最慢的一次")
            FactRow(symbol: "calendar", text: "每天一筆，保留 \(UsageStatsStore.keptDays) 天")
            FactRow(symbol: "eye.slash", text: "不記錄打的字、讀音、按了哪些鍵與 App")
            FactRow(symbol: "lock", text: "密碼欄位中不計數")
            FactRow(symbol: "internaldrive", text: "存在這台 Mac 的 stats.json")
        }
    }
}

private extension String {
    var nonEmpty: String? { isEmpty ? nil : self }
}

/// 打字節奏: how fast while typing (pauses left out), how much was thinking, the longest runs, how often the first
/// choice was right, keys per character, and when in the week the typing happens.
private struct RhythmSection: View {
    let summary: UsageSummary
    let rhythm: TypingRhythm

    var body: some View {
        let c = summary.counters
        SettingsSection("打字節奏", card: false) {
            StatGrid(stats: [
                ("打字速度（字/分鐘）", Int((c.activeSpeed ?? 0).rounded()), "speedometer"),
                ("一次就對（%）", Int(((c.firstTimeRight ?? 0) * 100).rounded()), "checkmark.seal"),
                ("最長連續（字）", rhythm.longestRun, "arrow.right.to.line"),
            ])
        }
        SettingsSection {
            SettingRow(title: "打字時間・思考停頓",
                       subtitle: "停頓超過 \(String(format: "%.0f", rhythm.pauseThreshold)) 秒算思考，超過 1 分鐘不計",
                       value: "\(duration(c.typingMilliseconds))・\(duration(c.thinkingMilliseconds))") {
                SplitBar(first: c.typingMilliseconds, second: c.thinkingMilliseconds)
            }
            SettingRow(title: "一次不改字最長", value: "\(rhythm.longestCleanRun.formatted()) 字")
            SettingRow(title: "每個字按幾個鍵", value: c.characters > 0
                ? String(format: "%.1f", Double(c.keys) / Double(c.characters)) : "—")
            SettingRow(title: "思考停頓的中位數", value: rhythm.pauses.percentile(0.5).map { String(format: "%.1f 秒", $0) } ?? "—")
            VStack(alignment: .leading, spacing: 8) {
                Text("什麼時候打字")
                WeekHeatMap(hours: rhythm.hours)
            }
        } footer: {
            FooterText("打字速度只算連續打字的時間；一次就對是沒有選字、⌫ 或 Esc 就送出的組字。")
        }
    }

    private func duration(_ milliseconds: Int) -> String {
        let minutes = milliseconds / 60_000
        return minutes >= 60 ? "\(minutes / 60) 小時 \(minutes % 60) 分" : "\(minutes) 分"
    }
}

/// Two parts of a whole as one bar: typing (accent) and thinking (grey).
private struct SplitBar: View {
    let first: Int
    let second: Int
    var body: some View {
        GeometryReader { proxy in
            let total = max(1, first + second)
            HStack(spacing: 2) {
                Capsule().fill(Color.accentColor.opacity(0.8))
                    .frame(width: max(first > 0 ? 3 : 0, (proxy.size.width - 2) * CGFloat(first) / CGFloat(total)))
                Capsule().fill(.quaternary)
            }
        }
        .frame(width: 160, height: 6)
        .accessibilityHidden(true)
    }
}

/// Characters by weekday and hour: one colour, darker for more.  Counts only.
private struct WeekHeatMap: View {
    let hours: [Int]
    private static let weekdays = ["日", "一", "二", "三", "四", "五", "六"]
    var body: some View {
        let top = max(1, hours.max() ?? 1)
        VStack(alignment: .leading, spacing: 2) {
            ForEach(0..<7, id: \.self) { day in
                HStack(spacing: 2) {
                    Text(Self.weekdays[day]).font(.caption2).foregroundStyle(.secondary).frame(width: 14)
                    ForEach(0..<24, id: \.self) { hour in
                        let value = hours.indices.contains(day * 24 + hour) ? hours[day * 24 + hour] : 0
                        RoundedRectangle(cornerRadius: 2)
                            .fill(value == 0 ? AnyShapeStyle(.quaternary.opacity(0.5))
                                  : AnyShapeStyle(Color.accentColor.opacity(0.15 + 0.85 * Double(value) / Double(top))))
                            .frame(maxWidth: .infinity)
                            .frame(height: 11)
                    }
                }
            }
            HStack(spacing: 0) {
                Color.clear.frame(width: 16)
                ForEach([0, 6, 12, 18], id: \.self) { hour in
                    Text("\(hour)").font(.caption2).foregroundStyle(.tertiary).frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .accessibilityLabel("一週中每小時打的字數")
    }
}
