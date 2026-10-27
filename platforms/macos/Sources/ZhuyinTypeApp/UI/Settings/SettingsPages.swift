import AppKit
import SwiftUI
import ZhuyinCore

// The pages of the settings window as docs/product/SETTINGS_IA.md lays them out: a few pages in the sidebar, the
// common settings on each, details one level down (rows ending in ›), and each feature's switch, keys and
// explanation together.

/// Pages one level down from a sidebar page.
enum SettingsSubpage: String, CaseIterable, Identifiable {
    case punctuation, typingKeys, fuzzy, context, placement, learnedWords, phrases, performance, apps
    var id: String { rawValue }

    var pane: SettingsPane {
        switch self {
        case .punctuation, .typingKeys: return .typing
        case .fuzzy, .context: return .choosing
        case .placement: return .candidates
        case .learnedWords, .phrases: return .words
        case .performance: return .analytics
        case .apps: return .advanced
        }
    }

    var title: String {
        switch self {
        case .punctuation: return "標點與符號"
        case .typingKeys: return "所有打字的按鍵"
        case .fuzzy: return "模糊音"
        case .context: return "前文"
        case .placement: return "位置與外觀"
        case .learnedWords: return "記住的字詞"
        case .phrases: return "自訂詞"
        case .performance: return "效能"
        case .apps: return "在候選字視窗顯示組字的 App"
        }
    }
}

/// Somewhere in the settings window: a page, maybe one of its sub-pages, maybe a row to scroll to.
struct SettingsPlace: Equatable {
    var pane: SettingsPane
    var subpage: SettingsSubpage?
    var row: String?
    init(_ pane: SettingsPane, subpage: SettingsSubpage? = nil, row: String? = nil) {
        self.pane = pane
        self.subpage = subpage
        self.row = row
    }
}

/// Rows other pages link to, by id.
enum SettingsRow {
    static let space = "space", escape = "escape", punctuationKeys = "punctuationKeys", shiftTap = "shiftTap"
    static let capsLock = "capsLock", newline = "newline", deleteWord = "deleteWord", corrections = "corrections"
    static let typeZhuyin = "typeZhuyin", selectPhrase = "selectPhrase", deleteAfterCommit = "deleteAfterCommit"
    static let pageKeys = "pageKeys", single = "single", zhuyinRow = "zhuyinRow", selection = "selection"
    static let layout = "layout", typo = "typo", fuzzy = "fuzzy", controlPunctuation = "controlPunctuation"
    static let optionSymbols = "optionSymbols", showsZhuyin = "showsZhuyin"
}

/// Where each feature with keys is set.
extension KeyFeature {
    var place: SettingsPlace {
        switch self {
        case .typeZhuyin: return SettingsPlace(.choosing, row: SettingsRow.typeZhuyin)
        case .zhuyinRowClick: return SettingsPlace(.candidates, row: SettingsRow.zhuyinRow)
        case .singleCharacterList: return SettingsPlace(.candidates, row: SettingsRow.single)
        case .correctionsList: return SettingsPlace(.choosing, row: SettingsRow.corrections)
        case .selectPhrase: return SettingsPlace(.choosing, row: SettingsRow.selectPhrase)
        case .pageKeys: return SettingsPlace(.candidates, row: SettingsRow.pageKeys)
        case .deleteWord: return SettingsPlace(.typing, row: SettingsRow.deleteWord)
        case .controlPunctuation: return SettingsPlace(.typing, subpage: .punctuation, row: SettingsRow.controlPunctuation)
        case .optionSymbols: return SettingsPlace(.typing, subpage: .punctuation, row: SettingsRow.optionSymbols)
        case .capsLockEnglish: return SettingsPlace(.typing, row: SettingsRow.capsLock)
        case .deleteAfterCommit: return SettingsPlace(.choosing, row: SettingsRow.deleteAfterCommit)
        }
    }
}

extension ShortcutCommand {
    /// The id of the command's 「快捷鍵」 row on its feature's page.
    var keysRow: String { "keys-" + rawValue }

    /// Where the command's keys are set: its 「快捷鍵」 row.
    var place: SettingsPlace {
        var place = feature?.place ?? SettingsPlace(.typing, row: SettingsRow.newline)
        place.row = keysRow
        return place
    }
}

