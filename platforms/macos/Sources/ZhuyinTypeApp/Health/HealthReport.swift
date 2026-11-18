import Foundation

/// 回報問題: the text the sheet shows, copies and saves.  Markdown, so it pastes into a GitHub issue as headings and
/// lists and still reads as plain text; the same layout, section titles and labels as the Linux and Windows versions.
/// Never typed text, custom phrases, learned words, correction logs, context, app lists or other apps' names (only
/// the app holding secure input is named: it is the one fact that explains it).  The home directory is 「~」 and the
/// account name 「<使用者>」 everywhere.
enum HealthReport {
    /// Our own recent errors (error and fault level, this process), or nil when the log could not be read in time.
    struct LogLine: Equatable {
        var date: Date
        var message: String
    }

    struct Section: Equatable {
        var title: String
        var lines: [HealthDetail]
    }

    static func text(facts: HealthFacts, checks: [HealthCheck], errors: [LogLine]?) -> String {
        var blocks: [[String]] = [["## ZhuyinType 問題回報"], list(header(facts, checks))]
        if !checks.isEmpty {
            blocks.append(["### 檢查結果"])
            blocks.append(checks.flatMap { check in
                ["- " + check.reportLine] + check.details.map { "  - \($0.label)：\($0.value)" }
            })
        }
        for section in sections(facts) where !section.lines.isEmpty {
            blocks += [["### \(section.title)"], list(section.lines)]
        }
        blocks.append(["### 最近的錯誤"])
        if let errors {
            let time = HealthChecks.formatter("HH:mm:ss")
            blocks.append(errors.isEmpty ? ["（沒有）"]
                : ["```text"] + errors.map { "\(time.string(from: $0.date)) \(oneLine($0.message))" } + ["```"])
        } else {
            blocks.append(["（無法讀取）"])
        }
        let text = blocks.map { $0.joined(separator: "\n") }.joined(separator: "\n\n") + "\n"
        return anonymize(text, home: facts.home, user: facts.user)
    }

    private static func list(_ lines: [HealthDetail]) -> [String] { lines.map { "- \($0.label)：\($0.value)" } }

    private static func header(_ facts: HealthFacts, _ checks: [HealthCheck]) -> [HealthDetail] {
        [HealthDetail("產生時間", HealthChecks.timestamp(facts.now)), HealthDetail("結果", HealthChecks.summary(checks)),
         HealthDetail("版本", HealthChecks.versionText(facts)),
         HealthDetail("系統", "macOS \(facts.systemVersion)（\(facts.systemBuild)）"),
         HealthDetail("架構", facts.architecture)]
    }

    // MARK: Sections

    /// Every section after 檢查結果 and before 最近的錯誤, in the report's order; a section without lines is left out.
    static func sections(_ facts: HealthFacts) -> [Section] {
        [Section(title: "版本與資料", lines: data(facts)), Section(title: "安裝", lines: install(facts)),
         Section(title: "輸入法", lines: input(facts)), Section(title: "服務", lines: service(facts)),
         Section(title: "更新", lines: HealthChecks.updateDetails(facts)),
         Section(title: "改過的設定", lines: facts.changedSettings.map(\.detail)),
         Section(title: "顯示與輔助功能", lines: display(facts))]
    }

    private static func shown(_ url: URL, _ facts: HealthFacts) -> String { HealthChecks.abbreviate(url.path, home: facts.home) }

    private static func data(_ facts: HealthFacts) -> [HealthDetail] {
        var lines: [HealthDetail] = []
        let folder = facts.dataFolder?.standardizedFileURL.path
        if let dataFolder = facts.dataFolder { lines.append(HealthDetail("資料夾", shown(dataFolder, facts))) }
        for item in facts.dataItems {
            guard let url = item.url else {
                lines.append(HealthDetail(item.name, "缺少"))
                continue
            }
            let path = url.standardizedFileURL.path
            var name = folder.flatMap { path.hasPrefix($0 + "/") ? String(path.dropFirst($0.count + 1)) : nil }
                ?? shown(url, facts)
            if url.hasDirectoryPath, !name.hasSuffix("/") { name += "/" }
            lines.append(HealthDetail(item.name, item.bytes.map { "\(name)，\(size($0))" } ?? name))
        }
        if !facts.bundledData.isEmpty {
            lines.append(HealthDetail("隨附的資料版本", facts.bundledData.map { "\($0.component) \($0.release)（\($0.hash)）" }
                .joined(separator: "、")))
        }
        if let times = facts.loadTimes {
            let parts = [("詞庫與語言模型", times.engine), ("選字模型", times.reranker), ("單字模型", times.characters)]
                .compactMap { name, seconds in seconds.map { "\(name) \(Int(($0 * 1000).rounded())) ms" } }
            if !parts.isEmpty { lines.append(HealthDetail("模型載入", parts.joined(separator: "、"))) }
        }
        return lines
    }

