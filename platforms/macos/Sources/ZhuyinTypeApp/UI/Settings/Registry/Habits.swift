import Foundation
import ZhuyinEngine

// Ways of using a feature (styles), named sets of them (presets), and what the setup wizard teaches (lessons), all as
// data over the settings registry.  A style group is exclusive (one of: 標點按法 Windows or macOS) or combinable
// (several at once: Tab, Page Down and ⌥↓ all turn the page).  Combinable key styles become alternate keys, and the key
// conflict check decides what is allowed.  See docs/product/SETTINGS_API.md.

/// One way of using a feature.
struct HabitStyle: Identifiable {
    let id: String
    let title: String
    var detail: String?
    /// Whether the settings use this style now.
    let isActive: (Preferences) -> Bool
    /// Turns the style on (an exclusive style replaces the others of its group) or off (combinable styles only).
    let set: (Preferences, Bool) -> Void

    /// A style that is one value of a setting.
    static func value(_ settingID: String, _ value: SettingValue, title: String, detail: String? = nil) -> HabitStyle {
        HabitStyle(id: "\(settingID)=\(value)", title: title, detail: detail,
                   isActive: { SettingsRegistry.setting(settingID)?.read($0) == value },
                   set: { p, on in
                       // An exclusive style is turned off by choosing another; a switch style (on) turns off to false.
                       if on { SettingsRegistry.setting(settingID)?.write(p, value) }
                       else if value == .bool(true) { SettingsRegistry.setting(settingID)?.write(p, .bool(false)) }
                   })
    }

    /// A style that is one key of a command, alongside its others.
    static func key(_ command: ShortcutCommand, _ shortcut: Shortcut, title: String) -> HabitStyle {
        HabitStyle(id: "keys.\(command.rawValue)+\(shortcut.compact)", title: title,
                   detail: shortcut.spoken == title || shortcut.compact == title ? nil : shortcut.spoken,
                   isActive: { $0.bindings[command]?.contains(shortcut) == true },
                   set: { p, on in
                       let list = p.bindings[command] ?? []
                       if on {
                           // Through the conflict check: a key another action uses is not taken.
                           if !list.contains(shortcut), list.count < ShortcutField.maximumBindings { _ = p.bind(command, at: list.count, to: shortcut) }
                       } else if let index = list.firstIndex(of: shortcut) {
                           _ = p.unbind(command, at: index)
                       }
                   })
    }

    /// A style that is one flag of a set (模糊音).
    static func flag(_ settingID: String, _ bit: Int, title: String, detail: String? = nil) -> HabitStyle {
        func raw(_ p: Preferences) -> Int {
            if case .number(let n)? = SettingsRegistry.setting(settingID)?.read(p) { return Int(n) }
            return 0
        }
        return HabitStyle(id: "\(settingID)&\(bit)", title: title, detail: detail, isActive: { raw($0) & bit != 0 },
                          set: { p, on in SettingsRegistry.setting(settingID)?.write(p, .number(Double(on ? raw(p) | bit : raw(p) & ~bit))) })
    }

    /// A style that is a feature turned on or off.
    static func feature(_ feature: KeyFeature, title: String, detail: String? = nil) -> HabitStyle {
        HabitStyle(id: "feature.\(feature.rawValue)", title: title, detail: detail, isActive: { $0.isEnabled(feature) },
                   set: { p, on in p.setEnabled(feature, on) })
    }
}

struct HabitGroup: Identifiable {
    let id: String
    let title: String
    /// One style at a time (a choice), or any number at once.
    let exclusive: Bool
    let styles: [HabitStyle]
}