// MARK: - The page host

struct SettingsPaneView: View {
    let pane: SettingsPane
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences = .shared

    var body: some View {
        ScrollViewReader { proxy in
            VStack(spacing: 0) {
                if let subpage = model.subpage, subpage.pane == pane {
                    SubpageHeader(parent: pane.title, title: subpage.title) { model.subpage = nil }
                    page(subpage)
                } else {
                    page()
                }
            }
            .formStyle(.grouped)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .onChange(of: model.scrollTarget) { _, target in reveal(target, proxy) }
            .onAppear { reveal(model.scrollTarget, proxy) }
        }
    }

    /// The arrival cue: scroll so the target's section header and the row are both in view (the row about a third
    /// down), then tint the row (or every row of a section that is the target) and light the section header, once:
    /// in 0.2 s, held 0.8 s, out 0.8 s.  Under Reduce Motion the scroll jumps and the tint only fades.
    private func reveal(_ target: String?, _ proxy: ScrollViewProxy) {
        guard let target, SettingsWindowController.shared.isShowing(pane) else { return }
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let isSection = target.hasPrefix("section-")
        let section = isSection ? String(target.dropFirst("section-".count)) : RowSections.map[target]
        DispatchQueue.main.async {
            let scroll = {
                if let section { proxy.scrollTo(RowSections.headerID(section), anchor: UnitPoint(x: 0.5, y: 0.08)) }
                else { proxy.scrollTo(target, anchor: UnitPoint(x: 0.5, y: 0.33)) }
            }
            if reduceMotion { scroll() } else { withAnimation(.easeInOut(duration: 0.35)) { scroll() } }
            model.scrollTarget = nil
            DispatchQueue.main.asyncAfter(deadline: .now() + (reduceMotion ? 0.05 : 0.4)) {
                withAnimation(.easeOut(duration: 0.2)) {
                    model.arrivedRow = isSection ? nil : target
                    model.arrivedSection = section
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                    guard model.arrivedRow == (isSection ? nil : target), model.arrivedSection == section else { return }
                    withAnimation(.easeInOut(duration: 0.8)) {
                        model.arrivedRow = nil
                        model.arrivedSection = nil
                    }
                }
            }
        }
    }

    @ViewBuilder private func page() -> some View {
        switch pane {
        case .intro: IntroPane(preferences: preferences, model: model)
        case .typing: TypingPane(preferences: preferences, model: model)
        case .choosing: ChoosingPane(preferences: preferences, model: model)
        case .candidates: CandidatesPane(preferences: preferences, model: model)
        case .words: WordsPane(preferences: preferences, model: model)
        case .analytics: AnalyticsPane(preferences: preferences, model: model)
        case .keys: KeysPane(preferences: preferences, model: model)
        case .advanced: AdvancedPane(model: model, preferences: preferences)
        case .about: AboutPane()
        }
    }

    @ViewBuilder private func page(_ subpage: SettingsSubpage) -> some View {
        switch subpage {
        case .punctuation: PunctuationPage(preferences: preferences)
        case .typingKeys: TypingKeysPage(preferences: preferences)
        case .fuzzy: FuzzyPage(preferences: preferences)
        case .context: ContextPane(preferences: preferences, pinnedScene: DemoPinning.context)
        case .placement: PlacementPage(preferences: preferences)
        case .learnedWords: WordListPage(model: model, list: .learned, preferences: preferences)
        case .phrases: WordListPage(model: model, list: .phrases, preferences: preferences)
        case .performance: PerformancePage()
        case .apps: AppsPage(model: model)
        }
    }
}

/// The bar above a sub-page: back to the page it belongs to, and its title.
private struct SubpageHeader: View {
    let parent: String
    let title: String
    let back: () -> Void
    var body: some View {
        HStack(spacing: 8) {
            Button(action: back) {
                HStack(spacing: 3) {
                    Image(systemName: "chevron.left").font(.system(size: 12, weight: .semibold))
                    Text(parent).lineLimit(1)
                }
            }
            .buttonStyle(.borderless)
            .keyboardShortcut("[", modifiers: .command)
            .help("回到「\(parent)」")
            Text("／").foregroundStyle(.tertiary)
            Text(title).font(.headline).lineLimit(1).truncationMode(.tail).help(title)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 20)
        .padding(.top, 14)
        .padding(.bottom, 2)
    }
}

