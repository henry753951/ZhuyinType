import AppKit
import SwiftUI
import ZhuyinCore

// 快捷鍵, in the manner of System Settings → Keyboard → Keyboard Shortcuts: every key grouped by feature, in the words
// of what it does.  Rebindable commands have a key recorder and a restore button; optional features have a checkbox
// (off: their keys go to the app); fixed keys are listed read-only with the reason.  Whether a feature is on comes
// from `Preferences` only (`KeyAvailability`), so this page and the others always agree.

/// The sections of the 快捷鍵 page; other pages link to them.
enum KeySection: String, CaseIterable, Identifiable {
    case composing, list, editing, punctuation, mode, other
    var id: String { rawValue }
    var title: String {
        switch self {
        case .composing: return "組字"
        case .list: return "候選字清單"
        case .editing: return "改字與選取"
        case .punctuation: return "標點與符號"
        case .mode: return "中英切換"
        case .other: return "其他"
        }
    }
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
                                   place: SettingsPlace(.choosing, row: SettingsRow.typo), linkTitle: "選字")
        default:
            return nil
        }
    }
}

/// One row of the page.
private struct KeyItem: Identifiable {
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
        if case .shiftTap = keys { return SettingsPlace(.typing, row: SettingsRow.shiftTap) }
        return feature?.place
    }

    func matches(_ query: String, _ keys: CurrentKeys) -> Bool {
        let query = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !query.isEmpty else { return true }
        var text = title + " " + (subtitle ?? "")
        switch self.keys {
        case .fixed(let a, let b): text += " " + (a + b).joined(separator: " ")
        case .command(let command): text += " " + keys.assigned(command).joined()
        case .shiftTap: text += " shift"
        }
        return text.lowercased().contains(query)
    }
}

