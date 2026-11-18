import AppKit

/// 檢查輸入法's self-tests, run by `--self-test` with the ControllerCore ones: every status and the details of every
/// check from made-up facts, the summary, the report's Markdown layout and what it leaves out (a populated profile's
/// account name, phrases, learned words and app list), 改過的設定, and one read of this Mac's facts.
enum HealthSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }
        let sample = HealthFacts.sample()
        func facts(_ change: (inout HealthFacts) -> Void) -> HealthFacts {
            var facts = sample
            change(&facts)
            return facts
        }
        func check(_ kind: HealthCheck.Kind, _ facts: HealthFacts) throws -> HealthCheck {
            guard let check = HealthChecks.evaluate(facts).first(where: { $0.kind == kind }) else {
                throw Failure(description: "no \(kind) check")
            }
            return check
        }

        test("health: everything in place is 全部正常, one check of each kind in the page's order") {
            let checks = HealthChecks.evaluate(sample)
            try expect(checks.map(\.kind) == HealthCheck.Kind.allCases, "order: \(checks.map(\.kind))")
            try expect(checks.allSatisfy { $0.status == .ok }, "not ok: \(checks.filter { $0.status != .ok }.map(\.title))")
            try expect(HealthChecks.summary(checks) == "全部正常", HealthChecks.summary(checks))
        }

        test("health: the input source — not registered, not added, not selectable") {
            let missing = try check(.inputSource, facts { $0.inputSource = .notRegistered })
            try expect(missing.status == .error, "not registered: \(missing.status)")
            let off = try check(.inputSource, facts { $0.inputSource = .registered(enabled: false, selectable: false) })
            try expect(off.status == .warning && off.actions == [.enableInputSource, .openKeyboardSettings], "not enabled: \(off)")
            let stuck = try check(.inputSource, facts { $0.inputSource = .registered(enabled: true, selectable: false) })
            try expect(stuck.status == .warning && !stuck.actions.contains(.enableInputSource), "not selectable: \(stuck)")
        }

        test("health: the running copy — outside the folders, two installed, others elsewhere, the Trash ignored") {
            let build = URL(fileURLWithPath: "/Users/sample/Developer/ZhuyinType/dist/ZhuyinType.app")
            let system = URL(fileURLWithPath: "/Library/Input Methods/ZhuyinType.app")
            let outside = try check(.installedCopy, facts { $0.running = build })
            try expect(outside.status == .warning && outside.explanation.contains("~/Developer/ZhuyinType/dist/ZhuyinType.app"),
                       "running outside: \(outside)")
            let two = try check(.installedCopy, facts { $0.copies.append(system) })
            try expect(two.status == .warning && two.actions == [.reveal(system)], "two installed: \(two)")
            let elsewhere = try check(.installedCopy, facts { $0.copies.append(build) })
            try expect(elsewhere.status == .info && elsewhere.explanation.contains("~/Developer"), "elsewhere: \(elsewhere)")
            let several = try check(.installedCopy, facts { $0.copies += [build, build.appendingPathComponent("x"), build] })
            try expect(several.explanation.contains("另有 2 份"), "duplicates counted: \(several.explanation)")
            let trash = try check(.installedCopy, facts {
                $0.copies.append(URL(fileURLWithPath: "/Users/sample/.Trash/ZhuyinType.app"))
            })
            try expect(trash.status == .ok, "the Trash counted: \(trash)")
            for check in [outside, two, elsewhere, trash] {
                try expect(!check.explanation.contains("/Users/sample"), "home not abbreviated: \(check.explanation)")
            }
        }

        test("health: secure input — the line stays general, its details and 輸入法 name the app") {
            let on = facts { $0.secureInput = .on(owner: "密碼管理員") }
            let secure = try check(.secureInput, on)
            try expect(secure.status == .warning && secure.explanation.contains("「密碼管理員」"), "on: \(secure)")
            try expect(!secure.reportLine.contains("密碼管理員"), "the line names the app: \(secure.reportLine)")
            try expect(secure.details == [HealthDetail("狀態", "開啟"), HealthDetail("開啟者", "密碼管理員")],
                       "details: \(secure.details)")
            let report = HealthReport.text(facts: on, checks: HealthChecks.evaluate(on), errors: [])
            try expect(report.contains("\n  - 開啟者：密碼管理員\n") && report.contains("\n- 安全輸入開啟者：密碼管理員\n"),
                       "the report leaves out the owner")
            let unknown = try check(.secureInput, facts { $0.secureInput = .on(owner: nil) })
            try expect(unknown.status == .warning && unknown.explanation == unknown.reportExplanation, "unknown owner: \(unknown)")
            try expect(unknown.details.last == HealthDetail("開啟者", "不明"), "unknown owner: \(unknown.details)")
            let off = try check(.secureInput, sample)
            try expect(off.details == [HealthDetail("狀態", "關閉")], "off: \(off.details)")
        }

        test("health: every check says what it looked at") {
            try expect(HealthChecks.evaluate(sample).allSatisfy { !$0.details.isEmpty }, "a check without details")
            let input = try check(.inputSource, sample)
            try expect(input.details == [HealthDetail("輸入方式 ID", "org.zhuyintype.inputmethod.ZhuyinType.Zhuyin"),
                                         HealthDetail("已登記", "是"), HealthDetail("已加入", "是"), HealthDetail("可選用", "是")],
                       "input source: \(input.details)")
            let missing = try check(.inputSource, facts { $0.inputSource = .notRegistered })
            try expect(missing.details.count == 2 && missing.details.last == HealthDetail("已登記", "否"), "\(missing.details)")
            let other = URL(fileURLWithPath: "/Users/sample/Downloads/ZhuyinType.app")
            let copies = try check(.installedCopy, facts { $0.copies.append(other) })
            try expect(copies.details == [HealthDetail("位置", "~/Library/Input Methods/ZhuyinType.app"),
                                          HealthDetail("在輸入法資料夾中", "是"),
                                          HealthDetail("其他副本", "~/Downloads/ZhuyinType.app")], "copies: \(copies.details)")
            let alone = try check(.installedCopy, sample)
            try expect(alone.details.last == HealthDetail("其他副本", "沒有"), "no other copy: \(alone.details)")
            let data = try check(.dataFiles, facts { $0.missingModels = [.reranker] })
            try expect(data.details == [HealthDetail("資料夾", "~/Library/Input Methods/ZhuyinType.app/Contents/Resources"),
                                        HealthDetail("詞庫", "lexicon.bin"), HealthDetail("缺少", "選字模型")],
                       "data: \(data.details)")
            let per = try check(.perDocumentSource, sample)
            try expect(per.details == [HealthDetail("自動切換成文件的輸入方式", "關閉")], "per document: \(per.details)")
            let settings = try check(.settings, sample)
            try expect(settings.details.map(\.label) == ["檔案", "狀態", "改過的設定"] && settings.details[2].value == "2 項",
                       "settings: \(settings.details)")
            let version = try check(.version, sample)
            try expect(version.details.map(\.label) == ["自動檢查", "上次檢查", "已下載的更新"], "version: \(version.details)")
            let dev = try check(.version, facts { $0.updates = .unavailable })
            try expect(dev.details.last == HealthDetail("自動檢查", "不提供（沒有更新來源）"), "no feed: \(dev.details)")
        }

        test("health: 自動切換成文件的輸入方式 is explained, not a problem") {
            let per = try check(.perDocumentSource, facts { $0.perDocumentInputSource = true })
            try expect(per.status == .info && per.actions == [.openKeyboardSettings], "\(per)")
        }

        test("health: data files — no lexicon, an unreadable one, missing models") {
            let missing = try check(.dataFiles, facts { $0.lexicon = .missing })
            try expect(missing.status == .error, "missing: \(missing)")
            let file = URL(fileURLWithPath: "/Users/sample/Library/Application Support/ZhuyinType/Models/lexicon.bin")
            let unreadable = try check(.dataFiles, facts { $0.lexicon = .unreadable(file) })
            try expect(unreadable.status == .error && unreadable.explanation.hasPrefix("~/Library/Application Support"),
                       "unreadable: \(unreadable)")
            let models = try check(.dataFiles, facts { $0.missingModels = [.reranker, .characterModel] })
            try expect(models.status == .warning && models.explanation.contains("選字模型、單字模型"), "models: \(models)")
            // No lexicon outranks missing models.
            let both = try check(.dataFiles, facts { $0.lexicon = .missing; $0.missingModels = [.reranker] })
            try expect(both.status == .error, "both: \(both)")
        }

        test("health: settings — none saved, an unreadable file") {
            let none = try check(.settings, facts { $0.settings = .none })
            try expect(none.status == .ok, "none: \(none)")
            let file = URL(fileURLWithPath: "/Users/sample/Library/Preferences/org.zhuyintype.inputmethod.ZhuyinType.plist")
            let bad = try check(.settings, facts { $0.settings = .unreadable(file) })
            try expect(bad.status == .warning && bad.explanation.hasPrefix("~/Library/Preferences/"), "unreadable: \(bad)")
        }

        test("health: version and updates — no feed, downloaded, checks off, not checked lately") {
            let dev = try check(.version, facts { $0.updates = .unavailable; $0.version = nil })
            try expect(dev.status == .info && dev.title == "版本 開發版", "no feed: \(dev)")
            let pending = try check(.version, facts {
                $0.updates = .available(automaticChecks: true, lastCheck: $0.now, pending: "0.6.2")
            })
            try expect(pending.status == .info && pending.explanation.contains("0.6.2"), "pending: \(pending)")
            let off = try check(.version, facts { $0.updates = .available(automaticChecks: false, lastCheck: nil, pending: nil) })
            try expect(off.status == .info && off.actions == [.checkForUpdates], "off: \(off)")
            let stale = try check(.version, facts {
                $0.updates = .available(automaticChecks: true, lastCheck: $0.now.addingTimeInterval(-8 * 24 * 3600), pending: nil)
            })
            try expect(stale.status == .info && stale.explanation.hasPrefix("上次檢查是"), "stale: \(stale)")
            let never = try check(.version, facts { $0.updates = .available(automaticChecks: true, lastCheck: nil, pending: nil) })
            try expect(never.status == .info && never.explanation.hasPrefix("還沒有檢查過"), "never: \(never)")
            let current = try check(.version, sample)
            try expect(current.title == "版本 0.6.1（建置 123）", "title: \(current.title)")
        }

        test("health: the summary counts warnings and errors, not 說明") {
            let checks = HealthChecks.evaluate(facts {
                $0.secureInput = .on(owner: nil)
                $0.lexicon = .missing
                $0.perDocumentInputSource = true
            })
            try expect(HealthChecks.summary(checks) == "有 2 個項目需要處理", HealthChecks.summary(checks))
            try expect(HealthStatus.allCases.filter(\.needsAttention) == [.warning, .error], "needsAttention")
        }

        test("health: the report is Markdown: the header list, sections, indented details, the fenced errors") {
            let on = facts { $0.secureInput = .on(owner: "某個 App") }
            let checks = HealthChecks.evaluate(on)
            let errors = [HealthReport.LogLine(date: on.now, message: "Engine load failed: /Users/sample/Library/x\nsecond ```")]
            let report = HealthReport.text(facts: on, checks: checks, errors: errors)
            let lines = report.components(separatedBy: "\n")
            let header = ["## ZhuyinType 問題回報", "", "- 產生時間：\(HealthChecks.timestamp(on.now))", "- 結果：有 1 個項目需要處理",
                          "- 版本：0.6.1（建置 123）", "- 系統：macOS 26.1（25B78）", "- 架構：arm64", "", "### 檢查結果", ""]
            try expect(Array(lines.prefix(header.count)) == header, "header: \(lines.prefix(header.count))")
            let checkLines = checks.flatMap { ["- " + $0.reportLine] + $0.details.map { "  - \($0.label)：\($0.value)" } }
            try expect(Array(lines.dropFirst(header.count).prefix(checkLines.count)) == checkLines, "check lines")
            try expect(lines.contains("- [注意] 安全輸入已開啟：" + (checks[2].reportExplanation ?? "")), "secure line")
            let headings = lines.filter { $0.hasPrefix("#") }
            try expect(headings == ["## ZhuyinType 問題回報", "### 檢查結果", "### 版本與資料", "### 安裝", "### 輸入法", "### 服務",
                                    "### 更新", "### 改過的設定", "### 顯示與輔助功能", "### 最近的錯誤"], "headings: \(headings)")
            for (index, line) in lines.enumerated() where line.hasPrefix("#") {
                try expect(index == 0 || lines[index - 1].isEmpty, "no blank line before \(line)")
                try expect(lines.indices.contains(index + 1) && lines[index + 1].isEmpty, "no blank line after \(line)")
            }
            try expect(report.hasSuffix("```\n") && !report.contains("\n\n\n"), "blank lines or the end")
            let fence = Array(lines.suffix(4))
            try expect(fence[0] == "```text" && fence[2] == "```" && fence[3] == "", "fence: \(fence)")
            try expect(fence[1].hasSuffix(" Engine load failed: ~/Library/x second '''"), "error line: \(fence[1])")
            for line in lines.dropLast(4) where !line.isEmpty && !line.hasPrefix("#") {
                try expect((line.hasPrefix("- ") || line.hasPrefix("  - ")) && line.contains("："), "not a list item: \(line)")
            }
            let expected = [
                "- 資料夾：~/Library/Input Methods/ZhuyinType.app/Contents/Resources", "- 詞庫：lexicon.bin，17.6 MB",
                "- 其他讀法表：channel.bin，861 KB", "- 選字模型：reranker/，1.0 MB",
                "- 隨附的資料版本：lexicon data-7（fb263aaa0525）、lm data-8（68d981d14b78）、lm_chat data-2（a93f6891b879）、"
                    + "reranker data-8（2f2796febc6a）、v3 data-8（82ba074b7781）",
                "- 模型載入：詞庫與語言模型 118 ms、選字模型 81 ms、單字模型 64 ms",
                "- 位置：~/Library/Input Methods/ZhuyinType.app", "- 擁有者：<使用者>", "- 權限：drwxr-xr-x",
                "- 隔離標記（com.apple.quarantine）：否", "- 設定檔：~/Library/Preferences/org.zhuyintype.inputmethod.ZhuyinType.plist",
                "- 使用者資料：~/Library/Application Support/ZhuyinType", "- 目前的輸入方式：org.zhuyintype.inputmethod.ZhuyinType.Zhuyin",
                "- 自動切換成文件的輸入方式：關閉", "- 安全輸入：開啟", "- 安全輸入開啟者：某個 App", "- 已執行：4 小時 51 分",
                "- 自動檢查：開啟", "- 已下載的更新：沒有", "- candidateFontSize：20", "- keyBindings：（已設定）", "- 螢幕縮放：200%",
                "- 主題：跟隨系統（淺色）", "- 螢幕閱讀器（VoiceOver）：關閉", "- 減少透明度：關閉",
            ]
            for line in expected { try expect(lines.contains(line), "missing: \(line)") }
            try expect(!report.contains("/Users/sample") && !report.contains("sample"), "home or user name in the report")
            let none = HealthReport.text(facts: on, checks: checks, errors: [])
            try expect(none.hasSuffix("\n\n### 最近的錯誤\n\n（沒有）\n") && !none.hasSuffix("\n\n（沒有）\n\n"), "no errors")
            let unread = HealthReport.text(facts: on, checks: checks, errors: nil)
            try expect(unread.hasSuffix("\n\n### 最近的錯誤\n\n（無法讀取）\n"), "log unreadable")
            // A section without lines is left out.
            let bare = facts {
                $0.changedSettings = []
                $0.started = nil
                $0.display = HealthFacts.Display()
            }
            let short = HealthReport.text(facts: bare, checks: HealthChecks.evaluate(bare), errors: [])
            for title in ["### 改過的設定", "### 服務", "### 顯示與輔助功能"] {
                try expect(!short.contains(title), "an empty section: \(title)")
            }
            try expect(!short.contains("\n\n\n"), "blank lines where a section was left out")
        }

        test("health: a populated profile leaks no account name, phrase, learned word or app list") {
            var amy = HealthFacts.sample(user: "amy")
            amy.secureInput = .on(owner: "鑰匙圈存取")
            amy.copies.append(URL(fileURLWithPath: "/Volumes/Backup/amy/ZhuyinType.app"))
            amy.dataFolder = URL(fileURLWithPath: "/Users/amy/Library/Application Support/ZhuyinType/Models", isDirectory: true)
            let phrases = ["王小明", "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ", "我的秘密自訂詞"]
            let learned = ["今天天氣", "阿美的貓"]
            let apps = ["com.amy.SecretDiary", "com.example.BankApp"]
            let stored: [String: Any] = [
                Preferences.Key.popupCompositionApps: apps,
                Preferences.Key.keyBindings: Data("{\"commitText\":[\"\(phrases[0])\"]}".utf8),
                Preferences.Key.disabledKeyFeatures: ["symbols"],
                Preferences.Key.candidateFontSize: 22.0,
                Preferences.Key.candidatesPerPage: 9,
                Preferences.Key.learningEnabled: false,
                Preferences.Key.showsZhuyinHint: true,
                Preferences.Key.typoTolerance: "aggressive",
                Preferences.Key.selectionKeys: phrases[2],
                "futurePhraseList": phrases + learned,
                "futureNote": learned[1],
                "NSWindow Frame ZhuyinTypeSettings": "0 0 760 600",
                "SULastCheckTime": Date(),
                "settingsLastPane": "about",
            ]
            amy.changedSettings = HealthSettings.changes(stored)
            let errors = [HealthReport.LogLine(date: amy.now, message: "Engine load failed: /Users/amy/Library/x"),
                          HealthReport.LogLine(date: amy.now, message: "cannot open /Users/amy; owner amy, group staff")]
            let report = HealthReport.text(facts: amy, checks: HealthChecks.evaluate(amy), errors: errors)
            for secret in ["amy", "/Users/amy"] + phrases + learned + apps + ["SecretDiary", "BankApp", "symbols"] {
                try expect(!report.contains(secret), "the report contains 「\(secret)」")
            }
            let expected = [
                "- popupCompositionApps：（已設定）", "- keyBindings：（已設定）", "- disabledKeyFeatures：（已設定）",
                "- candidateFontSize：22", "- learningEnabled：關閉", "- typoTolerance：aggressive", "- selectionKeys：（已設定）",
                "- futurePhraseList：（已設定）", "- futureNote：（已設定）", "- 擁有者：<使用者>",
                "- 資料夾：~/Library/Application Support/ZhuyinType/Models",
                "  - 其他副本：/Volumes/Backup/<使用者>/ZhuyinType.app", "- 安全輸入開啟者：鑰匙圈存取",
            ]
            for line in expected { try expect(report.contains("\n" + line + "\n"), "missing: \(line)") }
            try expect(report.contains(" Engine load failed: ~/Library/x\n") && report.contains("cannot open ~; owner <使用者>,"),
                       "error lines: \(report.components(separatedBy: "```text").last ?? "")")
            for key in ["candidatesPerPage", "showsZhuyinHint", "NSWindow", "SULastCheckTime", "settingsLastPane"] {
                try expect(!report.contains("- \(key)"), "listed: \(key)")
            }
        }

        test("health: 改過的設定: defaults left out, closed sets written, everything else the key alone") {
            typealias Key = Preferences.Key
            let changes = HealthSettings.changes([
                Key.candidateLayout: "horizontal", Key.candidateLayout + "x": "horizontal", Key.panelAppearance: "system",
                Key.spaceAction: "commitAndSpace", Key.pageAtListEdge: false, Key.fuzzySounds: 5, Key.showModeToast: "yes",
                Key.candidateFontSize: 18, Key.keyBindingsVersion: 2, Key.accentColor: "plum",
            ])
            let expected = [HealthSettings.Change(key: "accentColor", value: nil),
                            HealthSettings.Change(key: "candidateLayout", value: "horizontal"),
                            HealthSettings.Change(key: "candidateLayoutx", value: nil),
                            HealthSettings.Change(key: "fuzzySounds", value: "5"),
                            HealthSettings.Change(key: "pageAtListEdge", value: "關閉"),
                            HealthSettings.Change(key: "showModeToast", value: nil)]
            try expect(changes == expected, "changes: \(changes)")
        }

        test("health: the home folder and the account name are replaced only where they stand on their own") {
            let text = HealthReport.anonymize("/Users/amy /Users/amy/x /Users/amyx amy amyx xamy amy.b <amy>",
                                              home: "/Users/amy", user: "amy")
            try expect(text == "~ ~/x /Users/amyx <使用者> amyx xamy amy.b <<使用者>>", text)
            try expect(HealthReport.anonymize("a b", home: "/", user: "a") == "a b", "a root home or a one-letter name")
            try expect(HealthReport.size(17_594_752) == "17.6 MB" && HealthReport.size(861_224) == "861 KB"
                       && HealthReport.size(29) == "29 B", "sizes")
            try expect(HealthReport.duration(30) == "不到 1 分" && HealthReport.duration(12 * 60) == "12 分"
                       && HealthReport.duration(4 * 3600 + 51 * 60) == "4 小時 51 分"
                       && HealthReport.duration(3 * 86400 + 4 * 3600 + 59) == "3 天 4 小時", "durations")
        }

        test("health: paths keep a directory that only starts like the home one") {
            try expect(HealthChecks.abbreviate("/Users/sample", home: "/Users/sample") == "~", "home")
            try expect(HealthChecks.abbreviate("/Users/samples/x", home: "/Users/sample") == "/Users/samples/x", "prefix")
            try expect(HealthChecks.abbreviate("/Library/x", home: "/") == "/Library/x", "root home")
        }

        test("health: reading this Mac's facts and recent errors is quick and only reads") {
            let start = Date()
            let main = HealthFactsReader.readOnMain()
            var read: HealthFacts?
            var errors: [HealthReport.LogLine]?
            let done = DispatchSemaphore(value: 0)
            DispatchQueue.global().async {
                read = HealthFactsReader.read(completing: main)
                errors = HealthFactsReader.recentErrors()
                done.signal()
            }
            try expect(done.wait(timeout: .now() + 3) == .success, "the reader did not finish in 3 s")
            let seconds = Date().timeIntervalSince(start)
            try expect(seconds < 1.5, "took \(seconds) s")
            guard let read else { throw Failure(description: "no facts") }
            try expect(HealthChecks.evaluate(read).count == HealthCheck.Kind.allCases.count, "checks")
            try expect(read.dataItems.count == 6 && read.started != nil && !read.inputModeID.isEmpty
                       && read.preferencesFile != nil && read.app.permissions != nil, "facts not read: \(read)")
            let report = HealthReport.text(facts: read, checks: HealthChecks.evaluate(read), errors: errors)
            try expect(!report.contains(read.home + "/"), "the home directory in the report")
            try expect(read.user.count < 3 || !report.contains(read.user), "the account name in the report")
        }
    }
}