// MARK: - Rows

/// A row that opens a sub-page: title, description, the current state and ›.
struct DrillRow: View {
    @ObservedObject var model: SettingsModel
    let title: String
    var subtitle: String?
    let value: String
    let subpage: SettingsSubpage
    var anchor: String?

    var body: some View {
        SettingLinkRow(title: title, subtitle: subtitle, value: value.isEmpty ? nil : value, anchor: anchor) {
            model.subpage = subpage
        }
    }
}

/// A feature: its row (title, description, switch), then one indented row per command, 「快捷鍵」, with the key
/// field and, when the keys are not the defaults, 「還原預設」.  The key rows show only while the feature is on.
/// Turned off by another setting, the description says which and links to it.
struct FeatureRow: View {
    @ObservedObject var preferences: Preferences
    let title: String
    let subtitle: String
    var feature: KeyFeature?
    var commands: [ShortcutCommand] = []
    var anchor: String?

    var body: some View {
        let on = feature.map(preferences.isEnabled) ?? true
        let availability = feature.flatMap(preferences.availability)
        let line: String = {
            if let availability { return availability.note }
            if !on {
                if feature == .zhuyinRowClick { return "關閉時，點注音列不做事" }
                return commands.isEmpty ? subtitle : "關閉時，\(CurrentKeys(preferences).assigned(commands[0]).joined()) 交給 App"
            }
            return subtitle
        }()
        SettingRow(title: title, subtitle: line, dimmed: availability != nil,
                   link: availability.map { ("在「\($0.linkTitle)」設定", $0.place) }, anchor: anchor) {
            if let feature {
                RowSwitch(title: title, isOn: Binding(get: { preferences.isEnabled(feature) },
                                                      set: { preferences.setEnabled(feature, $0) }))
            }
        }
        if on {
            ForEach(commands, id: \.self) { command in
                SettingRow(title: commands.count > 1 ? command.label : "快捷鍵", indent: true, anchor: command.keysRow) {
                    if preferences.bindings[command] != command.defaultBindings {
                        Button("還原預設") { _ = preferences.resetBindings(command) }
                            .buttonStyle(.link)
                            .help("還原成 \(command.defaultShortcut.keycaps.joined())")
                    }
                    ShortcutField(preferences: preferences, command: command, preview: DemoPinning.fieldStates[command])
                }
            }
        }
    }
}

/// The page's one link to the overview of all keys, at its foot.
struct KeysOverviewLink: View {
    var body: some View {
        PlaceLink(title: "在快捷鍵總覽中查看全部按鍵", place: SettingsPlace(.keys))
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 6)
    }
}

/// A link to a place in the settings window.
struct PlaceLink: View {
    let title: String
    let place: SettingsPlace
    var body: some View {
        Button {
            SettingsWindowController.shared.open(place)
        } label: {
            HStack(spacing: 2) {
                Text(title).lineLimit(1)
                Image(systemName: "chevron.right").font(.system(size: 9, weight: .semibold))
            }
            .font(.callout)
        }
        .buttonStyle(.link)
    }
}

// MARK: - 打字