/// The ways of using each feature, from common Zhuyin input methods.  The table in SETTINGS_API.md lists them.
enum Habits {
    static let space = HabitGroup(id: "space", title: "空白鍵", exclusive: true, styles: Preferences.spaceActions.map {
        .value("spaceAction", .text($0.rawValue), title: OptionDemos.spaceLabel($0), detail: OptionDemos.spaceSubtitle($0))
    })
    static let escape = HabitGroup(id: "escape", title: "Esc", exclusive: true, styles: [
        .value("escape", .bool(true), title: "清除整段組字"), .value("escape", .bool(false), title: "只清除注音"),
    ])
    static let punctuation = HabitGroup(id: "punctuation", title: "標點按法", exclusive: true, styles: [
        .value("punctuationKeys", .text(PunctuationKeys.windows.rawValue), title: "Windows",
               detail: "⌃ 加符號鍵打中文標點，⇧ 加符號鍵打鍵上的符號"),
        .value("punctuationKeys", .text(PunctuationKeys.mac.rawValue), title: "macOS", detail: "⇧ 加符號鍵打中文標點"),
    ])
    static let modeSwitch = HabitGroup(id: "modeSwitch", title: "中英切換", exclusive: false, styles: [
        .value("shiftSwitch", .bool(true), title: "單按 Shift", detail: "在可以打字的地方單按一下"),
        .feature(.capsLockEnglish, title: "Caps Lock 打英文", detail: "開著時打英文"),
    ])
    static let layout = HabitGroup(id: "layout", title: "排列", exclusive: true, styles: [
        .value("candidateLayout", .text(CandidateLayout.vertical.rawValue), title: "直式", detail: "↑ ↓ 移動反白"),
        .value("candidateLayout", .text(CandidateLayout.horizontal.rawValue), title: "橫式", detail: "← → 移動反白，↑ ↓ 翻頁"),
    ])
    static let paging = HabitGroup(id: "paging", title: "翻頁", exclusive: false, styles: [
        .key(.nextPage, Shortcut("Tab"), title: "Tab"),
        .key(.nextPage, Shortcut("PageDown"), title: "fn ↓（Page Down）"),
        .key(.nextPage, Shortcut("↓", option: true), title: "⌥↓"),
        .value("pageAtListEdge", .bool(true), title: "在頁底按 ↓ 翻到下一頁"),
    ])
    static let selection = HabitGroup(id: "selection", title: "選字鍵", exclusive: true, styles: [
        .value("selectionKeys", .text(SelectionKeySet.digits.rawValue), title: "1–9"),
        .value("selectionKeys", .text(SelectionKeySet.homeRow.rawValue), title: "A S D F G H J K L"),
    ])
    static let anchor = HabitGroup(id: "anchor", title: "游標在句中時改", exclusive: true, styles: [
        .value("candidateAnchor", .text(AnchorSetting.after.rawValue), title: "游標右邊的字"),
        .value("candidateAnchor", .text(AnchorSetting.before.rawValue), title: "游標左邊的字"),
    ])
    static let typo = HabitGroup(id: "typo", title: "按錯鍵修正", exclusive: true, styles: TypoSetting.allCases.map {
        .value("typoTolerance", .text($0.rawValue), title: ["關閉", "標準", "積極"][TypoSetting.allCases.firstIndex(of: $0) ?? 0],
               detail: OptionDemos.typoSubtitle($0))
    })
    static let context = HabitGroup(id: "context", title: "前文", exclusive: true, styles: [
        .value("usesPrecedingText", .bool(true), title: "參考游標前的文字"), .value("usesPrecedingText", .bool(false), title: "不參考"),
    ])
    static let correctionLog = HabitGroup(id: "correctionLog", title: "記錄改字", exclusive: true, styles: [
        .value("recordsCorrections", .bool(false), title: "不記錄"), .value("recordsCorrections", .bool(true), title: "記錄改字"),
    ])

    static let fuzzy = HabitGroup(id: "fuzzy", title: "模糊音", exclusive: false, styles: [
        .flag("fuzzySounds", 1 << 0, title: "ㄗ 與 ㄓ"), .flag("fuzzySounds", 1 << 1, title: "ㄘ 與 ㄔ"),
        .flag("fuzzySounds", 1 << 2, title: "ㄙ 與 ㄕ"), .flag("fuzzySounds", 1 << 3, title: "ㄣ 與 ㄥ"),
        .flag("fuzzySounds", 1 << 4, title: "ㄢ 與 ㄤ"),
    ])
    static let density = HabitGroup(id: "density", title: "間距", exclusive: true, styles: [
        .value("candidateDensity", .text(CandidateDensity.standard.rawValue), title: "標準"),
        .value("candidateDensity", .text(CandidateDensity.compact.rawValue), title: "緊湊"),
    ])
    static let placement = HabitGroup(id: "placement", title: "位置", exclusive: true, styles: [
        .value("candidatePlacement", .text(CandidatePlacement.below.rawValue), title: "游標下方"),
        .value("candidatePlacement", .text(CandidatePlacement.above.rawValue), title: "游標上方"),
    ])

    static let all: [HabitGroup] = [space, escape, punctuation, modeSwitch, layout, paging, selection, anchor, typo, fuzzy,
                                    context, density, placement, correctionLog]
}

// MARK: - Presets

