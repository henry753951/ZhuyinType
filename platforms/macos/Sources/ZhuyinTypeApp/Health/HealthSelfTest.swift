import AppKit

/// 檢查輸入法's self-tests, run by `--self-test` with the ControllerCore ones: every status of every check from
/// made-up facts, the summary, the report's layout and what it leaves out, and one read of this Mac's facts.
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

        test("health: secure input names its app on the page only") {
            let on = facts { $0.secureInput = .on(owner: "密碼管理員") }
            let secure = try check(.secureInput, on)
            try expect(secure.status == .warning && secure.explanation.contains("「密碼管理員」"), "on: \(secure)")
            try expect(!secure.reportLine.contains("密碼管理員"), "the report names the app: \(secure.reportLine)")
            let report = HealthReport.text(facts: on, checks: HealthChecks.evaluate(on), errors: [])
            try expect(!report.contains("密碼管理員"), "the report names the app")
            let unknown = try check(.secureInput, facts { $0.secureInput = .on(owner: nil) })
            try expect(unknown.status == .warning && unknown.explanation == unknown.reportExplanation, "unknown owner: \(unknown)")
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

        test("health: the report's layout, and nothing but the facts in it") {
            let facts = facts { $0.secureInput = .on(owner: "某個 App") }
            let checks = HealthChecks.evaluate(facts)
            let errors = [HealthReport.LogLine(date: facts.now,
                                               message: "Engine load failed: /Users/sample/Library/x\nsecond line")]
            let report = HealthReport.text(facts: facts, checks: checks, errors: errors)
            let lines = report.components(separatedBy: "\n")
            let header = ["ZhuyinType 問題回報", "產生時間：\(HealthChecks.timestamp(facts.now))", "版本：0.6.1（建置 123）",
                          "系統：macOS 26.1（25B78）", "架構：arm64", "", "檢查結果"]
            try expect(Array(lines.prefix(header.count)) == header, "header: \(lines.prefix(header.count))")
            try expect(Array(lines.dropFirst(header.count).prefix(checks.count)) == checks.map(\.reportLine), "check lines")
            try expect(lines.contains("[注意] 安全輸入已開啟：" + (checks[2].reportExplanation ?? "")), "secure line")
            let tail = Array(lines.dropFirst(header.count + checks.count))
            try expect(tail.count == 4 && tail[0] == "" && tail[1] == "最近的錯誤" && tail[3] == "", "tail: \(tail)")
            try expect(tail[2].hasSuffix(" Engine load failed: ~/Library/x second line"), "error line: \(tail[2])")
            try expect(!report.contains("/Users/sample") && !report.contains("某個 App"), "home or app name in the report")
            let none = HealthReport.text(facts: facts, checks: checks, errors: [])
            try expect(none.hasSuffix("最近的錯誤\n（沒有）\n"), "no errors")
            let unread = HealthReport.text(facts: facts, checks: checks, errors: nil)
            try expect(unread.hasSuffix("最近的錯誤\n（無法讀取）\n"), "log unreadable")
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
            let report = HealthReport.text(facts: read, checks: HealthChecks.evaluate(read), errors: errors)
            try expect(!report.contains(read.home + "/"), "the home directory in the report")
        }
    }
}