private struct TypingPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        let keys = CurrentKeys(preferences)
        let punctuationOff = [KeyFeature.controlPunctuation, .optionSymbols].filter { !preferences.isEnabled($0) }.count
        Form {
            SettingsSection("空白與 Esc") {
                SettingRow(title: "空白鍵", subtitle: OptionDemos.spaceSubtitle(preferences.spaceAction),
                           accessory: .info(title: "空白鍵", content: AnyView(SpaceInfo(current: preferences.spaceAction))),
                           anchor: SettingsRow.space) {
                    RowPicker(title: "空白鍵", selection: binding(preferences, \.spaceAction),
                              options: [SpaceAction.commitAndSpace, .insertSpace, .openCandidates].map { (OptionDemos.spaceLabel($0), $0) })
                }
                SettingRow(title: "Esc", subtitle: preferences.escapeClearsComposition
                           ? "先清除注音，再按一次清除整段組字" : "只清除還沒打完的注音",
                           accessory: .info(title: "Esc", content: AnyView(EscapeInfo(clears: preferences.escapeClearsComposition))),
                           anchor: SettingsRow.escape) {
                    RowPicker(title: "Esc", selection: binding(preferences, \.escapeClearsComposition),
                              options: [("清除整段組字", true), ("只清除注音", false)])
                }
            }
            SettingsSection("標點") {
                SettingRow(title: "標點按法", subtitle: preferences.punctuationKeys == .windows
                           ? "⇧ 加符號鍵打出鍵上的符號，例如 <" : "⇧ 加符號鍵打出中文標點，例如 ，",
                           accessory: .info(title: "標點按法", content: AnyView(PunctuationTable(current: preferences.punctuationKeys))),
                           anchor: SettingsRow.punctuationKeys) {
                    RowPicker(title: "標點按法", selection: binding(preferences, \.punctuationKeys),
                              options: [("Windows", PunctuationKeys.windows), ("macOS", .mac)])
                }
                DrillRow(model: model, title: "標點與符號", subtitle: "⌃ 加符號鍵打中文標點、⌥ 加符號鍵打鍵上的符號",
                         value: punctuationOff == 0 ? "全部開啟" : "已關閉 \(punctuationOff) 項", subpage: .punctuation)
            }
            SettingsSection("中英切換") {
                SettingRow(title: "單按 Shift 切換中英",
                           subtitle: preferences.shiftSwitchEnabled ? "在可以打字的地方單按一下" : "關閉時，Shift 只當作 Shift",
                           anchor: SettingsRow.shiftTap) {
                    RowSwitch(title: "單按 Shift 切換中英", isOn: binding(preferences, \.shiftSwitchEnabled))
                }
                if preferences.shiftSwitchEnabled {
                    SettingRow(title: "使用的 Shift", indent: true) {
                        RowPicker(title: "使用的 Shift", selection: Binding(
                            get: { preferences.shiftToggle == .none ? .both : preferences.shiftToggle },
                            set: { preferences.shiftToggle = $0 }),
                                  options: [("左右 Shift", ShiftToggle.both), ("左 Shift", .left), ("右 Shift", .right)])
                    }
                }
                FeatureRow(preferences: preferences, title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出",
                           feature: .capsLockEnglish, anchor: SettingsRow.capsLock)
                SettingRow(title: "中英提示", subtitle: keys.modeKey.map { "按 \($0) 切換時，在游標旁顯示「中」或「英」" }
                           ?? "切換時在游標旁顯示「中」或「英」") {
                    RowSwitch(title: "中英提示", isOn: binding(preferences, \.showModeToast))
                }
                SettingRow(title: "開始輸入時使用英文") {
                    RowSwitch(title: "開始輸入時使用英文", isOn: binding(preferences, \.startInEnglish))
                }
            }
            SettingsSection("編輯組字") {
                FeatureRow(preferences: preferences, title: "送出並換行", subtitle: "送出組字，App 再換行",
                           commands: [.commitWithNewline], anchor: SettingsRow.newline)
                FeatureRow(preferences: preferences, title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音",
                           feature: .deleteWord, commands: [.deleteWord], anchor: SettingsRow.deleteWord)
                DrillRow(model: model, title: "所有打字的按鍵", subtitle: "注音鍵、聲調、⌫、方向鍵等固定的按鍵",
                         value: "", subpage: .typingKeys)
            } footer: {
                KeysOverviewLink()
            }
        }
    }
}

private struct EscapeInfo: View {
    let clears: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ChoiceDemo(title: "清除整段組字", selected: clears, frames: OptionDemos.escape(clearsComposition: true),
                       pinned: DemoPinning.mini)
            ChoiceDemo(title: "只清除注音", selected: !clears, frames: OptionDemos.escape(clearsComposition: false),
                       pinned: DemoPinning.mini)
        }
    }
}