struct KeysPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    @State private var confirmingReset = false

    var body: some View {
        let keys = CurrentKeys(preferences)
        let sections = KeySection.allCases.map { ($0, items($0, keys).filter { $0.matches(model.keysQuery, keys) }) }
            .filter { !$0.1.isEmpty }
        Form {
                Section {
                    LabeledContent {
                        Button("全部還原…") { confirmingReset = true }
                            .disabled(preferences.bindings == ShortcutCommand.defaultBindings && preferences.disabledKeyFeatures.isEmpty
                                      && preferences.shiftToggle == .both)
                    } label: {
                        RowLabel("按鍵與功能", "點按鍵後按下新的組合，Esc 取消；取消勾選的功能，按鍵交給 App")
                    }
                }
                let conflicts = preferences.allConflicts
                if !conflicts.isEmpty {
                    Section {
                        ForEach(Array(conflicts.enumerated()), id: \.offset) { _, conflict in
                            LabeledContent {
                                KeyCombo(keys: conflict.2.keycaps, tint: Color(nsColor: .systemRed))
                            } label: {
                                HStack(alignment: .firstTextBaseline, spacing: 8) {
                                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Color(nsColor: .systemRed))
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text("「\(conflict.0.label)」與「\(conflict.1.label)」用同一個按鍵")
                                        PlaceLink(title: "前往「\(conflict.0.label)」", place: conflict.0.place)
                                    }
                                }
                            }
                        }
                    } header: {
                        Text("按鍵衝突")
                    } footer: {
                        FooterText("同一個按鍵只會執行其中一個功能；請改掉其中一個。")
                    }
                }
                if sections.isEmpty {
                    Section {
                        Text("沒有符合「\(model.keysQuery)」的項目")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, minHeight: 60)
                    }
                }
                ForEach(sections, id: \.0) { section, rows in
                    Section {
                        ForEach(Array(rows.enumerated()), id: \.element.id) { index, item in
                            KeyItemRow(item: item, preferences: preferences, keys: keys)
                                .id(index == 0 ? section.id : "row-" + item.id)
                        }
                    } header: {
                        Text(section.title)
                    }
                }
            }
        .alert("要還原所有按鍵與功能嗎？", isPresented: $confirmingReset) {
            Button("全部還原", role: .destructive) {
                preferences.resetShortcuts()
                preferences.disabledKeyFeatures = []
                preferences.shiftToggle = .both
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("所有按鍵回到預設值，所有功能開啟，單按左右 Shift 切換中英。")
        }
    }

    // MARK: The rows

    private func items(_ section: KeySection, _ keys: CurrentKeys) -> [KeyItem] {
        let vertical = preferences.candidateLayout == .vertical
        let space = preferences.spaceAction
        switch section {
        case .composing:
            return [
                KeyItem(id: "zhuyin", title: "拼注音", subtitle: "固定：注音鍵的位置依標準注音鍵盤", keys: .fixed(["注音鍵"])),
                KeyItem(id: "tones", title: "聲調：二聲、三聲、四聲、輕聲", subtitle: "固定", keys: .fixed(["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"])),
                KeyItem(id: "space", title: "空白", subtitle: (space == .openCandidates ? "打完注音後開啟候選字視窗" : "打完注音後送出並輸入空白")
                        + "；打注音時是一聲", keys: .fixed(["空白"]), link: ("在「打字」設定", SettingsPlace(.typing, row: SettingsRow.space))),
                KeyItem(id: "commit", title: "送出", subtitle: "固定", keys: .fixed(["Return"])),
                KeyItem(id: "newline", title: "送出並換行", keys: .command(.commitWithNewline)),
                KeyItem(id: "backspace", title: "刪掉注音符號，或游標前的字", subtitle: "固定", keys: .fixed(["⌫"])),
                KeyItem(id: "deleteWord", title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音", keys: .command(.deleteWord),
                        feature: .deleteWord),
                KeyItem(id: "escape", title: preferences.escapeClearsComposition ? "清除注音，再按一次清除組字" : "清除注音",
                        keys: .fixed(["Esc"]), link: ("在「打字」設定", SettingsPlace(.typing, row: SettingsRow.escape))),
                KeyItem(id: "cursor", title: "在組字中移動游標", subtitle: "固定", keys: .fixed(["←", "→"], ["Home", "End"])),
            ]
        case .list:
            return [
                KeyItem(id: "open", title: "開啟候選字視窗", subtitle: "固定", keys: .fixed(["↓"], space == .openCandidates ? ["空白"] : [])),
                KeyItem(id: "pick", title: "選字", subtitle: "目前每頁 \(preferences.candidatesPerPage) 個；Return 選取反白的字",
                        keys: .fixed([keys.selection]), link: ("在「視窗與提示」設定", SettingsPlace(.candidates, row: SettingsRow.selection))),
                KeyItem(id: "move", title: "移動反白", subtitle: vertical ? "直式清單" : "橫式清單；↑ ↓ 翻頁",
                        keys: .fixed(vertical ? ["↑", "↓"] : ["←", "→"]), link: ("排列在「視窗與提示」設定", SettingsPlace(.candidates, row: SettingsRow.layout))),
            ] + (vertical ? [KeyItem(id: "neighbour", title: "改前一個或後一個字", subtitle: "直式清單", keys: .fixed(["←", "→"]))] : []) + [
                KeyItem(id: "nextPage", title: "下一頁", subtitle: "空白也翻到下一頁", keys: .command(.nextPage), feature: .pageKeys),
                KeyItem(id: "previousPage", title: "上一頁", keys: .command(.previousPage), feature: .pageKeys),
                KeyItem(id: "single", title: "只列一個字", subtitle: "再按一次還原", keys: .command(.singleCharacterList),
                        feature: .singleCharacterList),
                KeyItem(id: "corrections", title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原",
                        keys: .command(.correctionsList), feature: .correctionsList),
                KeyItem(id: "zhuyinRow", title: "點注音列打出注音", subtitle: "候選字視窗上方的注音", keys: .fixed(["點一下"]),
                        feature: .zhuyinRowClick),
                KeyItem(id: "close", title: "關閉候選字視窗", subtitle: "固定", keys: .fixed(["Esc"])),
            ]
        case .editing:
            return [
                KeyItem(id: "typeZhuyin", title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復", keys: .command(.typeZhuyin),
                        feature: .typeZhuyin),
                KeyItem(id: "selectLeft", title: "往左選取", subtitle: "再按 Return 加入自訂詞", keys: .command(.selectLeft),
                        feature: .selectPhrase),
                KeyItem(id: "selectRight", title: "往右選取", subtitle: "再按 Return 加入自訂詞", keys: .command(.selectRight),
                        feature: .selectPhrase),
                KeyItem(id: "deleteAfterCommit", title: "送出後刪掉重打",
                        subtitle: "送出後 30 秒內刪掉剛送出的字再打，算作改字並接著前文選字", keys: .fixed(["⌫"]),
                        feature: .deleteAfterCommit),
            ]
        case .punctuation:
            let windows = preferences.punctuationKeys == .windows
            return [
                KeyItem(id: "brackets", title: "「 」 、", subtitle: "固定", keys: .fixed(["[", "]", "'"])),
                KeyItem(id: "shiftSymbols", title: windows ? "鍵上的符號：< > _ ? !" : "中文標點：，。？！：『』——",
                        keys: .fixed(["⇧", "符號鍵"]), link: ("標點按法在「打字」設定", SettingsPlace(.typing, row: SettingsRow.punctuationKeys))),
                KeyItem(id: "controlPunctuation", title: "中文標點：，。、？！：；『』", subtitle: "兩種標點按法都能用",
                        keys: .fixed(["⌃", "符號鍵"]), feature: .controlPunctuation),
                KeyItem(id: "optionSymbols", title: "鍵上的符號：. / ; < >", subtitle: "兩種標點按法都能用",
                        keys: .fixed(["⌥", "數字或符號鍵"]), feature: .optionSymbols),
                KeyItem(id: "punctuationList", title: "列出同類的標點", subtitle: "在標點上按", keys: .fixed(["↓"])),
            ]
        case .mode:
            return [
                KeyItem(id: "shiftTap", title: "單按 Shift 切換中英", subtitle: "在可以打字的地方", keys: .shiftTap,
                        link: ("在「打字」設定", SettingsPlace(.typing, row: SettingsRow.shiftTap))),
                KeyItem(id: "capsLock", title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出",
                        keys: .fixed(["Caps Lock"]), feature: .capsLockEnglish),
            ]
        case .other:
            return [
                KeyItem(id: "shiftSpace", title: "在組字中放一個空白", subtitle: "固定", keys: .fixed(["⇧", "空白"])),
                KeyItem(id: "forwardDelete", title: "刪掉游標後的字", subtitle: "固定", keys: .fixed(["⌦"])),
            ]
        }
    }
}

