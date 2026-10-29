import AppKit
import ZhuyinEngine

// Every setting the user can change, declared once: its name, value, options and their labels, where it is shown,
// the preview state it affects and its demo.  Setting pages, the 快捷鍵 overview, the setup wizard, presets and the
// 「原本 → 改成」 list are all built from these; adding a setting means adding a descriptor here (and its lesson, if
// the wizard should teach it).  Values are read and written through `Preferences`, whose storage may be the user's
// defaults or a sandbox in memory.  See docs/product/SETTINGS_API.md.

/// A setting's value, in a form every kind of setting shares.
enum SettingValue: Hashable {
    case bool(Bool)
    case text(String)
    case number(Double)
    case keys([Shortcut])
}

/// One choice of a setting, with its label.
struct SettingOption: Hashable {
    let value: SettingValue
    let title: String
}

struct SettingDescriptor: Identifiable {
    enum Kind {
        case toggle
        case choice([SettingOption])
        /// A number in a range; `unit` follows it in labels (「 pt」「 個」).
        case number(ClosedRange<Double>, step: Double, unit: String)
        /// Flags of a set, each with its label (模糊音).
        case flags([(bit: Int, title: String)])
        /// A command's keys.
        case keys(ShortcutCommand)
    }

    let id: String
    let title: String
    let kind: Kind
    /// Where the setting is on the settings pages (its row, for links and the arrival cue).
    let place: SettingsPlace
    /// The 視窗與提示 preview state that shows it, if any.
    var preview: PreviewState?
    /// The ⓘ demo or wizard animation that shows it, by name.
    var demo: String?
    /// Words that find it in search, besides its title.
    var keywords: [String] = []
    /// A line under the title that follows the value.
    var detail: ((Preferences) -> String?)?
    let read: (Preferences) -> SettingValue
    let write: (Preferences, SettingValue) -> Void

    /// The value's label: an option's title, 「開啟」「關閉」, a number with its unit, keys.
    func label(_ value: SettingValue) -> String {
        switch (kind, value) {
        case (.toggle, .bool(let on)): return on ? "開啟" : "關閉"
        case (.choice(let options), _): return options.first { $0.value == value }?.title ?? "—"
        case (.number(_, _, let unit), .number(let number)):
            return (number.rounded() == number ? String(Int(number)) : String(format: "%.1f", number)) + unit
        case (.flags(let flags), .number(let raw)):
            let titles = flags.filter { Int(raw) & $0.bit != 0 }.map(\.title)
            return titles.isEmpty ? "關閉" : titles.joined(separator: "、")
        case (.keys, .keys(let keys)):
            return keys.isEmpty ? "沒有按鍵" : keys.map(\.compact).joined(separator: "、")
        default: return "—"
        }
    }
}

enum SettingsRegistry {
    /// Every setting, in the order of the settings pages.
    static let all: [SettingDescriptor] = typing + choosing + candidates + words + advanced + keys

    static func setting(_ id: String) -> SettingDescriptor? { index[id] }
    /// The commands' keys and the features that turn keys on and off (the 快捷鍵 page's 全部還原).
    static var keyAndFeatureIDs: Set<String> { Set(all.map(\.id).filter { $0.hasPrefix("keys.") || $0.hasPrefix("feature.") }) }
    private static let index = Dictionary(uniqueKeysWithValues: all.map { ($0.id, $0) })

    /// The defaults: what a fresh set of preferences holds.
    static var defaults: SettingsSnapshot { SettingsSnapshot(Preferences(defaults: MemorySettingsStorage())) }

    // MARK: Builders

    private static func toggle(_ id: String, _ title: String, _ place: SettingsPlace,
                               _ path: ReferenceWritableKeyPath<Preferences, Bool>, preview: PreviewState? = nil,
                               detail: ((Preferences) -> String?)? = nil, keywords: [String] = []) -> SettingDescriptor {
        SettingDescriptor(id: id, title: title, kind: .toggle, place: place, preview: preview, keywords: keywords, detail: detail,
                          read: { .bool($0[keyPath: path]) },
                          write: { p, v in if case .bool(let on) = v { p[keyPath: path] = on } })
    }

    private static func choice<E: RawRepresentable & Hashable>(
        _ id: String, _ title: String, _ place: SettingsPlace, _ path: ReferenceWritableKeyPath<Preferences, E>,
        _ options: [(E, String)], preview: PreviewState? = nil, demo: String? = nil,
        detail: ((Preferences) -> String?)? = nil, keywords: [String] = []
    ) -> SettingDescriptor where E.RawValue == String {
        SettingDescriptor(id: id, title: title, kind: .choice(options.map { SettingOption(value: .text($0.0.rawValue), title: $0.1) }),
                          place: place, preview: preview, demo: demo, keywords: keywords, detail: detail,
                          read: { .text($0[keyPath: path].rawValue) },
                          write: { p, v in if case .text(let raw) = v, let value = E(rawValue: raw) { p[keyPath: path] = value } })
    }