/// A named set of styles, as data.  Applying it is a list of changes, shown 原本 → 改成 before anything is written.
struct SettingsPreset: Identifiable {
    let id: String
    let title: String
    /// What it sets, one line each, for the choice: the keys it is about (drawn as keycaps) and the words.
    let lines: [(keys: [String], text: String)]
    /// The settings it sets; the others keep their values.
    let values: [String: SettingValue]
    /// Where the behaviour of the input method it follows is documented.
    var sources: [String] = []

    /// The changes from `preferences` to this preset.
    func changes(from preferences: Preferences) -> [SettingChange] {
        let now = SettingsSnapshot(preferences)
        return now.changes(to: SettingsSnapshot(values: now.values.merging(values) { _, preset in preset }))
    }

    /// Only behaviour with a source is in a preset (docs/product/PRESETS.md); the rest keeps the current value.
    static let windows = SettingsPreset(
        id: "windows", title: "Windows 注音的習慣",
        lines: [(["Shift"], "單按切換中英"), (["⌃", "，"], "打中文標點"), (["1–9"], "選字"), ([], "和目前的預設相同")],
        values: ["shiftSwitch": .bool(true), "shiftToggle": .text(ShiftToggle.both.rawValue), "startInEnglish": .bool(false),
                 "feature.controlPunctuation": .bool(true), "selectionKeys": .text(SelectionKeySet.digits.rawValue)],
        sources: ["https://support.microsoft.com/en-us/Windows/Hardware/Input-Devices/microsoft-traditional-chinese-ime"])
    static let mac = SettingsPreset(
        id: "mac", title: "macOS 注音的習慣",
        lines: [(["空白"], "開啟候選字視窗"), (["Caps Lock"], "切換中英"), (["Shift"], "不切換中英"), (["1–9"], "選字")],
        values: ["spaceAction": .text(SpaceAction.openCandidates.rawValue), "shiftSwitch": .bool(false),
                 "feature.capsLockEnglish": .bool(true), "feature.optionSymbols": .bool(true),
                 "selectionKeys": .text(SelectionKeySet.digits.rawValue)],
        sources: ["https://support.apple.com/guide/chinese-input-method/cim12992/mac"])
    static let zhuyinType = SettingsPreset(
        id: "default", title: "ZhuyinType 預設",
        lines: [(["空白"], "送出並輸入空白"), (["⌃", "，"], "打中文標點"), (["Shift"], "單按切換中英")],
        values: SettingsRegistry.defaults.values.filter { !$0.key.hasPrefix("keys.") && !$0.key.hasPrefix("feature.") }
            .merging(["shiftSwitch": .bool(true)]) { _, new in new })
    static let custom = SettingsPreset(id: "custom", title: "自訂", lines: [([], "從目前的設定開始"), ([], "每一步自己選")], values: [:])

    static let all = [windows, mac, zhuyinType, custom]
}

// MARK: - Lessons

/// What the practice field reports, for a task's check.
struct PracticeOutcome {
    /// The text in the field (sent, plus what is being composed).
    var text: String = ""
    /// What happened: "open", "page", "pick", "corrections", "mode", "phrase".
    var events: [String] = []
    /// Candidates picked from a list, in order.
    var picks: [String] = []
    // Filled in by the practice field (UI/Wizard/Practice) for the goals of PracticeGoal.
    /// The sent text only, without the composition.
    var sent: String = ""
    /// Candidates picked in place of different text (a character changed through the list).
    var corrected: [String] = []
    /// How many candidates were picked from the corrections list.
    var correctionPicks = 0
    /// User phrases added while practising.
    var phrases: [String] = []
}

struct PracticeTask {
    let prompt: String
    var hint: String?
    let isDone: (PracticeOutcome) -> Bool
}

/// What a lesson shows beside its choices: the real preview at a state, or one of the ⓘ demos.
enum LessonDemo {
    case preview(PreviewState)
    case space, punctuation, typo, fuzzy, context, correctionLog
}

/// One step of the setup wizard: a feature (or a few), how to use it, the choices, and practice.
struct Lesson: Identifiable {
    enum Level { case basic, full, optional, advanced }
    let id: String
    let title: String
    let detail: String
    let level: Level
    /// The 視窗與提示 preview state (or ⓘ demo) that shows it.
    var preview: PreviewState?
    let groups: [HabitGroup]
    let tasks: [PracticeTask]
    /// The version that added the lesson, for 「新功能導覽」.
    var since = "0.4.2"
    /// The demo beside the choices: an ⓘ demo for the lessons that have one, otherwise the real preview.
    var demo: LessonDemo? {
        switch id {
        case "typing": return .space
        case "punctuation": return .punctuation
        case "fuzzy": return .fuzzy
        case "context": return .context
        case "correctionLog": return .correctionLog
        default: return preview.map(LessonDemo.preview)
        }
    }