/// A row: checkbox (optional features), title, one line, link; keys or recorder, and restore, on the trailing side.
private struct KeyItemRow: View {
    let item: KeyItem
    @ObservedObject var preferences: Preferences
    let keys: CurrentKeys
    @State private var problem: String?

    init(item: KeyItem, preferences: Preferences, keys: CurrentKeys) {
        self.item = item
        self.preferences = preferences
        self.keys = keys
        _problem = State(initialValue: DemoPinning.keyProblems[item.id])
    }

    private var on: Bool {
        if case .shiftTap = item.keys { return preferences.shiftSwitchEnabled }
        return item.feature.map { keys.isOn($0) } ?? true
    }

    var body: some View {
        let availability = item.feature.flatMap(preferences.availability)
        LabeledContent {
            HStack(spacing: 8) {
                if let problem {
                    Text(problem).font(.callout).foregroundStyle(.red).lineLimit(2).multilineTextAlignment(.trailing)
                }
                trailing
                    .opacity(on && availability == nil ? 1 : 0.45)
                if let home = item.home, item.link == nil {
                    Button {
                        SettingsWindowController.shared.open(home)
                    } label: {
                        Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(.tertiary)
                            .frame(width: 16, height: 20)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help("在「\(home.pane.title)」設定")
                    .accessibilityLabel("在「\(home.pane.title)」設定")
                } else {
                    Color.clear.frame(width: 16, height: 1)
                }
            }
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                checkbox.frame(width: 16)
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title).foregroundStyle(on ? .primary : .secondary)
                    if let line = subtitle(availability) {
                        Text(line).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    }
                    if let availability {
                        PlaceLink(title: "在「\(availability.linkTitle)」設定", place: availability.place)
                    } else if let link = item.link {
                        PlaceLink(title: link.title, place: link.place)
                    }
                }
            }
        }
    }

    private func subtitle(_ availability: KeyAvailability?) -> String? {
        if let availability { return availability.note }
        if !on {
            switch item.keys {
            case .command(let command): return "已關閉：\(keys.assigned(command).joined()) 交給 App"
            case .shiftTap: return "已關閉：Shift 只當作 Shift"
            case .fixed(let caps, _):
                return item.feature == .zhuyinRowClick ? "已關閉：點注音列不做事" : "已關閉：\(caps.joined(separator: " ")) 交給 App"
            }
        }
        return item.subtitle
    }

    @ViewBuilder private var checkbox: some View {
        if case .shiftTap = item.keys {
            Toggle(item.title, isOn: binding(preferences, \.shiftSwitchEnabled)).labelsHidden().toggleStyle(.checkbox)
        } else if let feature = item.feature {
            Toggle(item.title, isOn: Binding(get: { preferences.isEnabled(feature) },
                                             set: { preferences.setEnabled(feature, $0) }))
                .labelsHidden()
                .toggleStyle(.checkbox)
        } else {
            Color.clear.frame(height: 1)
        }
    }

    @ViewBuilder private var trailing: some View {
        switch item.keys {
        case .fixed(let caps, let alternative):
            HStack(spacing: 6) {
                KeyCombo(keys: caps)
                if !alternative.isEmpty {
                    Text("或").foregroundStyle(.tertiary)
                    KeyCombo(keys: alternative)
                }
            }
        case .command(let command):
            ShortcutField(preferences: preferences, command: command, disabled: !on,
                          preview: DemoPinning.fieldStates[command])
        case .shiftTap:
            Picker(item.title, selection: Binding(
                get: { preferences.shiftToggle == .none ? .both : preferences.shiftToggle },
                set: { preferences.shiftToggle = $0 })) {
                Text("左右 Shift").tag(ShiftToggle.both)
                Text("左 Shift").tag(ShiftToggle.left)
                Text("右 Shift").tag(ShiftToggle.right)
            }
            .labelsHidden()
            .fixedSize()
            .disabled(!on)
        }
    }
}

/// The small keyboard button beside a setting with keys: opens its section of the 快捷鍵 page.
struct KeysLink: View {
    let section: KeySection
    var body: some View {
        Button {
            SettingsWindowController.shared.showKeys(section)
        } label: {
            Image(systemName: "keyboard")
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.borderless)
        .help("在「快捷鍵」顯示「\(section.title)」的按鍵")
        .accessibilityLabel("「\(section.title)」的按鍵")
    }
}