    private static func feature(_ feature: KeyFeature, _ title: String) -> SettingDescriptor {
        SettingDescriptor(id: "feature." + feature.rawValue, title: title, kind: .toggle, place: feature.place,
                          read: { .bool($0.isEnabled(feature)) },
                          write: { p, v in if case .bool(let on) = v { p.setEnabled(feature, on) } })
    }

    private static func place(_ pane: SettingsPane, _ row: String? = nil, _ subpage: SettingsSubpage? = nil) -> SettingsPlace {
        SettingsPlace(pane, subpage: subpage, row: row)
    }

    // MARK: 打字

    static let typing: [SettingDescriptor] = [
        SettingDescriptor(id: "spaceAction", title: "空白鍵",
                          kind: .choice(Preferences.spaceActions.map { SettingOption(value: .text($0.rawValue), title: OptionDemos.spaceLabel($0)) }),
                          place: place(.typing, SettingsRow.space), demo: "space", keywords: ["空白", "Space"],
                          detail: { OptionDemos.spaceSubtitle($0.spaceAction) },
                          read: { .text($0.spaceAction.rawValue) },
                          write: { p, v in if case .text(let raw) = v, let action = SpaceAction(rawValue: raw) { p.spaceAction = action } }),
        SettingDescriptor(id: "escape", title: "Esc",
                          kind: .choice([SettingOption(value: .bool(true), title: "清除整段組字"), SettingOption(value: .bool(false), title: "只清除注音")]),
                          place: place(.typing, SettingsRow.escape), demo: "escape",
                          detail: { $0.escapeClearsComposition ? "先清除注音，再按一次清除整段組字" : "只清除還沒打完的注音" },
                          read: { .bool($0.escapeClearsComposition) },
                          write: { p, v in if case .bool(let on) = v { p.escapeClearsComposition = on } }),
        choice("punctuationKeys", "標點按法", place(.typing, SettingsRow.punctuationKeys), \.punctuationKeys,
               [(PunctuationKeys.windows, "Windows"), (.mac, "macOS")], demo: "punctuation",
               detail: { $0.punctuationKeys == .windows ? "⇧ 加符號鍵打出鍵上的符號，例如 <" : "⇧ 加符號鍵打出中文標點，例如 ，" },
               keywords: ["標點", "全形"]),
        feature(.controlPunctuation, "⌃ 加符號鍵打中文標點"),
        feature(.optionSymbols, "⌥ 加數字或符號鍵打鍵上的符號"),
        toggle("shiftSwitch", "單按 Shift 切換中英", place(.typing, SettingsRow.shiftTap), \.shiftSwitchEnabled, preview: .mode,
               detail: { $0.shiftSwitchEnabled ? "在可以打字的地方單按一下" : "關閉時，Shift 只當作 Shift" }, keywords: ["中英", "Shift"]),
        choice("shiftToggle", "使用的 Shift", place(.typing, SettingsRow.shiftTap), \.shiftToggle,
               [(ShiftToggle.both, "左右 Shift"), (.left, "左 Shift"), (.right, "右 Shift"), (.none, "不使用")], preview: .mode),
        feature(.capsLockEnglish, "Caps Lock 打英文"),
        toggle("showModeToast", "中英提示", place(.candidates, SettingsRow.modeToast), \.showModeToast, preview: .mode,
               detail: { p in CurrentKeys(p).modeKey.map { "按 \($0) 切換時，在游標旁顯示「中」或「英」" } ?? "切換時在游標旁顯示「中」或「英」" }),
        toggle("startInEnglish", "開始輸入時使用英文", place(.typing), \.startInEnglish),
        feature(.deleteWord, "刪掉一個詞"),
    ]

    // MARK: 選字