    private static func install(_ facts: HealthFacts) -> [HealthDetail] {
        var lines = [HealthDetail("位置", shown(facts.running, facts))]
        if let owner = facts.app.owner { lines.append(HealthDetail("擁有者", owner == facts.user ? "<使用者>" : owner)) }
        if let permissions = facts.app.permissions { lines.append(HealthDetail("權限", permissions)) }
        if let quarantined = facts.app.quarantined { lines.append(.yesNo("隔離標記（com.apple.quarantine）", quarantined)) }
        if let file = facts.preferencesFile { lines.append(HealthDetail("設定檔", shown(file, facts))) }
        if let folder = facts.userData { lines.append(HealthDetail("使用者資料", shown(folder, facts))) }
        return lines
    }

    private static func input(_ facts: HealthFacts) -> [HealthDetail] {
        var lines = [HealthDetail("目前的輸入方式", facts.currentInputSource ?? "不明"),
                     HealthDetail("已加入的輸入方式", facts.enabledInputSources.isEmpty ? "沒有"
                         : facts.enabledInputSources.joined(separator: "、")),
                     .onOff("自動切換成文件的輸入方式", facts.perDocumentInputSource)]
        switch facts.secureInput {
        case .off: lines.append(.onOff("安全輸入", false))
        case .on(let owner): lines += [.onOff("安全輸入", true), HealthDetail("安全輸入開啟者", owner ?? "不明")]
        }
        return lines
    }

    private static func service(_ facts: HealthFacts) -> [HealthDetail] {
        guard let started = facts.started else { return [] }
        return [HealthDetail("啟動時間", HealthChecks.timestamp(started)),
                HealthDetail("已執行", duration(facts.now.timeIntervalSince(started)))]
    }

    private static func display(_ facts: HealthFacts) -> [HealthDetail] {
        let display = facts.display
        var lines: [HealthDetail] = []
        if let scale = display.scale { lines.append(HealthDetail("螢幕縮放", "\(Int((scale * 100).rounded()))%")) }
        let system = display.dark.map { $0 ? "深色" : "淺色" }
        switch display.panelAppearance {
        case .system?: lines.append(HealthDetail("主題", "跟隨系統" + (system.map { "（\($0)）" } ?? "")))
        case .light?: lines.append(HealthDetail("主題", "淺色"))
        case .dark?: lines.append(HealthDetail("主題", "深色"))
        case nil: if let system { lines.append(HealthDetail("系統外觀", system)) }
        }
        for (label, value) in [("螢幕閱讀器（VoiceOver）", display.voiceOver), ("減少動態效果", display.reduceMotion),
                               ("增加對比", display.increaseContrast), ("減少透明度", display.reduceTransparency)] {
            if let value { lines.append(.onOff(label, value)) }
        }
        return lines
    }

    // MARK: Text

    /// 「12.3 MB」, 「861 KB」.
    static func size(_ bytes: Int64) -> String {
        if bytes >= 1_000_000 { return String(format: "%.1f MB", Double(bytes) / 1_000_000) }
        if bytes >= 1000 { return "\(bytes / 1000) KB" }
        return "\(bytes) B"
    }

    /// 「3 天 4 小時」, 「4 小時 51 分」, 「12 分」, 「不到 1 分」.
    static func duration(_ seconds: TimeInterval) -> String {
        let minutes = max(0, Int(seconds / 60))
        let (days, hours) = (minutes / 1440, minutes / 60 % 24)
        if days > 0 { return "\(days) 天 \(hours) 小時" }
        if minutes >= 60 { return "\(hours) 小時 \(minutes % 60) 分" }
        return minutes == 0 ? "不到 1 分" : "\(minutes) 分"
    }

    /// A log message on one line, which cannot close the fenced block around it.
    static func oneLine(_ message: String) -> String {
        message.components(separatedBy: .newlines).joined(separator: " ").replacingOccurrences(of: "```", with: "'''")
    }

    /// The home directory as 「~」 wherever it starts a path, and the account name as 「<使用者>」 wherever it stands
    /// on its own (a file's owner, a path outside the home folder, a log message).
    static func anonymize(_ text: String, home: String, user: String) -> String {
        var text = text
        let boundary = "A-Za-z0-9_.\\-"
        if !home.isEmpty, home != "/" {
            let pattern = NSRegularExpression.escapedPattern(for: home) + "(?![\(boundary)])"
            text = text.replacingOccurrences(of: pattern, with: "~", options: .regularExpression)
        }
        if user.count >= 2 {
            let pattern = "(?<![\(boundary)])" + NSRegularExpression.escapedPattern(for: user) + "(?![\(boundary)])"
            text = text.replacingOccurrences(of: pattern, with: "<使用者>", options: .regularExpression)
        }
        return text
    }
}