    /// The lessons in order; 快速設定 takes the basic ones.
    static let all: [Lesson] = [
        Lesson(id: "typing", title: "打字與空白鍵", detail: "打注音時空白是一聲；打完以後，空白依你的選擇送出或開啟候選字視窗。",
               level: .basic, preview: .composing, groups: [Habits.space],
               tasks: [PracticeTask(prompt: "打出「今天天氣很好」", hint: "ㄐㄧㄣ ㄊㄧㄢ ㄊㄧㄢ ㄑㄧˋ ㄏㄣˇ ㄏㄠˇ") { $0.text.contains("今天天氣很好") }]),
        Lesson(id: "punctuation", title: "標點", detail: "按住修飾鍵再按符號鍵打出標點。選一種按法，在下面試打。",
               level: .basic, groups: [Habits.punctuation],
               tasks: [PracticeTask(prompt: "打出「好，謝謝。」") { $0.text.contains("，") && $0.text.contains("。") }]),
        Lesson(id: "modeSwitch", title: "中英切換", detail: "切換到英文打字，再切回中文；正在組的字會先送出。",
               level: .basic, preview: .mode, groups: [Habits.modeSwitch],
               tasks: [PracticeTask(prompt: "切換到英文打「OK」，再切回中文打「好」") {
                   $0.text.contains("OK好") && $0.events.filter { $0 == "mode" }.count >= 2
               }]),
        Lesson(id: "paging", title: "選字與翻頁", detail: "這些翻頁方式同時有效；不用的可以關掉。",
               level: .basic, preview: .paging, groups: [Habits.layout, Habits.paging, Habits.selection],
               tasks: [PracticeTask(prompt: "打 ㄇㄧㄥˊ，翻到下一頁，選「暝」") { $0.picks.contains("暝") && $0.events.contains("page") }]),
        Lesson(id: "editing", title: "改字", detail: "按 ↓ 列出其他的字，選一個換掉；游標可以先移到要改的字旁邊。",
               level: .basic, preview: .list, groups: [Habits.anchor],
               tasks: [PracticeTask(prompt: "打「謝謝你」，再把「你」改成「妳」") { $0.text.contains("謝謝妳") && $0.picks.contains("妳") }]),
        Lesson(id: "typo", title: "按錯鍵與修正清單", detail: "按到相鄰的鍵、多按或漏按時試著修正；修正的字也在修正清單中。",
               level: .full, preview: .corrections, groups: [Habits.typo],
               tasks: [PracticeTask(prompt: "照提示按錯一個鍵，再用修正清單選到正確的字") { $0.events.contains("corrections") && !$0.picks.isEmpty }]),
        Lesson(id: "fuzzy", title: "模糊音", detail: "容易混淆的兩個音互相通用；讀音完全符合的仍排在前面。",
               level: .optional, groups: [Habits.fuzzy],
               tasks: [PracticeTask(prompt: "開啟「ㄙ 與 ㄕ」，用 ㄙㄨ ㄈㄨˊ 打出「舒服」") { $0.text.contains("舒服") }]),
        Lesson(id: "context", title: "前文", detail: "參考游標前的文字選字；只在記憶體中，不會存下。",
               level: .full, groups: [Habits.context],
               tasks: [PracticeTask(prompt: "在「天氣」後面接著打出「很好」") { $0.text.contains("天氣很好") }]),
        Lesson(id: "phrase", title: "加入自訂詞", detail: "選取 1 到 6 個字，再按 Return 加入自訂詞。",
               level: .full, preview: .phrase, groups: [],
               tasks: [PracticeTask(prompt: "選取「王小明」，按 Return 加入") { $0.events.contains("phrase") }]),
        Lesson(id: "look", title: "候選字視窗的樣子", detail: "字級、間距與位置；預覽就是真正的候選字視窗。",
               level: .full, preview: .list, groups: [Habits.density, Habits.placement, Habits.selection], tasks: []),
        Lesson(id: "correctionLog", title: "改字記錄", detail: "記下改字的過程，用來改進選字；只存在這台 Mac，可以隨時關閉或清除。",
               level: .advanced, groups: [Habits.correctionLog], tasks: []),
    ]
}
