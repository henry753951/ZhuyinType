import AppKit
import SwiftUI
import ZhuyinEngine

// 快捷鍵, in the manner of System Settings → Keyboard → Keyboard Shortcuts: every key grouped by feature, in the words
// of what it does.  Rebindable commands have a key recorder and a restore button; optional features have a checkbox
// (off: their keys go to the app); fixed keys are listed read-only with the reason.  Whether a feature is on comes
// from `Preferences` only (`KeyAvailability`), so this page and the others always agree.

/// The sections of the 快捷鍵 page: one per page with keys, named as the page, so the overview and the pages sort
/// the keys the same way.  Other pages link to them.
enum KeySection: String, CaseIterable, Identifiable {
    case typing, punctuation, mode, candidates, corrections, words
    var id: String { rawValue }
    /// The page where these keys are set.
    var pane: SettingsPane {
        switch self {
        case .typing: return .typing
        case .punctuation: return .punctuation
        case .mode: return .mode
        case .candidates: return .candidates
        case .corrections: return .corrections
        case .words: return .words
        }
    }
    var title: String { pane.title }
}

/// Why a feature is not in use although its checkbox may be on: another setting turns it off or changes it.
struct KeyAvailability {
    var note: String
    var place: SettingsPlace
    var linkTitle: String
}

extension Preferences {
    /// What another page's setting does to a feature of the 快捷鍵 page; nil when nothing.
    func availability(_ feature: KeyFeature) -> KeyAvailability? {
        switch feature {
        case .zhuyinRowClick where !showsZhuyinHint:
            return KeyAvailability(note: "「顯示注音」已關閉，候選字視窗沒有注音列",
                                   place: SettingsPlace(.candidates, row: SettingsRow.showsZhuyin), linkTitle: "視窗與提示")
        case .correctionsList where typoTolerance == .off && fuzzySounds.isEmpty:
            return KeyAvailability(note: "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單",
                                   place: SettingsPlace(.corrections, row: SettingsRow.typo), linkTitle: "修正")
        default:
            return nil
        }
    }
}

/// One row of the page.
struct KeyItem: Identifiable {
    enum Keys {
        /// Read-only keys; a second group is an alternative.
        case fixed([String], [String] = [])
        /// A rebindable command.
        case command(ShortcutCommand)
        /// The Shift tap that switches 中/英, with its side.
        case shiftTap
    }
    var id: String
    var title: String
    var subtitle: String?
    var keys: Keys
    var feature: KeyFeature?
    /// Where the fixed keys or their behaviour are set.
    var link: (title: String, place: SettingsPlace)?

    /// The page and row where this action is set, for the row's ›.
    var home: SettingsPlace? {
        if let link { return link.place }
        if case .command(let command) = keys { return command.place }
        if case .shiftTap = keys { return SettingsPlace(.mode, row: SettingsRow.shiftTap) }
        return feature?.place
    }

    /// The row's id for links: a command's is its 「快捷鍵」 row id, as on its feature's page.
    var anchor: String {
        if case .command(let command) = keys { return command.keysRow }
        return "row-" + id
    }

    /// A key that cannot change and has no switch.
    var isFixed: Bool {
        if case .fixed = keys, feature == nil { return true }
        return false
    }

    func matches(_ query: String, _ keys: CurrentKeys) -> Bool {
        let query = query.trimmingCharacters(in: .whitespaces).lowercased()
            .replacingOccurrences(of: "全型", with: "全形").replacingOccurrences(of: "半型", with: "半形")
            .replacingOccurrences(of: "全形半形", with: "全半形")
            .replacingOccurrences(of: "+", with: " ").replacingOccurrences(of: "＋", with: " ")
        guard !query.isEmpty else { return true }
        var text = (title + " " + (subtitle ?? "")).replacingOccurrences(of: "全半形", with: "全形 半形 全半形")
        switch self.keys {
        case .fixed(let a, let b): text += " " + (a + b).joined(separator: " ")
        case .command(let command): text += " " + (keys.bindings[command] ?? []).map { shortcut in
            shortcut.keycaps.joined(separator: " ") + " " + shortcut.keycaps.map {
                ["⇧": "shift", "⌃": "control", "⌥": "option", "空白": "space"][$0] ?? $0
            }.joined(separator: " ")
        }.joined(separator: " ")
        case .shiftTap: text += " shift"
        }
        return query.split(whereSeparator: \.isWhitespace).allSatisfy { text.lowercased().contains($0) }
    }
}

