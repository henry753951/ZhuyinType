import AppKit
import SwiftUI

/// `ZhuyinType --render-ui DIR --only practice`: the practice field in several states, light and dark, each reached
/// by synthetic keys handed to the field (never posted to the system) on the bundled lexicon, in a headless window.
enum PracticeRender {
    private struct Missing: LocalizedError {
        var errorDescription: String? { "practice render: no lexicon in the app bundle" }
    }

    /// One state: the task, the settings and the keys that lead to it.
    private struct State {
        var name: String
        var task: PracticeTask
        var settings = SettingsRegistry.defaults
        var steps: (PracticeFieldModel) -> Void
    }

    static func render(to directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let bundled = Bundle.main.resourceURL.flatMap { root -> URL? in
            let manager = FileManager.default
            guard manager.fileExists(atPath: root.appendingPathComponent("lexicon.bin").path)
                    || manager.fileExists(atPath: root.appendingPathComponent("lexicon.tsv").path) else { return nil }
            return root.appendingPathComponent("lexicon.tsv")
        }
        guard let lexicon = bundled ?? EngineHost.lexiconURL() else { throw Missing() }
        // Animation off: every picture is the state itself, not a moment of a fade.
        let still = SettingsRegistry.defaults.with { $0.panelAnimation = false; $0.learningEnabled = false }
        let sentence = PracticeTask.sentence("今天天氣很好")
        let states: [State] = [
            State(name: "start", task: sentence, settings: still) { _ in },
            State(name: "composing", task: sentence, settings: still) { type("rup wu0 wu0 fu4cp3cl3", $0) },
            State(name: "candidates", task: .action("打 ㄇㄧㄥˊ，翻到下一頁，選一個字", .openCandidates, .turnPage, .pick(nil)),
                  settings: still) { type("au/6", $0); press("down", $0) },
            State(name: "page", task: .action("打 ㄇㄧㄥˊ，翻到下一頁，選一個字", .openCandidates, .turnPage, .pick(nil)),
                  settings: still) { type("au/6", $0); press("down", $0); press("tab", $0) },
            State(name: "horizontal", task: .action("打 ㄇㄧㄥˊ，翻到下一頁，選一個字", .openCandidates, .turnPage, .pick(nil)),
                  settings: still.with { $0.candidateLayout = .horizontal }) { type("au/6", $0); press("down", $0) },
            State(name: "above", task: sentence, settings: still.with { $0.candidatePlacement = .above; $0.candidateDensity = .compact }) {
                type("rup wu0 wu0 fu4cp3cl3", $0); press("down", $0)
            },
            State(name: "phrase", task: .userPhrase("王曉明"), settings: still) {
                type("j;6vul3au/6", $0)
                for _ in 0..<3 { press("left", $0, .shift) }
            },
            State(name: "english", task: .goals("切換到英文打「OK」，再切回中文打「好」", hint: nil,
                                                      [.text("OK好"), .switchMode(times: 2)]), settings: still) {
                $0.view.textView.flagsChanged(with: PracticeKeys.flags(56, down: true))
                $0.view.textView.flagsChanged(with: PracticeKeys.flags(56, down: false))
                type("OK", $0)
                $0.view.pinBadge()
            },
            State(name: "done", task: sentence, settings: still) {
                type("rup wu0 wu0 fu4cp3cl3", $0)
                press("return", $0)
            },
        ]
        let data = try PracticeUserData.make(lexiconURL: lexicon, reranker: nil, characterModel: nil)
        for (suffix, appearance) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
            NSApp.appearance = NSAppearance(named: appearance)
            for state in states {
                try autoreleasepool {
                    let model = PracticeFieldModel(settings: state.settings, task: state.task, data: data)
                    model.view.textView.pinnedFocus = true
                    model.view.frame.size = NSSize(width: 560, height: model.height)
                    model.view.layoutSubtreeIfNeeded()
                    state.steps(model)
                    model.view.textView.focusChanged()
                    try UIPreviewRenderer.capture(swiftUI: PracticeField(model: model).padding(20)
                                                    .background(Color(nsColor: .windowBackgroundColor)),
                                                  width: 600, to: directory.appendingPathComponent("\(state.name)-\(suffix).png"),
                                                  settle: 0.2)
                    // Each state starts from empty data.
                    model.session.endSession()
                }
            }
        }
    }

    private static func type(_ keys: String, _ model: PracticeFieldModel) {
        for key in keys { model.view.textView.keyDown(with: PracticeKeys.keyDown(key)) }
    }

    private static func press(_ name: String, _ model: PracticeFieldModel, _ modifiers: NSEvent.ModifierFlags = []) {
        model.view.textView.keyDown(with: PracticeKeys.keyDown(named: name, modifiers))
    }
}