    static let choosing: [SettingDescriptor] = [
        choice("candidateAnchor", "游標在句中時改", place(.choosing), \.candidateAnchor,
               [(AnchorSetting.after, "游標右邊的字"), (.before, "游標左邊的字")], demo: "anchor", detail: { _ in "要改的字以底線標出" }),
        choice("characterForms", "台／臺", place(.choosing), \.characterForms, [(CharacterForms.common, "台"), (.standard, "臺")],
               detail: { _ in "顯示與送出的寫法" }),
        choice("typoTolerance", "按錯鍵修正", place(.choosing, SettingsRow.typo), \.typoTolerance,
               [(TypoSetting.off, "關閉"), (.standard, "標準"), (.aggressive, "積極")], preview: .corrections, demo: "typo",
               detail: { OptionDemos.typoSubtitle($0.typoTolerance) }, keywords: ["按錯", "容錯"]),
        toggle("readingChannel", "接受常見的其他讀法", place(.choosing, SettingsRow.otherReadings), \.readingChannel,
               detail: { $0.readingChannel ? "有邊讀邊、輕聲照本調打，也找得到要的詞" : "只照字典的讀音找詞" },
               keywords: ["讀音", "讀法", "有邊讀邊", "輕聲", "台灣讀音"]),
        feature(.correctionsList, "列出修正的字"),
        SettingDescriptor(id: "fuzzySounds", title: "模糊音",
                          kind: .flags([(1 << 0, "ㄗ 與 ㄓ"), (1 << 1, "ㄘ 與 ㄔ"), (1 << 2, "ㄙ 與 ㄕ"), (1 << 3, "ㄣ 與 ㄥ"),
                                        (1 << 4, "ㄢ 與 ㄤ")].map { (bit: $0.0, title: $0.1) }),
                          place: place(.choosing, SettingsRow.fuzzy), demo: "fuzzy", keywords: ["ㄗㄓ", "ㄣㄥ"],
                          read: { .number(Double($0.fuzzySounds.rawValue)) },
                          write: { p, v in if case .number(let raw) = v { p.fuzzySounds = FuzzySounds(rawValue: Int(raw)) } }),
        toggle("usesPrecedingText", "參考游標前的文字", place(.choosing, nil, .context), \.usesPrecedingText, keywords: ["前文"]),
        feature(.typeZhuyin, "打出注音本身"),
        feature(.selectPhrase, "選取加入自訂詞"),
        feature(.deleteAfterCommit, "送出後刪掉重打"),
    ]

    // MARK: 視窗與提示

    static let candidates: [SettingDescriptor] = [
        choice("candidateLayout", "排列", place(.candidates, SettingsRow.layout), \.candidateLayout,
               [(CandidateLayout.vertical, "直式"), (.horizontal, "橫式")], preview: .list,
               detail: { p in p.candidateLayout == .vertical ? "↑ ↓ 移動反白，← → 改前一個或後一個字"
                   : "← → 移動反白，↑ ↓ 翻頁" + (CurrentKeys(p).text(.previousCharacter).map { "，\($0) 改前一個字" } ?? "") }),
        SettingDescriptor(id: "candidateFontSize", title: "字級", kind: .number(13...28, step: 1, unit: " pt"), place: place(.candidates),
                          preview: .list, read: { .number($0.candidateFontSize) },
                          write: { p, v in if case .number(let n) = v { p.candidateFontSize = n } }),
        choice("candidateDensity", "間距", place(.candidates), \.candidateDensity, [(CandidateDensity.standard, "標準"), (.compact, "緊湊")],
               preview: .list),
        SettingDescriptor(id: "candidatesPerPage", title: "每頁的候選字", kind: .number(3...9, step: 1, unit: " 個"),
                          place: place(.candidates, SettingsRow.selection), preview: .list,
                          detail: { "用 \(CurrentKeys($0).selection) 選字；Return 選取反白的字" },
                          read: { .number(Double($0.candidatesPerPage)) },
                          write: { p, v in if case .number(let n) = v { p.candidatesPerPage = Int(n) } }),
        choice("selectionKeys", "選字鍵", place(.candidates), \.selectionKeys,
               [(SelectionKeySet.digits, "1 2 3 4 5 6 7 8 9"), (.homeRow, "A S D F G H J K L")], preview: .list),
        feature(.pageKeys, "翻頁"),
        toggle("pageAtListEdge", "到頁底時 ↓ 翻到下一頁", place(.candidates), \.pageAtListEdge, preview: .paging,
               detail: { $0.pageAtListEdge ? "在一頁的最後一個字按 ↓ 到下一頁，到清單結尾停住" : "↓ 停在一頁的最後一個字；用翻頁鍵翻頁" }),
        toggle("showsZhuyinHint", "顯示注音", place(.candidates, SettingsRow.showsZhuyin), \.showsZhuyinHint, preview: .list,
               detail: { _ in "要改的字的注音，列在清單上方" }),
        feature(.zhuyinRowClick, "點注音列打出注音"),
        toggle("showsAnnotations", "標出修正的讀音", place(.candidates), \.showsAnnotations, preview: .corrections,
               detail: { p in CurrentKeys(p).text(.correctionsList).map { "修正過的字旁邊以灰字標出注音，例如 \($0) 列出的字" }
                   ?? "修正過的字旁邊以灰字標出注音" }),
        feature(.singleCharacterList, "只列一個字"),
        toggle("showsListSwitches", "單字與修正按鈕", place(.candidates), \.showsListSwitches, preview: .corrections),
        choice("candidatePlacement", "位置", place(.candidates, SettingsRow.placement), \.candidatePlacement,
               [(CandidatePlacement.below, "游標下方"), (.above, "游標上方")], preview: .list,
               detail: { _ in "空間不足時換到另一邊，一律留在螢幕內" }),
        choice("panelAppearance", "外觀", place(.candidates, SettingsRow.appearance), \.panelAppearance,
               [(PanelAppearance.system, "跟隨系統"), (.light, "淺色"), (.dark, "深色")], preview: .list,
               detail: { _ in "也套用到加入自訂詞的提示與「中」「英」提示" }),
        toggle("panelAnimation", "動畫", place(.candidates, SettingsRow.motion), \.panelAnimation, preview: .paging,
               detail: { _ in NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? "淡入淡出；已開啟「減少動態效果」，不滑動"
                   : "候選字視窗出現、翻頁與「中」「英」提示的動態" }),
    ]