private struct AnchorInfo: View {
    let current: AnchorSetting
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ChoiceDemo(title: "游標右邊的字", selected: current == .after, frames: OptionDemos.anchor(.after), pinned: DemoPinning.mini)
            ChoiceDemo(title: "游標左邊的字", selected: current == .before, frames: OptionDemos.anchor(.before), pinned: DemoPinning.mini)
        }
    }
}

private struct PunctuationPage: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        Form {
            SettingsSection("標點按法：\(preferences.punctuationKeys == .windows ? "Windows" : "macOS")") {
                PunctuationTable(current: preferences.punctuationKeys).padding(.vertical, 6)
            } footer: {
                PlaceLink(title: "標點按法在「打字」設定", place: SettingsPlace(.typing, row: SettingsRow.punctuationKeys))
            }
            SettingsSection("可以關閉的按鍵") {
                FeatureRow(preferences: preferences, title: "⌃ 加符號鍵打中文標點", subtitle: "，。、？！：；『』，兩種標點按法都能用",
                           feature: .controlPunctuation, anchor: SettingsRow.controlPunctuation)
                FeatureRow(preferences: preferences, title: "⌥ 加數字或符號鍵打鍵上的符號", subtitle: ". / ; < >，兩種標點按法都能用",
                           feature: .optionSymbols, anchor: SettingsRow.optionSymbols)
            }
            SettingsSection("固定的按鍵") {
                SettingRow(title: "「 」 、", subtitle: "固定") { KeyCombo(keys: ["[", "]", "'"]) }
                SettingRow(title: "列出同類的標點", subtitle: "在標點上按") { KeyCombo(keys: ["↓"]) }
                SettingRow(title: "在組字中放一個空白", subtitle: "固定") { KeyCombo(keys: ["⇧", "空白"]) }
            }
        }
    }
}

/// The typing keys that cannot change, with their reasons.
private struct TypingKeysPage: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        Form {
            SettingsSection {
                ForEach([("拼注音", ["注音鍵"], "注音鍵的位置依標準注音鍵盤"),
                         ("聲調：二聲、三聲、四聲、輕聲", ["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"], "標準注音鍵盤的聲調鍵"),
                         ("一聲", ["空白"], "打注音時，空白完成這個音"),
                         ("送出", ["Return"], "送出整段組字"),
                         ("刪掉注音符號，或游標前的字", ["⌫"], "和一般的刪除鍵相同"),
                         ("刪掉游標後的字", ["⌦"], "和一般的刪除鍵相同"),
                         ("在組字中移動游標", ["←", "→"], "Home、End 移到開頭或結尾"),
                         ("在組字中放一個空白", ["⇧", "空白"], "空白本身用在一聲與送出")], id: \.0) { row in
                    SettingRow(title: row.0, subtitle: row.2) { KeyCombo(keys: row.1) }
                }
            } footer: {
                FooterText("這些按鍵用在打字本身，無法更改。")
            }
        }
    }
}

// MARK: - 選字

