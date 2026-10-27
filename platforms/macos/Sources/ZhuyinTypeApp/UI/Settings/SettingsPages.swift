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
    /// Where the command's keys are set.
    var place: SettingsPlace {
        if let feature { return feature.place }
        return SettingsPlace(.typing, row: SettingsRow.newline)
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

    /// Scrolls to a row a link pointed at, once the page shows it.
    private func reveal(_ target: String?, _ proxy: ScrollViewProxy) {
        guard let target, SettingsWindowController.shared.isShowing(pane) else { return }
        DispatchQueue.main.async {
            withAnimation(.easeInOut(duration: 0.3)) { proxy.scrollTo(target, anchor: .top) }
            model.scrollTarget = nil
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

/// A row that opens a sub-page: title, one line, the current state and ›.
struct DrillRow: View {
    @ObservedObject var model: SettingsModel
    let title: String
    var subtitle: String?
    let value: String
    let subpage: SettingsSubpage

    var body: some View {
        Button {
            model.subpage = subpage
        } label: {
            LabeledContent {
                HStack(spacing: 6) {
                    Text(value).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                    Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(.tertiary)
                }
            } label: {
                RowLabel(title, subtitle)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityHint("開啟「\(subpage.title)」")
    }
}

/// A setting with its title and a one-line description on the leading side; the control and an ⓘ button on the
/// trailing side.  Rows without an ⓘ keep its place, so the controls of a section line up.
struct OptionRow<Info: View, Control: View>: View {
    let title: String
    let subtitle: String?
    var keys: KeySection?
    @ViewBuilder let info: () -> Info
    @ViewBuilder let control: () -> Control

    var body: some View {
        LabeledContent {
            HStack(spacing: 6) {
                control()
                info().frame(width: 20)
            }
        } label: {
            FeatureLabel(title: title, subtitle: subtitle, keys: keys)
        }
    }
}

/// A row's title (with the 快捷鍵 link when it has keys) and its line.
struct FeatureLabel: View {
    let title: String
    let subtitle: String?
    var keys: KeySection?
    var dimmed = false
    var link: (title: String, place: SettingsPlace)?
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Text(title).lineLimit(2).truncationMode(.tail).help(title)
                    .foregroundStyle(dimmed ? .secondary : .primary)
                if let keys { KeysLink(section: keys) }
            }
            if let subtitle {
                Text(subtitle).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
            if let link { PlaceLink(title: link.title, place: link.place) }
        }
    }
}

/// A feature with keys: its switch (when it can be turned off) and its key fields, together.  Off, the keys are
/// dimmed and the line says where they go; turned off by another setting, the line says which and links to it.
struct FeatureRow: View {
    @ObservedObject var preferences: Preferences
    let title: String
    let subtitle: String
    var feature: KeyFeature?
    var commands: [ShortcutCommand] = []
    /// Read-only keys, for a feature without a rebindable command.
    var fixedKeys: [String]?
    let section: KeySection

    var body: some View {
        let on = feature.map(preferences.isEnabled) ?? true
        let availability = feature.flatMap(preferences.availability)
        let keys = CurrentKeys(preferences)
        let line: String = {
            if let availability { return availability.note }
            if !on {
                if feature == .zhuyinRowClick { return "已關閉：點注音列不做事" }
                let caps = commands.first.map { keys.assigned($0).joined() } ?? fixedKeys?.joined(separator: " ") ?? ""
                return "已關閉：\(caps) 交給 App"
            }
            return subtitle
        }()
        LabeledContent {
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .trailing, spacing: 6) {
                    ForEach(commands, id: \.self) { command in
                        HStack(alignment: .top, spacing: 6) {
                            if commands.count > 1 {
                                Text(command.label).font(.callout).foregroundStyle(.secondary).lineLimit(1).padding(.top, 3)
                            }
                            ShortcutField(preferences: preferences, command: command, disabled: !on,
                                          preview: DemoPinning.fieldStates[command])
                        }
                    }
                    if let fixedKeys {
                        KeyCombo(keys: fixedKeys).opacity(on && availability == nil ? 1 : 0.45)
                    }
                }
                if let feature {
                    Toggle(title, isOn: Binding(get: { preferences.isEnabled(feature) }, set: { preferences.setEnabled(feature, $0) }))
                        .labelsHidden()
                        .toggleStyle(.switch)
                        .controlSize(.small)
                        .padding(.top, 1)
                }
            }
        } label: {
            FeatureLabel(title: title, subtitle: line, keys: section, dimmed: !on || availability != nil,
                         link: availability.map { ("在「\($0.linkTitle)」設定", $0.place) })
        }
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
            Section("空白與 Esc") {
                OptionRow(title: "空白鍵", subtitle: OptionDemos.spaceSubtitle(preferences.spaceAction), keys: .composing) {
                    InfoButton(title: "空白鍵") { SpaceInfo(current: preferences.spaceAction) }
                } control: {
                    Picker("空白鍵", selection: binding(preferences, \.spaceAction)) {
                        Text("送出並輸入空白").tag(SpaceAction.commitAndSpace)
                        Text("開啟候選字視窗").tag(SpaceAction.openCandidates)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                .id(SettingsRow.space)
                OptionRow(title: "Esc", subtitle: preferences.escapeClearsComposition
                          ? "先清除注音，再按一次清除整段組字" : "只清除還沒打完的注音", keys: .composing) {
                    InfoButton(title: "Esc") {
                        VStack(alignment: .leading, spacing: 14) {
                            ChoiceDemo(title: "清除整段組字", selected: preferences.escapeClearsComposition,
                                       frames: OptionDemos.escape(clearsComposition: true), pinned: DemoPinning.mini)
                            ChoiceDemo(title: "只清除注音", selected: !preferences.escapeClearsComposition,
                                       frames: OptionDemos.escape(clearsComposition: false), pinned: DemoPinning.mini)
                        }
                    }
                } control: {
                    Picker("Esc", selection: binding(preferences, \.escapeClearsComposition)) {
                        Text("清除整段組字").tag(true)
                        Text("只清除注音").tag(false)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                .id(SettingsRow.escape)
            }
            Section("標點") {
                OptionRow(title: "標點按法", subtitle: preferences.punctuationKeys == .windows
                          ? "⇧ 加符號鍵打出鍵上的符號，例如 <" : "⇧ 加符號鍵打出中文標點，例如 ，", keys: .punctuation) {
                    InfoButton(title: "標點按法") { PunctuationTable(current: preferences.punctuationKeys) }
                } control: {
                    Picker("標點按法", selection: binding(preferences, \.punctuationKeys)) {
                        Text("Windows").tag(PunctuationKeys.windows)
                        Text("macOS").tag(PunctuationKeys.mac)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                .id(SettingsRow.punctuationKeys)
                DrillRow(model: model, title: "標點與符號", subtitle: "⌃ 加符號鍵打中文標點、⌥ 加符號鍵打鍵上的符號",
                         value: punctuationOff == 0 ? "全部開啟" : "已關閉 \(punctuationOff) 項", subpage: .punctuation)
            }
            Section("中英切換") {
                LabeledContent {
                    HStack(spacing: 10) {
                        Picker("單按 Shift 切換中英", selection: Binding(
                            get: { preferences.shiftToggle == .none ? .both : preferences.shiftToggle },
                            set: { preferences.shiftToggle = $0 })) {
                            Text("左右 Shift").tag(ShiftToggle.both)
                            Text("左 Shift").tag(ShiftToggle.left)
                            Text("右 Shift").tag(ShiftToggle.right)
                        }
                        .labelsHidden()
                        .fixedSize()
                        .disabled(!preferences.shiftSwitchEnabled)
                        Toggle("單按 Shift 切換中英", isOn: binding(preferences, \.shiftSwitchEnabled))
                            .labelsHidden().toggleStyle(.switch).controlSize(.small)
                    }
                } label: {
                    FeatureLabel(title: "單按 Shift 切換中英",
                                 subtitle: preferences.shiftSwitchEnabled ? "在可以打字的地方單按一下" : "已關閉：Shift 只當作 Shift",
                                 keys: .mode, dimmed: !preferences.shiftSwitchEnabled)
                }
                .id(SettingsRow.shiftTap)
                FeatureRow(preferences: preferences, title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出",
                           feature: .capsLockEnglish, fixedKeys: ["Caps Lock"], section: .mode)
                    .id(SettingsRow.capsLock)
                Toggle(isOn: binding(preferences, \.showModeToast)) {
                    RowLabel("中英提示", keys.modeKey.map { "按 \($0) 切換時，在游標旁顯示「中」或「英」" }
                             ?? "切換時在游標旁顯示「中」或「英」")
                }
                Toggle("開始輸入時使用英文", isOn: binding(preferences, \.startInEnglish))
            }
            Section("編輯組字") {
                FeatureRow(preferences: preferences, title: "送出並換行", subtitle: "送出組字，App 再換行",
                           commands: [.commitWithNewline], section: .composing)
                    .id(SettingsRow.newline)
                FeatureRow(preferences: preferences, title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音",
                           feature: .deleteWord, commands: [.deleteWord], section: .composing)
                    .id(SettingsRow.deleteWord)
                DrillRow(model: model, title: "所有打字的按鍵", subtitle: "注音鍵、聲調、⌫、方向鍵等固定的按鍵",
                         value: "", subpage: .typingKeys)
            }
        }
    }
}

private struct PunctuationPage: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        Form {
            Section {
                PunctuationTable(current: preferences.punctuationKeys).padding(.vertical, 6)
            } header: {
                Text("標點按法：\(preferences.punctuationKeys == .windows ? "Windows" : "macOS")")
            } footer: {
                PlaceLink(title: "標點按法在「打字」設定", place: SettingsPlace(.typing, row: SettingsRow.punctuationKeys))
            }
            Section("可以關閉的按鍵") {
                FeatureRow(preferences: preferences, title: "⌃ 加符號鍵打中文標點", subtitle: "，。、？！：；『』，兩種標點按法都能用",
                           feature: .controlPunctuation, fixedKeys: ["⌃", "符號鍵"], section: .punctuation)
                    .id(SettingsRow.controlPunctuation)
                FeatureRow(preferences: preferences, title: "⌥ 加數字或符號鍵打鍵上的符號", subtitle: ". / ; < >，兩種標點按法都能用",
                           feature: .optionSymbols, fixedKeys: ["⌥", "數字或符號鍵"], section: .punctuation)
                    .id(SettingsRow.optionSymbols)
            }
            Section("固定的按鍵") {
                LabeledContent { KeyCombo(keys: ["[", "]", "'"]) } label: { RowLabel("「 」 、", "固定") }
                LabeledContent { KeyCombo(keys: ["↓"]) } label: { RowLabel("列出同類的標點", "在標點上按") }
                LabeledContent { KeyCombo(keys: ["⇧", "空白"]) } label: { RowLabel("在組字中放一個空白", "固定") }
            }
        }
    }
}

/// The typing keys that cannot change, with their reasons.
private struct TypingKeysPage: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        Form {
            Section {
                ForEach([("拼注音", ["注音鍵"], "注音鍵的位置依標準注音鍵盤"),
                         ("聲調：二聲、三聲、四聲、輕聲", ["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"], "標準注音鍵盤的聲調鍵"),
                         ("一聲", ["空白"], "打注音時，空白完成這個音"),
                         ("送出", ["Return"], "送出整段組字"),
                         ("刪掉注音符號，或游標前的字", ["⌫"], "和一般的刪除鍵相同"),
                         ("刪掉游標後的字", ["⌦"], "和一般的刪除鍵相同"),
                         ("在組字中移動游標", ["←", "→"], "Home、End 移到開頭或結尾"),
                         ("在組字中放一個空白", ["⇧", "空白"], "空白本身用在一聲與送出")], id: \.0) { row in
                    LabeledContent { KeyCombo(keys: row.1) } label: { RowLabel(row.0, row.2) }
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
            Section("轉換") {
                OptionRow(title: "游標在句中時改", subtitle: "要改的字以底線標出") {
                    InfoButton(title: "游標在句中時改") {
                        VStack(alignment: .leading, spacing: 14) {
                            ChoiceDemo(title: "游標右邊的字", selected: preferences.candidateAnchor == .after,
                                       frames: OptionDemos.anchor(.after), pinned: DemoPinning.mini)
                            ChoiceDemo(title: "游標左邊的字", selected: preferences.candidateAnchor == .before,
                                       frames: OptionDemos.anchor(.before), pinned: DemoPinning.mini)
                        }
                    }
                } control: {
                    Picker("游標在句中時改", selection: binding(preferences, \.candidateAnchor)) {
                        Text("游標右邊的字").tag(AnchorSetting.after)
                        Text("游標左邊的字").tag(AnchorSetting.before)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                OptionRow(title: "台／臺", subtitle: "顯示與送出的寫法") {
                    Color.clear.frame(height: 1)
                } control: {
                    Picker("台／臺", selection: binding(preferences, \.characterForms)) {
                        Text("台").tag(CharacterForms.common)
                        Text("臺").tag(CharacterForms.standard)
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .fixedSize()
                }
            }
            Section("按錯鍵") {
                OptionRow(title: "按錯鍵修正", subtitle: OptionDemos.typoSubtitle(preferences.typoTolerance)) {
                    InfoButton(title: "按錯鍵修正") {
                        TypoInfo(current: preferences.typoTolerance, correctionsKey: CurrentKeys(preferences).text(.correctionsList))
                    }
                } control: {
                    Picker("按錯鍵修正", selection: binding(preferences, \.typoTolerance)) {
                        Text("關閉").tag(TypoSetting.off)
                        Text("標準").tag(TypoSetting.standard)
                        Text("積極").tag(TypoSetting.aggressive)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                .id(SettingsRow.typo)
                FeatureRow(preferences: preferences, title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原",
                           feature: .correctionsList, commands: [.correctionsList], section: .list)
                    .id(SettingsRow.corrections)
                DrillRow(model: model, title: "模糊音", subtitle: "ㄗ／ㄓ、ㄣ／ㄥ 等容易混的音",
                         value: fuzzy == 0 ? "關閉" : "已開啟 \(fuzzy) 項", subpage: .fuzzy)
                    .id(SettingsRow.fuzzy)
            }
            Section("前文") {
                DrillRow(model: model, title: "參考游標前的文字", subtitle: "依前文選字；只在記憶體中",
                         value: preferences.usesPrecedingText ? "開啟" : "關閉", subpage: .context)
            }
            Section("改字") {
                FeatureRow(preferences: preferences, title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復",
                           feature: .typeZhuyin, commands: [.typeZhuyin], section: .editing)
                    .id(SettingsRow.typeZhuyin)
                FeatureRow(preferences: preferences, title: "選取加入自訂詞", subtitle: "選取 1 到 6 個字，再按 Return",
                           feature: .selectPhrase, commands: [.selectLeft, .selectRight], section: .editing)
                    .id(SettingsRow.selectPhrase)
                FeatureRow(preferences: preferences, title: "送出後刪掉重打", subtitle: "送出後 30 秒內用 ⌫ 刪掉再打，算作改字並接著前文選字",
                           feature: .deleteAfterCommit, section: .editing)
                    .id(SettingsRow.deleteAfterCommit)
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
            Section {
                MiniDemo(frames: OptionDemos.fuzzy, pinned: DemoPinning.mini).padding(.vertical, 6)
            } footer: {
                FooterText("開啟的兩個音互相通用；兩個詞同樣常用時，讀音完全符合的排在前面。")
            }
            Section {
                ForEach(Self.pairs, id: \.0) { pair in
                    Toggle(isOn: Binding(get: { preferences.fuzzySounds.contains(pair.1) }, set: { on in
                        var sounds = preferences.fuzzySounds
                        if on { sounds.insert(pair.1) } else { sounds.remove(pair.1) }
                        preferences.fuzzySounds = sounds
                    })) { RowLabel(pair.0, pair.2) }
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
            Section {
                CandidatePreview(preferences: preferences, animated: !DemoPinning.still)
                    .padding(.vertical, 6)
            }
            Section("候選字視窗") {
                Picker(selection: binding(preferences, \.candidateLayout)) {
                    Text("直式").tag(CandidateLayout.vertical)
                    Text("橫式").tag(CandidateLayout.horizontal)
                } label: {
                    RowLabel("排列", vertical ? "↑ ↓ 移動反白，← → 改前一個或後一個字" : "← → 移動反白，↑ ↓ 翻頁")
                }
                .pickerStyle(.segmented)
                .id(SettingsRow.layout)
                LabeledContent("字級") {
                    HStack(spacing: 8) {
                        Text("字").font(.system(size: 11)).foregroundStyle(.secondary)
                        Slider(value: Binding(get: { preferences.candidateFontSize },
                                              set: { preferences.candidateFontSize = $0.rounded() }), in: 13...28)
                            .frame(minWidth: 80, maxWidth: 200)
                            .accessibilityValue("\(Int(preferences.candidateFontSize)) pt")
                        Text("字").font(.system(size: 17)).foregroundStyle(.secondary)
                        Text("\(Int(preferences.candidateFontSize)) pt")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 40, alignment: .trailing)
                    }
                }
                Picker("間距", selection: binding(preferences, \.candidateDensity)) {
                    Text("標準").tag(CandidateDensity.standard)
                    Text("緊湊").tag(CandidateDensity.compact)
                }
                .pickerStyle(.segmented)
                LabeledContent {
                    Stepper(value: binding(preferences, \.candidatesPerPage), in: 3...9) {
                        Text("\(preferences.candidatesPerPage) 個").monospacedDigit()
                    }
                } label: {
                    FeatureLabel(title: "每頁的候選字", subtitle: "用 \(keys.selection) 選字；Return 選取反白的字", keys: .list)
                }
                .id(SettingsRow.selection)
                Picker("選字鍵", selection: binding(preferences, \.selectionKeys)) {
                    Text("1 2 3 4 5 6 7 8 9").tag(SelectionKeySet.digits)
                    Text("A S D F G H J K L").tag(SelectionKeySet.homeRow)
                }
                FeatureRow(preferences: preferences, title: "翻頁", subtitle: "空白也翻到下一頁",
                           feature: .pageKeys, commands: [.nextPage, .previousPage], section: .list)
                    .id(SettingsRow.pageKeys)
            }
            Section("清單的內容") {
                Toggle(isOn: binding(preferences, \.showsZhuyinHint)) {
                    RowLabel("顯示注音", "要改的字的注音，列在清單上方")
                }
                .id(SettingsRow.showsZhuyin)
                FeatureRow(preferences: preferences, title: "點注音列打出注音",
                           subtitle: zhuyin.map { "和按 \($0) 相同；緊接著再按一次或按 Esc 恢復" } ?? "打出注音本身",
                           feature: .zhuyinRowClick, section: .list)
                    .id(SettingsRow.zhuyinRow)
                Toggle(isOn: binding(preferences, \.showsAnnotations)) {
                    RowLabel("標出修正的讀音", corrections.map { "修正過的字旁邊以灰字標出注音，例如 \($0) 列出的字" }
                             ?? "修正過的字旁邊以灰字標出注音")
                }
                FeatureRow(preferences: preferences, title: "只列一個字", subtitle: "清單只列游標處的一個字；再按一次還原",
                           feature: .singleCharacterList, commands: [.singleCharacterList], section: .list)
                    .id(SettingsRow.single)
                Toggle(isOn: binding(preferences, \.showsListSwitches)) {
                    RowLabel("單字與修正按鈕", switches.isEmpty ? "兩種清單都已關閉"
                             : "清單下方的 " + switches.joined(separator: "、") + "；關閉時只在使用中顯示")
                }
                .disabled(switches.isEmpty)
            }
            Section {
                DrillRow(model: model, title: "位置與外觀", subtitle: "套用到候選字視窗、加入自訂詞的提示與中英提示",
                         value: [preferences.candidatePlacement == .below ? "游標下方" : "游標上方",
                                 preferences.panelAppearance == .system ? "跟隨系統" : preferences.panelAppearance == .light ? "淺色" : "深色"]
                            .joined(separator: "、"),
                         subpage: .placement)
            }
        }
    }
}

private struct PlacementPage: View {
    @ObservedObject var preferences: Preferences
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        Form {
            Section {
                Picker(selection: binding(preferences, \.candidatePlacement)) {
                    Text("游標下方").tag(CandidatePlacement.below)
                    Text("游標上方").tag(CandidatePlacement.above)
                } label: {
                    RowLabel("位置", "空間不足時換到另一邊，一律留在螢幕內")
                }
                Picker(selection: binding(preferences, \.panelAppearance)) {
                    Text("跟隨系統").tag(PanelAppearance.system)
                    Text("淺色").tag(PanelAppearance.light)
                    Text("深色").tag(PanelAppearance.dark)
                } label: {
                    RowLabel("外觀", "也套用到「中」「英」提示")
                }
                Toggle(isOn: binding(preferences, \.panelAnimation)) {
                    RowLabel("動畫", reduceMotion ? "候選字視窗淡入；已開啟「減少動態效果」，不滑動"
                                              : "候選字視窗淡入，「中」「英」提示淡出")
                }
            } footer: {
                FooterText("套用到候選字視窗、加入自訂詞的提示與「中」「英」提示。")
            }
            Section {
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
            Section {
                OptionRow(title: "記住選過的字詞", subtitle: "下次打同樣的讀音時排在前面；只存在這台 Mac") {
                    InfoButton(title: "記住的字詞") { LearnedInfo() }
                } control: {
                    Toggle("記住選過的字詞", isOn: binding(preferences, \.learningEnabled))
                        .labelsHidden()
                        .toggleStyle(.switch)
                }
                DrillRow(model: model, title: "記住的字詞", subtitle: "檢視、移除、匯出或匯入",
                         value: counts.map { "\($0.learned.formatted()) 個" } ?? "", subpage: .learnedWords)
            }
            Section {
                DrillRow(model: model, title: "自訂詞", subtitle: (keys.text(.selectLeft)).map { _ in
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
            Section {
                if model.engine == nil {
                    LoadingRow()
                } else {
                    ListEditor(view: list == .learned ? model.learned.view : model.phrases.view)
                        .frame(minHeight: 340, maxHeight: .infinity)
                        .listRowInsets(EdgeInsets())
                }
            } footer: {
                if list == .phrases {
                    HStack(alignment: .firstTextBaseline) {
                        if let left = keys.text(.selectLeft), let right = keys.text(.selectRight) {
                            FooterText("自訂詞在選字時優先列出；打字時用 \(left) \(right) 選取字詞，再按 Return 也能加入。")
                        } else {
                            FooterText("自訂詞在選字時優先列出。")
                        }
                        KeysLink(section: .editing)
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
            Section {
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