    // MARK: 字詞, 進階, 分析

    static let words: [SettingDescriptor] = [
        toggle("learningEnabled", "記住選過的字詞", place(.words), \.learningEnabled, detail: { _ in "下次打同樣的讀音時排在前面；只存在這台 Mac" },
               keywords: ["學習"]),
    ]

    static let advanced: [SettingDescriptor] = [
        toggle("recordsCorrections", "記錄改字", place(.advanced), \.recordsCorrections,
               detail: { _ in "改字時記下打的鍵與改前改後的字，用來改進選字" }),
        toggle("recordsCorrectionContext", "一併記錄游標前的文字", place(.advanced), \.recordsCorrectionContext),
        toggle("recordsUsageStats", "記錄使用統計", place(.analytics), \.recordsUsageStats),
    ]

    // MARK: 按鍵

    static let keys: [SettingDescriptor] = ShortcutCommand.allCases.map { command in
        SettingDescriptor(id: "keys." + command.rawValue, title: command.label, kind: .keys(command), place: command.place,
                          preview: command.section == .list ? .list : nil,
                          read: { .keys($0.bindings[command] ?? []) },
                          write: { p, v in
                              guard case .keys(let list) = v else { return }
                              var all = p.bindings
                              all[command] = list
                              p.bindings = all
                          })
    }
}

// MARK: - Snapshots and changes

/// Every setting's value at one moment.
struct SettingsSnapshot: Equatable {
    let values: [String: SettingValue]

    init(_ preferences: Preferences) {
        values = Dictionary(uniqueKeysWithValues: SettingsRegistry.all.map { ($0.id, $0.read(preferences)) })
    }
    init(values: [String: SettingValue]) { self.values = values }

    /// What differs from `self` to `other`, in the registry's order.
    func changes(to other: SettingsSnapshot) -> [SettingChange] {
        SettingsRegistry.all.compactMap { setting in
            guard let old = values[setting.id], let new = other.values[setting.id], old != new else { return nil }
            return SettingChange(setting: setting, old: old, new: new)
        }
    }
}

/// One setting changed: 原本 → 改成.
struct SettingChange: Identifiable {
    let setting: SettingDescriptor
    let old: SettingValue
    let new: SettingValue
    var id: String { setting.id }
    var oldLabel: String { setting.label(old) }
    var newLabel: String { setting.label(new) }

    func apply(to preferences: Preferences) { setting.write(preferences, new) }
    func revert(in preferences: Preferences) { setting.write(preferences, old) }
}

extension Array where Element == SettingChange {
    func apply(to preferences: Preferences) { forEach { $0.apply(to: preferences) } }
    func revert(in preferences: Preferences) { reversed().forEach { $0.revert(in: preferences) } }
}

extension Preferences {
    /// A copy of these settings in memory, to try changes on (the setup wizard's sandbox).
    func sandbox() -> Preferences {
        let copy = Preferences(defaults: MemorySettingsStorage())
        SettingsSnapshot(copy).changes(to: SettingsSnapshot(self)).apply(to: copy)
        return copy
    }

    /// Restores every registered setting (or those in `ids`) to its default.
    func restoreDefaults(_ ids: Set<String>? = nil) {
        let defaults = SettingsRegistry.defaults
        let changes = SettingsSnapshot(self).changes(to: defaults).filter { ids?.contains($0.id) ?? true }
        changes.apply(to: self)
    }
}