private struct ChoosingPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        let fuzzy = [FuzzySounds.zZh, .cCh, .sSh, .enEng, .anAng].filter { preferences.fuzzySounds.contains($0) }.count
        Form {
            SettingsSection("轉換") {
                SettingRow(title: "游標在句中時改", subtitle: "要改的字以底線標出",
                           accessory: .info(title: "游標在句中時改", content: AnyView(AnchorInfo(current: preferences.candidateAnchor)))) {
                    RowPicker(title: "游標在句中時改", selection: binding(preferences, \.candidateAnchor),
                              options: [("游標右邊的字", AnchorSetting.after), ("游標左邊的字", .before)])
                }
                SettingRow(title: "台／臺", subtitle: "顯示與送出的寫法") {
                    RowPicker(title: "台／臺", selection: binding(preferences, \.characterForms),
                              options: [("台", CharacterForms.common), ("臺", .standard)], segmented: true)
                }
            }
            SettingsSection("按錯鍵") {
                SettingRow(title: "按錯鍵修正", subtitle: OptionDemos.typoSubtitle(preferences.typoTolerance),
                           accessory: .info(title: "按錯鍵修正", content: AnyView(TypoInfo(
                            current: preferences.typoTolerance, correctionsKey: CurrentKeys(preferences).text(.correctionsList)))),
                           anchor: SettingsRow.typo) {
                    RowPicker(title: "按錯鍵修正", selection: binding(preferences, \.typoTolerance),
                              options: [("關閉", TypoSetting.off), ("標準", .standard), ("積極", .aggressive)])
                }
                FeatureRow(preferences: preferences, title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原",
                           feature: .correctionsList, commands: [.correctionsList], anchor: SettingsRow.corrections)
                DrillRow(model: model, title: "模糊音", subtitle: "ㄗ／ㄓ、ㄣ／ㄥ 等容易混的音",
                         value: fuzzy == 0 ? "關閉" : "已開啟 \(fuzzy) 項", subpage: .fuzzy, anchor: SettingsRow.fuzzy)
            }
            SettingsSection("前文") {
                DrillRow(model: model, title: "參考游標前的文字", subtitle: "依前文選字；只在記憶體中",
                         value: preferences.usesPrecedingText ? "開啟" : "關閉", subpage: .context)
            }
            SettingsSection("改字") {
                FeatureRow(preferences: preferences, title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復",
                           feature: .typeZhuyin, commands: [.typeZhuyin], anchor: SettingsRow.typeZhuyin)
                FeatureRow(preferences: preferences, title: "選取加入自訂詞", subtitle: "選取 1 到 6 個字，再按 Return",
                           feature: .selectPhrase, commands: [.selectLeft, .selectRight], anchor: SettingsRow.selectPhrase)
                FeatureRow(preferences: preferences, title: "送出後刪掉重打",
                           subtitle: "送出後 30 秒內用 ⌫ 刪掉再打，算作改字並接著前文選字",
                           feature: .deleteAfterCommit, anchor: SettingsRow.deleteAfterCommit)
            } footer: {
                KeysOverviewLink()
            }
        }
    }
}

private struct FuzzyPage: View {
    @ObservedObject var preferences: Preferences
    private static let pairs: [(String, FuzzySounds, String)] = [
        ("ㄗ 與 ㄓ", .zZh, "例如 ㄗㄨˇ 也打得出「主」"), ("ㄘ 與 ㄔ", .cCh, "例如 ㄘ 也打得出「吃」"),
        ("ㄙ 與 ㄕ", .sSh, "例如 ㄙㄨ ㄈㄨˊ 也打得出「舒服」"), ("ㄣ 與 ㄥ", .enEng, "例如 ㄒㄧㄣ 也打得出「星」"),
        ("ㄢ 與 ㄤ", .anAng, "例如 ㄈㄢˊ 也打得出「房」"),
    ]
    var body: some View {
        Form {
            SettingsSection {
                MiniDemo(frames: OptionDemos.fuzzy, pinned: DemoPinning.mini).padding(.vertical, 6)
            } footer: {
                FooterText("開啟的兩個音互相通用；兩個詞同樣常用時，讀音完全符合的排在前面。")
            }
            SettingsSection {
                ForEach(Self.pairs, id: \.0) { pair in
                    SettingRow(title: pair.0, subtitle: pair.2) {
                        RowSwitch(title: pair.0, isOn: Binding(get: { preferences.fuzzySounds.contains(pair.1) }, set: { on in
                            var sounds = preferences.fuzzySounds
                            if on { sounds.insert(pair.1) } else { sounds.remove(pair.1) }
                            preferences.fuzzySounds = sounds
                        }))
                    }
                }
            }
        }
    }
}

// MARK: - 視窗與提示