struct KeysPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    @State private var confirmingReset = false
    @State private var showsFixed = DemoPinning.showsFixedKeys

    var body: some View {
        let keys = CurrentKeys(preferences)
        let all = KeySection.allCases.map { ($0, items($0, keys).filter { $0.matches(model.keysQuery, keys) }) }
        // Keys that can change come first, by feature; the fixed ones wait under a disclosure at the bottom.
        let sections = all.map { ($0.0, $0.1.filter { !$0.isFixed }) }.filter { !$0.1.isEmpty }
        let fixed = all.flatMap { $0.1.filter(\.isFixed) }
        let conflicts = preferences.allConflicts
        let changed = ShortcutCommand.allCases.contains(where: preferences.isCustomized) || !preferences.disabledKeyFeatures.isEmpty
            || preferences.shiftToggle != .both
        SettingsForm {
            if !conflicts.isEmpty {
                SettingsSection("按鍵衝突") {
                    ForEach(Array(conflicts.enumerated()), id: \.offset) { _, conflict in
                        Button {
                            SettingsWindowController.shared.open(conflict.0.place)
                        } label: {
                            SettingRow(title: "「\(conflict.0.label)」與「\(conflict.1.label)」",
                                       subtitle: "用同一個按鍵；同一個按鍵只會執行其中一個功能", accessory: .chevron) {
                                KeyChip(text: conflict.2.compact, tint: Color(nsColor: .systemRed))
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            if sections.isEmpty && fixed.isEmpty {
                SettingsSection {
                    Text("沒有符合「\(model.keysQuery)」的項目")
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 60)
                }
            }
            ForEach(sections, id: \.0) { section, rows in
                SettingsSection(section.title) {
                    ForEach(rows, id: \.anchor) { item in
                        KeyItemRow(item: item, preferences: preferences, keys: keys)
                    }
                }
            }
            if !fixed.isEmpty {
                // A link to one of the fixed keys unfolds them.
                let open = showsFixed || !model.keysQuery.isEmpty || fixed.contains { $0.anchor == model.focusTarget }
                SettingsSection {
                    Button {
                        withAnimation(.easeInOut(duration: 0.2)) { showsFixed.toggle() }
                    } label: {
                        SettingRow(title: "固定的按鍵", subtitle: "打注音、聲調、送出與移動游標的按鍵，無法更改") {
                            Image(systemName: "chevron.right")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(.tertiary)
                                .rotationEffect(.degrees(open ? 90 : 0))
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(open ? "收起固定的按鍵" : "顯示固定的按鍵")
                    if open {
                        ForEach(fixed, id: \.anchor) { item in
                            KeyItemRow(item: item, preferences: preferences, keys: keys)
                        }
                    }
                } footer: {
                    HStack {
                        FooterText("按鍵可以在這裡或功能的頁面修改；› 到功能的頁面。")
                        Button("全部還原…") { confirmingReset = true }
                            .buttonStyle(.link)
                            .disabled(!changed)
                    }
                }
            }
        }
        .alert("要還原所有按鍵與功能嗎？", isPresented: $confirmingReset) {
            Button("全部還原", role: .destructive) {
                // Every key and key feature back to its default, through the registry.
                preferences.restoreDefaults(SettingsRegistry.keyAndFeatureIDs)
                preferences.shiftToggle = .both
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("所有按鍵回到預設值，所有功能開啟，單按左右 Shift 切換中英。")
        }
    }

    // MARK: The rows

    /// The rows of `section` (also read by `SettingsLinks` for their links).
    func items(_ section: KeySection, _ keys: CurrentKeys) -> [KeyItem] {
        let vertical = preferences.candidateLayout == .vertical
        let space = preferences.spaceAction
        switch section {
        case .typing:
            return [
                KeyItem(id: "zhuyin", title: "拼注音", subtitle: "固定：注音鍵的位置依標準注音鍵盤", keys: .fixed(["注音鍵"])),
                KeyItem(id: "tones", title: "聲調：二聲、三聲、四聲、輕聲", subtitle: "固定", keys: .fixed(["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"])),
                KeyItem(id: "space", title: "空白", subtitle: OptionDemos.spaceSubtitle(space).replacingOccurrences(of: "，再按空白", with: "")
                        + "；打注音時是一聲", keys: .fixed(["空白"]), link: ("在「打字」設定", SettingsPlace(.typing, row: SettingsRow.space))),
                KeyItem(id: "commit", title: "送出", subtitle: "固定", keys: .fixed(["Return"])),
                KeyItem(id: "newline", title: "送出並換行", keys: .command(.commitWithNewline)),
                KeyItem(id: "backspace", title: "刪掉注音符號，或游標前的字", subtitle: "固定", keys: .fixed(["⌫"])),
                KeyItem(id: "deleteWord", title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音", keys: .command(.deleteWord),
                        feature: .deleteWord),
                KeyItem(id: "escape", title: preferences.escapeClearsComposition ? "清除注音，再按一次清除組字" : "清除注音",
                        keys: .fixed(["Esc"]), link: ("在「打字」設定", SettingsPlace(.typing, row: SettingsRow.escape))),
                KeyItem(id: "cursor", title: "在組字中移動游標", subtitle: "固定", keys: .fixed(["←", "→"], ["Home", "End"])),
                KeyItem(id: "insertSpace", title: "在組字中放一個空白", keys: .command(.insertSpace)),
                KeyItem(id: "forwardDelete", title: "刪掉游標後的字", subtitle: "固定", keys: .fixed(["⌦"])),
            ]
        case .punctuation:
            let windows = preferences.punctuationKeys == .windows
            return ([
                KeyItem(id: "punctuationWidth", title: "切換標點全半形", subtitle: "中文預設全形，英數與空白維持半形",
                        keys: .command(.togglePunctuationWidth)),
                KeyItem(id: "brackets", title: "「 」 、", subtitle: "固定", keys: .fixed(["[", "]", "'"])),
                KeyItem(id: "shiftSymbols", title: windows ? "鍵上的符號：＜ ＞ ＿ ？ ！" : "中文標點：，。？！：『』——",
                        keys: .fixed(["⇧", "符號鍵"]), link: ("標點按法在「標點與符號」設定", SettingsPlace(.punctuation, row: SettingsRow.punctuationKeys))),
                KeyItem(id: "controlPunctuation", title: "中文標點：，。、？！：；『』", subtitle: "兩種標點按法都能用",
                        keys: .fixed(["⌃", "符號鍵"]), feature: .controlPunctuation),
                KeyItem(id: "optionSymbols", title: "鍵上的符號：. / ; < >", subtitle: "兩種標點按法都能用",
                        keys: .fixed(["⌥", "數字或符號鍵"]), feature: .optionSymbols),
                KeyItem(id: "punctuationList", title: "列出同類的標點", subtitle: "在標點上按", keys: .fixed(["↓"])),
                KeyItem(id: "symbolPicker", title: "開啟符號與 Emoji 面板", subtitle: "再按一次或按 Esc 關閉",
                        keys: .command(.symbolPicker), feature: .symbolPicker),
            ] as [KeyItem]).filter { $0.id != "symbolPicker" || SymbolSources.isAvailable }
        case .mode:
            return [
                KeyItem(id: "shiftTap", title: "單按 Shift 切換中英", subtitle: "在可以打字的地方", keys: .shiftTap,
                        link: ("在「中英切換」設定", SettingsPlace(.mode, row: SettingsRow.shiftTap))),
                KeyItem(id: "capsLock", title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出",
                        keys: .fixed(["Caps Lock"]), feature: .capsLockEnglish),
                KeyItem(id: "quickSettings", title: "開啟快速設定", subtitle: "在游標旁切換英文、半形標點、簡體與暫停學習",
                        keys: .command(.quickSettings)),
            ]
        case .candidates:
            return [
                KeyItem(id: "open", title: "開啟候選字視窗", subtitle: "固定", keys: .fixed(["↓"], space == .openCandidates ? ["空白"] : [])),
                KeyItem(id: "pick", title: "選字", subtitle: "目前每頁 \(preferences.candidatesPerPage) 個；Return 選取反白的字",
                        keys: .fixed([keys.selection]), link: ("在「候選字視窗」設定", SettingsPlace(.candidates, row: SettingsRow.selection))),
                KeyItem(id: "move", title: "移動反白", subtitle: vertical ? "直式清單" : "橫式清單；↑ ↓ 翻頁",
                        keys: .fixed(vertical ? ["↑", "↓"] : ["←", "→"]), link: ("排列在「候選字視窗」設定", SettingsPlace(.candidates, row: SettingsRow.layout))),
                KeyItem(id: "nextPage", title: "下一頁", subtitle: "空白也翻到下一頁", keys: .command(.nextPage), feature: .pageKeys),
                KeyItem(id: "previousPage", title: "上一頁", keys: .command(.previousPage), feature: .pageKeys),
                KeyItem(id: "previousCharacter", title: "改前一個字", subtitle: vertical ? "清單開著時；直式清單也可以按 ←" : "清單開著時",
                        keys: .command(.previousCharacter)),
                KeyItem(id: "nextCharacter", title: "改後一個字", subtitle: vertical ? "清單開著時；直式清單也可以按 →" : "清單開著時",
                        keys: .command(.nextCharacter)),
                KeyItem(id: "single", title: "只列一個字", subtitle: "再按一次還原", keys: .command(.singleCharacterList),
                        feature: .singleCharacterList),
                KeyItem(id: "zhuyinRow", title: "點注音列打出注音", subtitle: "候選字視窗上方的注音", keys: .fixed(["點一下"]),
                        feature: .zhuyinRowClick),
                KeyItem(id: "close", title: "關閉候選字視窗", subtitle: "固定", keys: .fixed(["Esc"])),
            ]
        case .corrections:
            return [
                KeyItem(id: "corrections", title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原",
                        keys: .command(.correctionsList), feature: .correctionsList),
                KeyItem(id: "typeZhuyin", title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復", keys: .command(.typeZhuyin),
                        feature: .typeZhuyin),
                KeyItem(id: "deleteAfterCommit", title: "送出後刪掉重打",
                        subtitle: "送出後 30 秒內刪掉剛送出的字再打，算作改字並接著前文選字", keys: .fixed(["⌫"]),
                        feature: .deleteAfterCommit),
            ]
        case .words:
            return [
                KeyItem(id: "selectLeft", title: "往左選取", subtitle: "再按 Return 加入自訂詞", keys: .command(.selectLeft),
                        feature: .selectPhrase),
                KeyItem(id: "selectRight", title: "往右選取", subtitle: "再按 Return 加入自訂詞", keys: .command(.selectRight),
                        feature: .selectPhrase),
                KeyItem(id: "addSelectedText", title: "選取的字加入自訂詞", subtitle: "在 App 裡選取 1 到 6 個字再按；預設沒有按鍵",
                        keys: .command(.addSelectedText)),
                KeyItem(id: "blockCandidate", title: "不要再推薦", subtitle: "反白的字詞之後不再自動選用；也可以在候選字上按右鍵",
                        keys: .command(.blockCandidate)),
                KeyItem(id: "forgetCandidate", title: "忘記這個選字", subtitle: "忘記選過反白的字詞",
                        keys: .command(.forgetCandidate)),
                KeyItem(id: "addCandidatePhrase", title: "加入自訂詞", subtitle: "反白的字詞；沒有開清單時是整段組字",
                        keys: .command(.addCandidatePhrase)),
                KeyItem(id: "undoWordAction", title: "復原上一個動作", subtitle: "動作後 5 秒內，組字還在時",
                        keys: .fixed(["⌃", "Z"])),
            ]
        }
    }
}

/// An index row: the action, its keys (and 「已自訂」 when they are not the defaults) and ›; the whole row opens the
/// feature's page at its 快捷鍵 row, where the keys are edited.  A feature that is off is dimmed with 「已關閉」.
/// Fixed keys without a page are plain rows.
private struct KeyItemRow: View {
    let item: KeyItem
    @ObservedObject var preferences: Preferences
    let keys: CurrentKeys

    private var on: Bool {
        if case .shiftTap = item.keys { return preferences.shiftSwitchEnabled }
        return item.feature.map { keys.isOn($0) } ?? true
    }

    @ObservedObject private var hover = HoverState.shared

    var body: some View {
        let availability = item.feature.flatMap(preferences.availability)
        if case .command(let command) = item.keys, on {
            // Keys set right here, with the same field as the feature's page; › goes to the feature.
            editableRow(command, availability)
        } else if let home = item.home {
            Button {
                SettingsWindowController.shared.open(home)
            } label: {
                row(availability).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityHint("開啟「\(home.subpage?.title ?? home.pane.title)」")
        } else {
            row(availability)
        }
    }

    private func editableRow(_ command: ShortcutCommand, _ availability: KeyAvailability?) -> some View {
        let hovered = hover.hovered == AnyHashable("row-" + item.id) || DemoPinning.hoveredRow == "row-" + item.id
        let customized = preferences.isCustomized(command)
        var accessory = RowAccessory.none
        if let home = item.home {
            let name: String = home.subpage?.title ?? home.pane.title
            accessory = .link(help: "前往「\(name)」", action: { SettingsWindowController.shared.open(home) })
        }
        let rowID = AnyHashable("row-" + item.id)
        _ = hovered
        return SettingRow(title: item.title, subtitle: availability?.note ?? item.subtitle, marked: customized,
                          accessory: accessory, anchor: item.anchor) {
            ShortcutField(preferences: preferences, command: command, disabled: availability != nil,
                          row: rowID, maxVisible: 2, preview: DemoPinning.fieldStates[command])
        }
        .hoverTracked("row-" + item.id)
        .contextMenu {
            if customized { Button("還原預設") { _ = preferences.resetBindings(command) } }
            if let home = item.home { Button("前往「\(home.subpage?.title ?? home.pane.title)」") { SettingsWindowController.shared.open(home) } }
        }
    }

    private func row(_ availability: KeyAvailability?) -> some View {
        SettingRow(title: item.title, subtitle: availability?.note ?? item.subtitle, dimmed: !on,
                   accessory: item.home == nil ? .none : .chevron, anchor: item.anchor) {
            if !on {
                Text("已關閉").foregroundStyle(.secondary)
            } else {
                trailing.opacity(availability == nil ? 1 : 0.45)
            }
        }
    }

    @ViewBuilder private var trailing: some View {
        switch item.keys {
        case .fixed(let caps, let alternative):
            HStack(spacing: 6) {
                KeyChip(text: caps.joined())
                if !alternative.isEmpty { KeyChip(text: alternative.joined()) }
            }
            .accessibilityLabel(alternative.isEmpty ? caps.joined() : caps.joined() + " 或 " + alternative.joined())
        case .command(let command):
            let conflicts = preferences.conflicts(of: command)
            HStack(spacing: 6) {
                ForEach(Array((preferences.bindings[command] ?? []).enumerated()), id: \.offset) { index, shortcut in
                    KeyChip(text: shortcut.compact, tint: conflicts[index] == nil ? nil : Color(nsColor: .systemRed))
                }
            }
        case .shiftTap:
            KeyChip(text: keys.shiftTap ?? "Shift")
        }
    }
}
