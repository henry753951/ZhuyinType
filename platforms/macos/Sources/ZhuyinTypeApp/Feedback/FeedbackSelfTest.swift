import AppKit
import SwiftUI

/// The self-test of 更新內容 (`ReleaseNotes`) and 意見與問題 (`Feedback`), run by `--self-test`.
enum FeedbackSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    private static let json = """
        {"releases": [
          {"version": "0.8.0", "date": "d", "sections": [
            {"title": "新功能", "items": [{"title": "甲", "text": "說明。", "platforms": []}]},
            {"title": "修正", "items": [{"title": null, "text": "丙壞了。現在好了。", "platforms": ["macos"]},
                                        {"title": null, "text": "丁壞了。", "platforms": ["windows"]}]}]},
          {"version": "0.7.10", "date": "d", "sections": [
            {"title": "改進", "items": [{"title": null, "text": "乙更快。", "platforms": []},
                                        {"title": null, "text": "戊更快。", "platforms": ["macos", "windows"]}]}]},
          {"version": "0.7.9", "date": "d", "sections": [
            {"title": "修正", "items": [{"title": null, "text": "己壞了。", "platforms": ["linux"]}]}]}]}
        """

    static func run(test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }
        let notes = ReleaseNotes.parse(Data(json.utf8))
        func between(_ previous: String?, _ current: String) -> [ReleaseNotes] {
            ReleaseNotes.between(notes, previous: previous, current: current)
        }

        test("release notes: this system's changes, every version since the last one seen") {
            try expect(notes.map(\.version) == ["0.8.0", "0.7.10", "0.7.9"], "\(notes.map(\.version))")
            try expect(notes[0].sections[1].items.count == 1 && notes[2].sections.isEmpty, "another system's changes were kept")
            try expect(between("0.7.9", "0.8.0").map(\.version) == ["0.8.0", "0.7.10"], "over two versions")
            // 0.7.9 changed nothing on macOS.
            try expect(between("0.7.8", "0.8.0").map(\.version) == ["0.8.0", "0.7.10"], "an empty version is listed")
            try expect(between(nil, "0.8.0").map(\.version) == ["0.8.0"], "from an unknown version")
            try expect(between("0.8.0", "0.7.10").map(\.version) == ["0.7.10"], "an older version put back")
            let one = ReleaseNotes.announcement(between("0.7.10", "0.8.0"), current: "0.8.0")
            try expect(one.lines == ["甲", "修正：丙壞了"], "\(one.lines)")
            let two = ReleaseNotes.announcement(between("0.7.9", "0.8.0"), current: "0.8.0")
            try expect(two.lines == ["甲", "乙更快", "戊更快", "2 個版本，另有 1 項"], "\(two.lines)")
            let long = ReleaseNotes.Item(title: nil, text: String(repeating: "長", count: 80), platforms: [])
            try expect(long.headline.count == ReleaseNotes.headlineLimit && long.headline.hasSuffix("…"), long.headline)
        }

        test("update notice: never after a new install, once per version, not when turned off") {
            let progress = ReleaseNotesProgress(defaults: MemorySettingsStorage())
            try expect(progress.start(current: "0.8.0", usedBefore: false, wanted: true) == .remember, "a new install")
            try expect(progress.start(current: "0.8.0", usedBefore: true, wanted: true) == .announce(previous: nil), "from 0.7.6")
            progress.remember("0.7.9")
            // Not shown yet (the user never typed): asked again at the next launch, from the same version.
            for _ in 0..<2 {
                try expect(progress.start(current: "0.8.0", usedBefore: true, wanted: true) == .announce(previous: "0.7.9"), "update")
            }
            try expect(progress.start(current: "0.8.0", usedBefore: true, wanted: false) == .remember, "turned off")
            progress.remember("0.8.0")
            try expect(progress.start(current: "0.8.0", usedBefore: true, wanted: true) == .nothing, "a second time")
            try expect(progress.shown(notes, current: "0.8.0").map(\.version) == ["0.8.0", "0.7.10"], "更新內容 after it")
            try expect(progress.start(current: "0.7.9", usedBefore: true, wanted: true) == .remember, "an older version put back")
        }

        test("update notice: its panel cannot become key, and the headless modes never show it") {
            let view = UpdateNoticeView(announcement: ReleaseNotes.announcement(between("0.7.9", "0.8.0"), current: "0.8.0")) { _ in }
            let hosting = FirstMouseHostingView(rootView: view)
            let panel = InputPanel(content: InputSurface(content: hosting))
            try expect(!panel.canBecomeKey && !panel.canBecomeMain, "the panel can take the focus")
            try expect(panel.styleMask.contains(.nonactivatingPanel), "a click activates the app")
            try expect(hosting.acceptsFirstMouse(for: nil), "the first click does nothing")
            UpdateNotice.shared.start()
            UpdateNotice.shared.typed()
        }

        test("feedback: the address carries the version and the system only") {
            let url = Feedback.url(.uninstall(reason: "改用 A&B"), version: "0.8.0", system: "macOS 27.0（26A1），arm64")
            let parts = URLComponents(url: url, resolvingAgainstBaseURL: false)
            try expect(url.absoluteString.hasPrefix(Feedback.newIssue + "?"), url.absoluteString)
            try expect(parts?.queryItems == [
                URLQueryItem(name: "template", value: "uninstall.yml"), URLQueryItem(name: "version", value: "0.8.0"),
                URLQueryItem(name: "system", value: "macOS 27.0（26A1），arm64"), URLQueryItem(name: "reason", value: "改用 A&B"),
            ], url.absoluteString)
            let home = FileManager.default.homeDirectoryForCurrentUser.path
            for topic in [Feedback.Topic.conversion, .crash, .other, .suggestion] {
                let text = Feedback.url(topic, version: "0.8.0", system: Feedback.system).query(percentEncoded: false) ?? ""
                try expect(!text.contains(home) && !text.contains(NSUserName()), text)
            }
        }
    }
}