struct CandidatesPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        let keys = CurrentKeys(preferences)
        let zhuyin = keys.text(.typeZhuyin)
        let corrections = keys.text(.correctionsList)
        let single = keys.text(.singleCharacterList)
        let switches = [single.map { "\($0) 單字" }, corrections.map { "\($0) 修正" }].compactMap { $0 }
        let vertical = preferences.candidateLayout == .vertical
        Form {
            SettingsSection {
                CandidatePreview(preferences: preferences, animated: !DemoPinning.still)
                    .padding(.vertical, 6)
            }
            SettingsSection("候選字視窗") {
                SettingRow(title: "排列", subtitle: vertical ? "↑ ↓ 移動反白，← → 改前一個或後一個字" : "← → 移動反白，↑ ↓ 翻頁",
                           anchor: SettingsRow.layout) {
                    RowPicker(title: "排列", selection: binding(preferences, \.candidateLayout),
                              options: [("直式", CandidateLayout.vertical), ("橫式", .horizontal)], segmented: true)
                }
                SettingRow(title: "字級", value: "\(Int(preferences.candidateFontSize)) pt") {
                    Slider(value: Binding(get: { preferences.candidateFontSize },
                                          set: { preferences.candidateFontSize = $0.rounded() }), in: 13...28) {
                        Text("字級")
                    } minimumValueLabel: {
                        Text("字").font(.system(size: 11))
                    } maximumValueLabel: {
                        Text("字").font(.system(size: 17))
                    }
                    .labelsHidden()
                    .frame(minWidth: 120, maxWidth: 200)
                    .accessibilityValue("\(Int(preferences.candidateFontSize)) pt")
                }
                SettingRow(title: "間距") {
                    RowPicker(title: "間距", selection: binding(preferences, \.candidateDensity),
                              options: [("標準", CandidateDensity.standard), ("緊湊", .compact)], segmented: true)
                }
                SettingRow(title: "每頁的候選字", subtitle: "用 \(keys.selection) 選字；Return 選取反白的字",
                           value: "\(preferences.candidatesPerPage) 個", anchor: SettingsRow.selection) {
                    Stepper("每頁的候選字", value: binding(preferences, \.candidatesPerPage), in: 3...9).labelsHidden()
                }
                SettingRow(title: "選字鍵") {
                    RowPicker(title: "選字鍵", selection: binding(preferences, \.selectionKeys),
                              options: [("1 2 3 4 5 6 7 8 9", SelectionKeySet.digits), ("A S D F G H J K L", .homeRow)])
                }
                FeatureRow(preferences: preferences, title: "翻頁", subtitle: "空白也翻到下一頁",
                           feature: .pageKeys, commands: [.nextPage, .previousPage], anchor: SettingsRow.pageKeys)
            }
            SettingsSection("清單的內容") {
                SettingRow(title: "顯示注音", subtitle: "要改的字的注音，列在清單上方", anchor: SettingsRow.showsZhuyin) {
                    RowSwitch(title: "顯示注音", isOn: binding(preferences, \.showsZhuyinHint))
                }
                FeatureRow(preferences: preferences, title: "點注音列打出注音",
                           subtitle: zhuyin.map { "和按 \($0) 相同；緊接著再按一次或按 Esc 恢復" } ?? "打出注音本身",
                           feature: .zhuyinRowClick, anchor: SettingsRow.zhuyinRow)
                SettingRow(title: "標出修正的讀音", subtitle: corrections.map { "修正過的字旁邊以灰字標出注音，例如 \($0) 列出的字" }
                           ?? "修正過的字旁邊以灰字標出注音") {
                    RowSwitch(title: "標出修正的讀音", isOn: binding(preferences, \.showsAnnotations))
                }
                FeatureRow(preferences: preferences, title: "只列一個字", subtitle: "清單只列游標處的一個字；再按一次還原",
                           feature: .singleCharacterList, commands: [.singleCharacterList], anchor: SettingsRow.single)
                SettingRow(title: "單字與修正按鈕", subtitle: switches.isEmpty ? "兩種清單都已關閉"
                           : "清單下方的 " + switches.joined(separator: "、") + "；關閉時只在使用中顯示", dimmed: switches.isEmpty) {
                    RowSwitch(title: "單字與修正按鈕", isOn: binding(preferences, \.showsListSwitches)).disabled(switches.isEmpty)
                }
            }
            SettingsSection {
                DrillRow(model: model, title: "位置與外觀", subtitle: "套用到候選字視窗、加入自訂詞的提示與中英提示",
                         value: [preferences.candidatePlacement == .below ? "游標下方" : "游標上方",
                                 preferences.panelAppearance == .system ? "跟隨系統" : preferences.panelAppearance == .light ? "淺色" : "深色"]
                            .joined(separator: "、"),
                         subpage: .placement)
            } footer: {
                KeysOverviewLink()
            }
        }
    }
}

