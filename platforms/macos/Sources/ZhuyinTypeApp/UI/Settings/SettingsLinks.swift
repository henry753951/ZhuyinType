import AppKit
import SwiftUI

/// Every place a link in the settings window leads to, in one list (docs/product/SETTINGS_IA.md §4.2, §4.3), the rows
/// that moved in 0.7.1 (§4.4), and the self-test that draws every page and checks each place is a row it drew.
enum SettingsLinks {
    /// 介紹's 「目前的設定」.
    enum Summary {
        static let space = SettingsPlace(.typing, row: SettingsRow.space)
        static let punctuation = SettingsPlace(.punctuation, row: SettingsRow.punctuationKeys)
        static let typo = SettingsPlace(.corrections, row: SettingsRow.typo)
        static let mode = SettingsPlace(.mode, row: SettingsRow.shiftTap)
    }

    /// The pages of 0.7.0 that rows moved out of.
    static let formerPanes: Set<SettingsPane> = [.typing, .choosing, .candidates]

    /// Rows of 0.7.0's 打字, 選字 and 視窗與提示 that are on another page now, and that page.
    static let movedRows: [String: SettingsPane] = {
        var rows: [String: SettingsPane] = [:]
        let moved: [(SettingsPane, [String])] = [
            (.punctuation, [SettingsRow.punctuationKeys, SettingsRow.symbolPicker, SettingsRow.controlPunctuation,
                            SettingsRow.optionSymbols, ShortcutCommand.togglePunctuationWidth.keysRow,
                            ShortcutCommand.symbolPicker.keysRow]),
            (.mode, [SettingsRow.shiftTap, SettingsRow.shiftSide, SettingsRow.capsLock, SettingsRow.startInEnglish,
                     SettingsRow.modeToast, ShortcutCommand.quickSettings.keysRow]),
            (.corrections, [SettingsRow.typo, SettingsRow.otherReadings, SettingsRow.corrections, SettingsRow.showCorrections,
                            SettingsRow.undoCorrection, SettingsRow.fuzzy, SettingsRow.typeZhuyin, SettingsRow.deleteAfterCommit,
                            ShortcutCommand.correctionsList.keysRow, ShortcutCommand.typeZhuyin.keysRow]),
            (.words, [SettingsRow.selectPhrase] + [ShortcutCommand.selectLeft, .selectRight, .addSelectedText, .blockCandidate,
                                                   .forgetCandidate, .addCandidatePhrase].map(\.keysRow)),
            (.colors, [SettingsRow.appearance, SettingsRow.accentColor]),
        ]
        for (pane, anchors) in moved { anchors.forEach { rows[$0] = pane } }
        return rows
    }()

    /// Every link's place with what it is, for the settings `preferences` (the links that depend on settings).
    static func all(_ preferences: Preferences, model: SettingsModel) -> [(String, SettingsPlace)] {
        var links: [(String, SettingsPlace)] = [
            ("介紹 › 空白鍵", Summary.space), ("介紹 › 標點按法", Summary.punctuation),
            ("介紹 › 按錯鍵修正", Summary.typo), ("介紹 › 中英切換", Summary.mode),
            ("快速設定 › 標點全半形", ShortcutCommand.togglePunctuationWidth.place),
            ("快速設定 › 繁／簡", SettingsPlace(.choosing, row: SettingsRow.outputScript)),
            ("快速設定 › 暫停學習", SettingsPlace(.words, row: SettingsRow.pauseLearning)),
        ]
        links.append(("輸入法選單 › 回報問題…", Feedback.place))
        let keys = CurrentKeys(preferences)
        links += IntroFeature.all(keys, preferences: preferences).map { ("介紹 › 功能 › \($0.title)", $0.place) }
        links += KeySection.allCases.map { ("「\($0.title)」的頁尾 › 快捷鍵總覽", .keysSection($0)) }
        let available = { (command: ShortcutCommand) in command != .symbolPicker || SymbolSources.isAvailable }
        links += KeyFeature.allCases.filter { $0 != .symbolPicker || SymbolSources.isAvailable }
            .map { ("功能 \($0.rawValue)", $0.place) }
        for command in ShortcutCommand.allCases where available(command) {
            links.append(("快捷鍵 \(command.rawValue)", command.place))
            links.append(("總覽 \(command.rawValue)", command.overviewPlace))
        }
        let overview = KeysPane(preferences: preferences, model: model)
        for section in KeySection.allCases {
            links += overview.items(section, keys).compactMap { item in item.home.map { ("總覽的列 \(item.id)", $0) } }
        }
        // Features made unavailable by another setting link to it.
        let unavailable = Preferences(defaults: MemorySettingsStorage())
        unavailable.showsZhuyinHint = false
        unavailable.typoTolerance = .off
        links += KeyFeature.allCases.compactMap { feature in
            unavailable.availability(feature).map { ("「\(feature.rawValue)」無法使用 › \($0.linkTitle)", $0.place) }
        }
        // Registry rows: the anchor each registered setting is drawn with.
        links += SettingsRegistry.all.filter { $0.place.row != nil && ($0.id != "keys.symbolPicker" || SymbolSources.isAvailable)
            && ($0.id != "feature.symbolPicker" || SymbolSources.isAvailable) }.map { ("設定 \($0.id)", $0.place) }
        // Places of 0.7.0 resolved to their rows now.
        links += [SettingsPlace(.typing, row: SettingsRow.shiftTap), SettingsPlace(.choosing, row: SettingsRow.typo),
                  SettingsPlace(.candidates, row: SettingsRow.modeToast), SettingsPlace(.candidates, row: SettingsRow.appearance),
                  SettingsPlace(.typing, row: SettingsRow.punctuationKeys), SettingsPlace(.choosing, row: SettingsRow.selectPhrase),
                  SettingsPlace(.choosing, row: ShortcutCommand.correctionsList.keysRow)].map { ("0.7.0 \($0.pane.rawValue)#\($0.row ?? "")", $0.resolved) }
        return links
    }
}