private struct PlacementPage: View {
    @ObservedObject var preferences: Preferences
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        Form {
            SettingsSection {
                SettingRow(title: "位置", subtitle: "空間不足時換到另一邊，一律留在螢幕內") {
                    RowPicker(title: "位置", selection: binding(preferences, \.candidatePlacement),
                              options: [("游標下方", CandidatePlacement.below), ("游標上方", .above)])
                }
                SettingRow(title: "外觀", subtitle: "也套用到「中」「英」提示") {
                    RowPicker(title: "外觀", selection: binding(preferences, \.panelAppearance),
                              options: [("跟隨系統", PanelAppearance.system), ("淺色", .light), ("深色", .dark)])
                }
                SettingRow(title: "動畫", subtitle: reduceMotion ? "候選字視窗淡入；已開啟「減少動態效果」，不滑動"
                           : "候選字視窗淡入，「中」「英」提示淡出") {
                    RowSwitch(title: "動畫", isOn: binding(preferences, \.panelAnimation))
                }
            } footer: {
                FooterText("套用到候選字視窗、加入自訂詞的提示與「中」「英」提示。")
            }
            SettingsSection {
                CandidatePreview(preferences: preferences, animated: !DemoPinning.still)
                    .padding(.vertical, 6)
            }
        }
    }
}

// MARK: - 字詞

private struct WordsPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    @State private var counts: (learned: Int, phrases: Int)?

    var body: some View {
        let keys = CurrentKeys(preferences)
        Form {
            SettingsSection {
                SettingRow(title: "記住選過的字詞", subtitle: "下次打同樣的讀音時排在前面；只存在這台 Mac",
                           accessory: .info(title: "記住的字詞", content: AnyView(LearnedInfo()))) {
                    RowSwitch(title: "記住選過的字詞", isOn: binding(preferences, \.learningEnabled))
                }
                DrillRow(model: model, title: "記住的字詞", subtitle: "檢視、移除、匯出或匯入",
                         value: counts.map { "\($0.learned.formatted()) 個" } ?? "", subpage: .learnedWords)
            }
            SettingsSection {
                DrillRow(model: model, title: "自訂詞", subtitle: keys.text(.selectLeft).map { _ in
                    "選字時優先列出；打字時也能選取後加入" } ?? "選字時優先列出",
                         value: counts.map { "\($0.phrases.formatted()) 個" } ?? "", subpage: .phrases)
            }
        }
        .onAppear(perform: count)
        .onChange(of: model.engine != nil) { _, _ in count() }
    }

    private func count() {
        guard let engine = model.engine else { return }
        counts = (engine.learnedEntries().count, engine.userPhrases().count)
    }
}

private struct WordListPage: View {
    enum List { case learned, phrases }
    @ObservedObject var model: SettingsModel
    let list: List
    @ObservedObject var preferences: Preferences

    var body: some View {
        let keys = CurrentKeys(preferences)
        Form {
            SettingsSection {
                if model.engine == nil {
                    LoadingRow()
                } else {
                    ListEditor(view: list == .learned ? model.learned.view : model.phrases.view)
                        .frame(minHeight: 340, maxHeight: .infinity)
                        .listRowInsets(EdgeInsets())
                }
            } footer: {
                if list == .phrases {
                    if let left = keys.text(.selectLeft), let right = keys.text(.selectRight) {
                        FooterText("自訂詞在選字時優先列出；打字時用 \(left) \(right) 選取字詞，再按 Return 也能加入。")
                    } else {
                        FooterText("自訂詞在選字時優先列出。")
                    }
                } else {
                    FooterText("依次數由多到少排列；設為「不記住」的字詞之後不再學。")
                }
            }
        }
    }
}

// MARK: - 進階

private struct AppsPage: View {
    @ObservedObject var model: SettingsModel
    var body: some View {
        Form {
            SettingsSection {
                ListEditor(view: model.apps.view)
                    .frame(minHeight: 240, maxHeight: .infinity)
                    .listRowInsets(EdgeInsets())
            } footer: {
                FooterText("清單中的 App，組字顯示在候選字視窗上方。")
            }
        }
    }
}

struct LoadingRow: View {
    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text("正在載入詞庫…").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 120)
    }
}