/// The links self-test (`--self-test`): draws every page and sub-page offscreen, then checks that every place a link
/// leads to is a row or section that page drew, that every command has its 「快捷鍵」 row on its page and on the
/// overview, that old places resolve, and that following a link focuses its row.
enum SettingsLinksSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }

        test("settings links: every link leads to a row the page draws") {
            _ = NSApplication.shared
            let saved = (Preferences.shared, SettingsWindowController.isPreview, DemoPinning.showsFixedKeys)
            Preferences.shared = Preferences(defaults: MemorySettingsStorage())
            SettingsWindowController.isPreview = true
            DemoPinning.showsFixedKeys = true
            defer {
                (Preferences.shared, SettingsWindowController.isPreview, DemoPinning.showsFixedKeys) = saved
                SettingsWindowController.shared.model.subpage = nil
            }
            let model = SettingsWindowController.shared.model
            RowSections.forgetDrawn()
            var pages: [(SettingsPane, SettingsSubpage?)] = SettingsPane.allCases.map { ($0, nil) }
            pages += SettingsSubpage.allCases.map { ($0.pane, $0) }
            for (pane, subpage) in pages {
                model.subpage = subpage
                let hosting = NSHostingView(rootView: SettingsPaneView(pane: pane, model: model))
                hosting.frame = NSRect(x: 0, y: 0, width: 860, height: 4000)
                let window = HeadlessWindow(size: hosting.frame.size)
                window.contentView = hosting
                window.orderFrontRegardless()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.15))
                hosting.layoutSubtreeIfNeeded()
                hosting.display()
                window.orderOut(nil)
                window.contentView = nil
                window.close()
            }
            model.subpage = nil
            var missing: [String] = []
            for (name, place) in SettingsLinks.all(Preferences.shared, model: model) {
                let page = RowSections.pageKey(place.pane, place.subpage)
                guard RowSections.drawn[page] != nil else { missing.append("\(name): page \(page) drew nothing"); continue }
                if let row = place.row, RowSections.drawn[page]?.contains(row) != true {
                    missing.append("\(name): \(page)#\(row)")
                }
            }
            try expect(missing.isEmpty, "links to rows that are not drawn: " + missing.joined(separator: "; "))
        }

        test("settings links: old places resolve to the page the row moved to") {
            try expect(SettingsPlace(.typing, row: SettingsRow.shiftTap).resolved == SettingsPlace(.mode, row: SettingsRow.shiftTap),
                       "打字#shiftTap")
            try expect(SettingsPlace(.choosing, row: SettingsRow.typo).resolved.pane == .corrections, "選字#typo")
            try expect(SettingsPlace(.candidates, row: SettingsRow.accentColor).resolved.pane == .colors, "視窗與提示#accentColor")
            try expect(SettingsPlace(.choosing, subpage: .fuzzy).resolved == SettingsPlace(.corrections, subpage: .fuzzy),
                       "選字 › 模糊音")
            try expect(SettingsPlace(.keys, row: ShortcutCommand.typeZhuyin.keysRow).resolved.pane == .keys,
                       "a row of the overview stayed on the overview")
            try expect(SettingsPane.earlier["general"] == .typing && SettingsPane(rawValue: "about") == .about,
                       "remembered pages of earlier versions")
        }

        test("settings links: following a link shows its page, scrolls and focuses its row") {
            _ = NSApplication.shared
            let saved = (Preferences.shared, SettingsWindowController.isPreview)
            Preferences.shared = Preferences(defaults: MemorySettingsStorage())
            SettingsWindowController.isPreview = true
            let controller = SettingsWindowController.shared
            let window = controller.makeWindow(pane: .intro)
            window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
            window.alphaValue = 0
            window.orderFrontRegardless()
            defer {
                controller.discardWindow()
                (Preferences.shared, SettingsWindowController.isPreview) = saved
            }
            for place in [ShortcutCommand.correctionsList.overviewPlace, ShortcutCommand.correctionsList.place,
                          SettingsPlace(.corrections, subpage: .fuzzy), SettingsPlace.keysSection(.words),
                          SettingsPlace(.keys, row: "row-cursor")] {
                controller.model.focusedRow = nil
                controller.open(place)
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.8))
                try expect(controller.isShowing(place.pane) && controller.model.subpage == place.subpage,
                           "\(place) did not show its page")
                if place.subpage == nil {
                    try expect(controller.model.focusedRow == place.row,
                               "\(place.row ?? "") did not take the focus (\(controller.model.focusedRow ?? "none"))")
                }
            }
        }
    }
}
