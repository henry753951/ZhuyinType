import AppKit
import SwiftUI
import ZhuyinEngine

// The pages of the settings window as docs/product/SETTINGS_IA.md lays them out: a few pages in the sidebar, the
// common settings on each, details one level down (rows ending in ›), and each feature's switch, keys and
// explanation together.

/// Pages one level down from a sidebar page.
enum SettingsSubpage: String, CaseIterable, Identifiable {
    case typingKeys, fuzzy, context, learnedWords, phrases, performance, apps, sources, releaseNotes, removal
    var id: String { rawValue }

    var pane: SettingsPane {
        switch self {
        case .typingKeys: return .typing
        case .context: return .choosing
        case .fuzzy: return .corrections
        case .learnedWords, .phrases: return .words
        case .performance: return .analytics
        case .apps: return .advanced
        case .sources, .releaseNotes, .removal: return .about
        }
    }

    var title: String {
        switch self {
        case .typingKeys: return "所有打字的按鍵"
        case .fuzzy: return "模糊音"
        case .context: return "前文"
        case .learnedWords: return "記住的字詞"
        case .phrases: return "自訂詞"
        case .performance: return "效能"
        case .apps: return "在候選字視窗顯示組字的 App"
        case .sources: return "授權與資料來源"
        case .releaseNotes: return "更新內容"
        case .removal: return "移除 ZhuyinType"
        }
    }
}

/// Somewhere in the settings window: a page, maybe one of its sub-pages, maybe a row (or a section,
/// `RowSections.headerID`) to arrive at.
struct SettingsPlace: Equatable {
    var pane: SettingsPane
    var subpage: SettingsSubpage?
    var row: String?
    init(_ pane: SettingsPane, subpage: SettingsSubpage? = nil, row: String? = nil) {
        self.pane = pane
        self.subpage = subpage
        self.row = row
    }

    /// A whole section of a page: its header near the top, its card lit.
    static func section(_ pane: SettingsPane, _ title: String, subpage: SettingsSubpage? = nil) -> SettingsPlace {
        SettingsPlace(pane, subpage: subpage, row: RowSections.headerID(title))
    }

    /// The section of the 快捷鍵 page with the keys of `section` (named as its page).
    static func keysSection(_ section: KeySection) -> SettingsPlace { .section(.keys, section.title) }

    /// The same place in this version's layout (SETTINGS_IA.md §4.4): a sub-page under the page it now belongs to, and
    /// a row of 0.7.0's 打字, 選字 or 視窗與提示 on the page it moved to.
    var resolved: SettingsPlace {
        var place = self
        if let subpage, subpage.pane != pane { place.pane = subpage.pane }
        if place.subpage == nil, let row, let home = SettingsLinks.movedRows[row], home != place.pane,
           SettingsLinks.formerPanes.contains(place.pane) {
            place.pane = home
        }
        return place
    }
}

/// Rows other pages link to, by id.
enum SettingsRow {
    static let space = "space", escape = "escape", punctuationKeys = "punctuationKeys", shiftTap = "shiftTap"
    static let shiftSide = "shiftSide", startInEnglish = "startInEnglish"
    static let capsLock = "capsLock", newline = "newline", deleteWord = "deleteWord", corrections = "corrections"
    static let keyOrder = "keyOrder"
    static let typeZhuyin = "typeZhuyin", selectPhrase = "selectPhrase", deleteAfterCommit = "deleteAfterCommit"
    static let pageKeys = "pageKeys", single = "single", zhuyinRow = "zhuyinRow", selection = "selection"
    static let layout = "layout", typo = "typo", fuzzy = "fuzzy", controlPunctuation = "controlPunctuation"
    static let optionSymbols = "optionSymbols", showsZhuyin = "showsZhuyin", otherReadings = "otherReadings"
    static let placement = "placement", appearance = "appearance", accentColor = "accentColor", motion = "motion"
    static let modeToast = "modeToast"
    static let showCorrections = "showCorrections", undoCorrection = "undoCorrection"
    static let associatedPhrases = "nextWordSuggestions", outputScript = "outputScript", candidateAnchor = "candidateAnchor"
    static let showUpdateNotes = "showUpdateNotes"
    static let symbolPicker = "symbolPicker", context = "context", learning = "learning", pauseLearning = "pauseLearning"
}

/// Where each feature with keys is set.
extension KeyFeature {
    var place: SettingsPlace {
        switch self {
        case .typeZhuyin: return SettingsPlace(.corrections, row: SettingsRow.typeZhuyin)
        case .zhuyinRowClick: return SettingsPlace(.candidates, row: SettingsRow.zhuyinRow)
        case .singleCharacterList: return SettingsPlace(.candidates, row: SettingsRow.single)
        case .correctionsList: return SettingsPlace(.corrections, row: SettingsRow.corrections)
        case .selectPhrase: return SettingsPlace(.words, row: SettingsRow.selectPhrase)
        case .pageKeys: return SettingsPlace(.candidates, row: SettingsRow.pageKeys)
        case .deleteWord: return SettingsPlace(.typing, row: SettingsRow.deleteWord)
        case .controlPunctuation: return SettingsPlace(.punctuation, row: SettingsRow.controlPunctuation)
        case .optionSymbols: return SettingsPlace(.punctuation, row: SettingsRow.optionSymbols)
        case .capsLockEnglish: return SettingsPlace(.mode, row: SettingsRow.capsLock)
        case .deleteAfterCommit: return SettingsPlace(.corrections, row: SettingsRow.deleteAfterCommit)
        case .symbolPicker: return SettingsPlace(.punctuation, row: SettingsRow.symbolPicker)
        }
    }
}

extension ShortcutCommand {
    /// The id of the command's 「快捷鍵」 row, on its feature's page and on the 快捷鍵 page alike.
    var keysRow: String { "keys-" + rawValue }

    /// Where the command's keys are set: its 「快捷鍵」 row on the page of its feature.
    var place: SettingsPlace {
        let pane: SettingsPane = {
            switch self {
            case .previousCharacter, .nextCharacter, .nextPage, .previousPage, .singleCharacterList: return .candidates
            case .commitWithNewline, .deleteWord, .insertSpace: return .typing
            case .togglePunctuationWidth, .symbolPicker: return .punctuation
            case .quickSettings: return .mode
            case .correctionsList, .typeZhuyin: return .corrections
            case .selectLeft, .selectRight, .addSelectedText, .blockCandidate, .forgetCandidate, .addCandidatePhrase: return .words
            }
        }()
        return SettingsPlace(pane, row: keysRow)
    }

    /// The command's row on the 快捷鍵 page.
    var overviewPlace: SettingsPlace { SettingsPlace(.keys, row: keysRow) }
}

// MARK: - The page host

struct SettingsPaneView: View {
    let pane: SettingsPane
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences = .shared

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Group {
            ZStack(alignment: .top) {
                if let pinned = DemoPinning.navigation, let subpage = model.subpage, subpage.pane == pane {
                    // The UI previews: a push (or pop) drawn still at `progress`.
                    let shown = pinned.push ? pinned.progress : 1 - pinned.progress
                    page().modifier(NavigationMotion(offset: -12, visible: 1 - shown))
                        .environment(\.settingsPage, RowSections.pageKey(pane, nil))
                    subpageView(subpage).modifier(NavigationMotion(offset: 24, visible: shown))
                } else if let subpage = model.subpage, subpage.pane == pane {
                    subpageView(subpage)
                        .id(subpage)
                        .transition(reduceMotion ? .opacity : .modifier(active: NavigationMotion(offset: 24, visible: 0),
                                                                         identity: NavigationMotion(offset: 24, visible: 1)))
                } else {
                    page()
                        .environment(\.settingsPage, RowSections.pageKey(pane, nil))
                        .transition(reduceMotion ? .opacity : .modifier(active: NavigationMotion(offset: -12, visible: 0),
                                                                         identity: NavigationMotion(offset: -12, visible: 1)))
                }
            }
            // The UI previews draw offscreen, where animations do not run: they show pushes through DemoPinning.
            .animation(SettingsWindowController.isPreview ? nil
                       : reduceMotion ? .easeInOut(duration: 0.18) : .easeOut(duration: 0.22), value: model.subpage)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .onChange(of: model.scrollTarget) { _, target in reveal(target) }
            .onAppear { reveal(model.scrollTarget) }
        }
    }

    /// The arrival cue (SETTINGS_IA.md §4.1): scroll so the target's section header and the row are both in view (a row
    /// deep in a long section: the row about a third down), then tint the row (or every row of a section that is the
    /// target) and light the section header, once: in 0.2 s, held 0.8 s, out 0.8 s.  The row takes the keyboard focus
    /// (`ArrivalFocus`).  Under Reduce Motion the scroll jumps and the tint only fades.  A row that is not drawn yet (a
    /// sub-page sliding in, a folded part unfolding) is waited for, briefly.
    private func reveal(_ target: String?, attempt: Int = 0) {
        guard let target, SettingsWindowController.shared.isShowing(pane) else { return }
        let page = RowSections.pageKey(pane, model.subpage)
        // A sub-page that was just pushed, or rows that just unfolded, are drawn a moment later.
        guard RowSections.drawn[page]?.contains(target) == true || attempt >= 12 else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { reveal(target, attempt: attempt + 1) }
            return
        }
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let isSection = target.hasPrefix("section-")
        let section = isSection ? String(target.dropFirst("section-".count)) : RowSections.section(of: target, page: page)
        let nearHeader = isSection || section.map { RowSections.position(of: target, in: $0, page: page) < 3 } ?? false
        // A moment after the page settled: rows inserted with the page (a folded part opening) are laid out by then.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            if let section, nearHeader {
                model.scrollRequest = ScrollRequest(id: RowSections.headerID(section), fraction: 0.08, animated: !reduceMotion)
            } else {
                model.scrollRequest = ScrollRequest(id: target, fraction: 0.33, animated: !reduceMotion)
            }
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
        case .analytics: AnalyticsPane(preferences: preferences, model: model)
        case .typing: TypingPane(preferences: preferences, model: model)
        case .punctuation: PunctuationPane(preferences: preferences)
        case .mode: ModePane(preferences: preferences)
        case .choosing: ChoosingPane(preferences: preferences, model: model)
        case .corrections: CorrectionsPane(preferences: preferences, model: model)
        case .words: WordsPane(preferences: preferences, model: model)
        case .candidates: CandidatesPane(preferences: preferences, model: model)
        case .colors: ColorsPane(preferences: preferences)
        case .keys: KeysPane(preferences: preferences, model: model)
        case .health: HealthPage()
        case .advanced: AdvancedPane(model: model, preferences: preferences)
        case .about: AboutPane(model: model, preferences: preferences)
        }
    }

    /// A sub-page, with its pinned bar (drawn by its `SettingsForm`).
    private func subpageView(_ subpage: SettingsSubpage) -> some View {
        page(subpage).environment(\.subpageHeader, SubpageHeaderInfo(parent: pane.title, title: subpage.title) { model.subpage = nil })
            .environment(\.settingsPage, RowSections.pageKey(pane, subpage))
    }

    @ViewBuilder private func page(_ subpage: SettingsSubpage) -> some View {
        switch subpage {
        case .typingKeys: TypingKeysPage(preferences: preferences)
        case .fuzzy: FuzzyPage(preferences: preferences)
        case .context: ContextPane(preferences: preferences, pinnedScene: DemoPinning.context)
        case .learnedWords: WordListPage(model: model, list: .learned, preferences: preferences)
        case .phrases: WordListPage(model: model, list: .phrases, preferences: preferences)
        case .performance: PerformancePage()
        case .apps: AppsPage(model: model)
        case .sources: SourcesPane()
        case .releaseNotes: ReleaseNotesPane(preferences: preferences, model: model)
        case .removal: RemovalPane()
        }
    }
}

/// The bar pinned above a sub-page: back to the page it belongs to, and its title, on the content column's left edge.
/// It is clear at the top; once content scrolls under it, it takes the bar material and a hairline below.
struct SubpageBar: View {
    let info: SubpageHeaderInfo
    let scrolled: Bool
    var body: some View {
        HStack(spacing: 8) {
            Button(action: info.back) {
                HStack(spacing: 3) {
                    Image(systemName: "chevron.left").font(.system(size: 12, weight: .semibold))
                    Text(info.parent).lineLimit(1)
                }
            }
            .buttonStyle(.borderless)
            .keyboardShortcut("[", modifiers: .command)
            .help("回到「\(info.parent)」")
            Text("／").foregroundStyle(.tertiary)
            Text(info.title).font(.headline).lineLimit(1).truncationMode(.tail).help(info.title)
            Spacer(minLength: 0)
        }
        // Lined up with the cards' content below: the page inset, then the card inset, in the same column.
        .padding(.horizontal, RowMetrics.cardInset)
        .frame(maxWidth: RowMetrics.maxWidth)
        .padding(.horizontal, RowMetrics.pageInset)
        .padding(.vertical, RowMetrics.barPadding)
        .frame(maxWidth: .infinity)
        .background {
            ZStack(alignment: .bottom) {
                Rectangle().fill(.bar).opacity(scrolled ? 1 : 0)
                Rectangle().fill(Color(nsColor: .textBackgroundColor)).opacity(scrolled ? 0 : 1)
                Rectangle().fill(Color.primary.opacity(0.12)).frame(height: 1).opacity(scrolled ? 1 : 0)
            }
            // Up to the window's top edge, so nothing scrolls into view between the toolbar and the bar.
            .ignoresSafeArea(edges: .top)
            .animation(.easeOut(duration: 0.15), value: scrolled)
        }
        .background(BarProbe())
    }
}

private struct BarProbe: View {
    var body: some View {
        if RowGeometry.recording {
            GeometryReader { proxy in
                let _ = RowGeometry.recordBar(proxy.frame(in: .global))
                Color.clear
            }
        }
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
                // Every key of the feature's commands; Page Up and Page Down page the list anyway.
                let handed = commands.flatMap { preferences.bindings[$0] ?? [] }
                    .filter { $0 != Shortcut("PageUp") && $0 != Shortcut("PageDown") }.map(\.compact)
                return handed.isEmpty ? subtitle : "關閉時，\(handed.joined(separator: "、")) 交給 App"
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
        if on { CommandKeyRows(preferences: preferences, commands: commands) }
    }
}

/// The indented 「快捷鍵」 rows of a setting: one per command (named after the command when there are several), with
/// 「還原預設」 while the keys differ from the default.
struct CommandKeyRows: View {
    @ObservedObject var preferences: Preferences
    let commands: [ShortcutCommand]
    var body: some View {
        ForEach(commands, id: \.self) { command in
            let row = AnyHashable("keysrow-" + command.rawValue)
            SettingRow(title: commands.count > 1 ? command.label : "快捷鍵", indent: true,
                       marked: preferences.isCustomized(command), anchor: command.keysRow) {
                ShortcutField(preferences: preferences, command: command, row: row, preview: DemoPinning.fieldStates[command])
            }
            .hoverTracked(row)
        }
    }
}

/// The page's one link to the overview of all keys, at its foot: to the overview's section named as the page.
struct KeysOverviewLink: View {
    let section: KeySection
    var body: some View {
        PlaceLink(title: "在快捷鍵總覽中查看全部按鍵", place: .keysSection(section))
            .frame(maxWidth: .infinity, alignment: .leading)
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

struct EscapeInfo: View {
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

struct AnchorInfo: View {
    let current: AnchorSetting
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ChoiceDemo(title: "游標右邊的字", selected: current == .after, frames: OptionDemos.anchor(.after), pinned: DemoPinning.mini)
            ChoiceDemo(title: "游標左邊的字", selected: current == .before, frames: OptionDemos.anchor(.before), pinned: DemoPinning.mini)
        }
    }
}

/// The typing keys that cannot change, with their reasons.
private struct TypingKeysPage: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        SettingsForm {
            SettingsSection {
                ForEach([("拼注音", ["注音鍵"], "注音鍵的位置依標準注音鍵盤"),
                         ("聲調：二聲、三聲、四聲、輕聲", ["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"], "標準注音鍵盤的聲調鍵"),
                         ("一聲", ["空白"], "打注音時，空白完成這個音"),
                         ("送出", ["Return"], "送出整段組字"),
                         ("刪掉注音符號，或游標前的字", ["⌫"], "和一般的刪除鍵相同"),
                         ("刪掉游標後的字", ["⌦"], "和一般的刪除鍵相同"),
                         ("在組字中移動游標", ["←", "→"], "Home、End 移到開頭或結尾")], id: \.0) { row in
                    SettingRow(title: row.0, subtitle: row.2) { KeyCombo(keys: row.1) }
                }
            } footer: {
                FooterText("這些按鍵用在打字本身，無法更改。")
            }
        }
    }
}

// MARK: - 選字

private struct FuzzyPage: View {
    @ObservedObject var preferences: Preferences
    private static let pairs: [(String, FuzzySounds, String)] = [
        ("ㄗ 與 ㄓ", .zZh, "例如 ㄗㄨˇ 也打得出「主」"), ("ㄘ 與 ㄔ", .cCh, "例如 ㄘ 也打得出「吃」"),
        ("ㄙ 與 ㄕ", .sSh, "例如 ㄙㄨ ㄈㄨˊ 也打得出「舒服」"), ("ㄣ 與 ㄥ", .enEng, "例如 ㄒㄧㄣ 也打得出「星」"),
        ("ㄢ 與 ㄤ", .anAng, "例如 ㄈㄢˊ 也打得出「房」"),
    ]
    var body: some View {
        SettingsForm {
            SettingsSection {
                MiniDemo(frames: OptionDemos.fuzzy, pinned: DemoPinning.mini)
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

// MARK: - 候選字視窗

struct CandidatesPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Whether the preview at the top is in view; when it is not, a change shows a floating preview for a moment.
    @State private var previewInView = true
    @State private var flash: UUID?
    @State private var flashAt: CGPoint?
    @StateObject private var director = PreviewDirector(name: "candidates")
    @State private var shownSettings: PreviewSettings?

    var body: some View {
        let keys = CurrentKeys(preferences)
        let zhuyin = keys.text(.typeZhuyin)
        let corrections = keys.text(.correctionsList)
        let single = keys.text(.singleCharacterList)
        let switches = [single.map { "\($0) 單字" }, corrections.map { "\($0) 修正" }].compactMap { $0 }
        SettingsForm {
            // One preview at the top, with the real window; it plays its states, and a change below switches it to the
            // state that shows it.  It scrolls away like the rest.
            SettingsSection {
                UnifiedPreview(preferences: preferences, director: director)
                    .background(GeometryReader { proxy in
                        Color.clear.preference(key: PreviewInView.self, value: proxy.frame(in: .named("settingsPage")).maxY > 60)
                    })
            }
            SettingsSection("候選字清單") {
                RegistryRow("candidateLayout", preferences, segmented: true)
                CommandKeyRows(preferences: preferences, commands: [.previousCharacter, .nextCharacter])
                SettingRow(title: "字級", value: "\(Int(preferences.candidateFontSize)) pt") {
                    Slider(value: Binding(get: { preferences.candidateFontSize },
                                          set: { preferences.candidateFontSize = $0.rounded() }), in: Preferences.candidateFontSizes) {
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
                RegistryRow("candidateDensity", preferences, segmented: true)
                RegistryRow("candidatesPerPage", preferences)
                RegistryRow("selectionKeys", preferences)
            }
            SettingsSection("翻頁") {
                FeatureRow(preferences: preferences, title: "翻頁", subtitle: "空白也翻到下一頁",
                           feature: .pageKeys, commands: [.nextPage, .previousPage], anchor: SettingsRow.pageKeys)
                RegistryRow("pageAtListEdge", preferences)
            }
            SettingsSection("清單的內容") {
                RegistryRow("showsZhuyinHint", preferences)
                FeatureRow(preferences: preferences, title: "點注音列打出注音",
                           subtitle: zhuyin.map { "和按 \($0) 相同；緊接著再按一次或按 Esc 恢復" } ?? "打出注音本身",
                           feature: .zhuyinRowClick, anchor: SettingsRow.zhuyinRow)
                RegistryRow("showsAnnotations", preferences)
                FeatureRow(preferences: preferences, title: "只列一個字", subtitle: "清單只列游標處的一個字；再按一次還原",
                           feature: .singleCharacterList, commands: [.singleCharacterList], anchor: SettingsRow.single)
                SettingRow(title: "單字與修正按鈕", subtitle: switches.isEmpty ? "兩種清單都已關閉"
                           : "清單下方的 " + switches.joined(separator: "、") + "；關閉時只在使用中顯示", dimmed: switches.isEmpty) {
                    RowSwitch(title: "單字與修正按鈕", isOn: binding(preferences, \.showsListSwitches)).disabled(switches.isEmpty)
                }
            }
            SettingsSection("位置與動畫") {
                RegistryRow("candidatePlacement", preferences)
                RegistryRow("panelAnimation", preferences)
            } footer: {
                KeysOverviewLink(section: .candidates)
            }
        }
        .onPreferenceChange(PreviewInView.self) { previewInView = $0 }
        .onAppear { shownSettings = PreviewSettings(preferences) }
        .onReceive(preferences.objectWillChange) { _ in
            // After the change lands: switch the preview to the state that shows it, and if the preview is out of view,
            // show that state beside the pointer for a moment.
            DispatchQueue.main.async {
                let now = PreviewSettings(preferences)
                guard let before = shownSettings, before != now else { return }
                shownSettings = now
                if let state = PreviewFlows.state(showing: before, now) { director.show(state) }
                if before.perPage != now.perPage { director.fullPageUntil = Date().addingTimeInterval(PreviewDirector.hold) }
                showFlash()
            }
        }
        .overlay { flashOverlay }
    }

    private func showFlash() {
        guard !previewInView, !SettingsWindowController.isPreview, let window = SettingsWindowController.shared.sheetWindow,
              let content = window.contentView else { return }
        let point = window.convertPoint(fromScreen: NSEvent.mouseLocation)
        let id = UUID()
        flashAt = CGPoint(x: point.x, y: content.bounds.height - point.y)
        withAnimation(reduceMotion ? .easeInOut(duration: 0.15) : .spring(response: 0.25, dampingFraction: 0.9)) { flash = id }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            guard flash == id else { return }
            withAnimation(.easeOut(duration: 0.3)) { flash = nil }
        }
    }

    /// The floating preview after a change: beside the pointer, above it when there is room, taking no layout space.
    @ViewBuilder private var flashOverlay: some View {
        let pinned = DemoPinning.flashAt
        if flash != nil || pinned != nil {
            GeometryReader { proxy in
                let frame = proxy.frame(in: .global)
                let at = pinned ?? flashAt ?? CGPoint(x: frame.midX, y: frame.midY)
                let size = CGSize(width: 300, height: 190)
                let x = min(max(at.x - frame.minX, size.width / 2 + 12), proxy.size.width - size.width / 2 - 12)
                let above = at.y - frame.minY - size.height - 24
                let y = above > 8 ? above + size.height / 2 : at.y - frame.minY + 24 + size.height / 2
                CandidateScenePreview(preferences: preferences, scene: flashScene)
                    .padding(10)
                    .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.regularMaterial))
                    .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.quaternary, lineWidth: 0.5))
                    .shadow(color: .black.opacity(0.18), radius: 12, y: 4)
                    .fixedSize()
                    .position(x: x, y: min(y, proxy.size.height - size.height / 2 - 8))
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: 0.96)))
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }
}

extension CandidatesPane {
    /// The floating preview shows the state the change switched to, at its last moment.
    fileprivate var flashScene: PreviewScene? {
        let flows = PreviewFlows(preferences: preferences)
        let state = DemoPinning.previewState?.state ?? director.state
        return flows.frames(state).last
    }
}

private struct PreviewInView: PreferenceKey {
    static let defaultValue = true
    static func reduce(value: inout Bool, nextValue: () -> Bool) { value = nextValue() }
}


// MARK: - 字詞

private struct WordListPage: View {
    enum List { case learned, phrases }
    @ObservedObject var model: SettingsModel
    let list: List
    @ObservedObject var preferences: Preferences

    var body: some View {
        let keys = CurrentKeys(preferences)
        SettingsForm {
            if model.engine == nil {
                SettingsSection { LoadingRow() }
            } else {
                ListSection(list: list == .learned ? model.learned.list : model.phrases.list)
                Group {
                    if list == .phrases {
                        if let left = keys.text(.selectLeft), let right = keys.text(.selectRight) {
                            FooterText("自訂詞在選字時優先列出；打字時用 \(left) \(right) 選取字詞，再按 Return 也能加入。")
                        } else {
                            FooterText("自訂詞在選字時優先列出。")
                        }
                        FooterText(wordActionsFooter(keys, phrases: true))
                    } else {
                        FooterText("選過的字詞下次排在前面；設為「不記住」的字詞之後不再學。")
                        FooterText(wordActionsFooter(keys, phrases: false))
                    }
                }
                .padding(.horizontal, RowMetrics.cardInset)
                .padding(.top, RowMetrics.footerGap - RowMetrics.sectionSpacing)
            }
        }
    }
}

/// How the candidate window's word actions reach these lists, with the keys as they are bound now.
private func wordActionsFooter(_ keys: CurrentKeys, phrases: Bool) -> String {
    if phrases {
        let selected = keys.text(.addSelectedText).map { "；在 App 裡選取文字再按 \($0)，也會打開加入自訂詞的視窗" } ?? ""
        let add = keys.text(.addCandidatePhrase).map { "（\($0)）" } ?? ""
        return "在候選字上按右鍵選「加入自訂詞」\(add)，把反白的字詞加入\(selected)。"
    }
    let block = keys.text(.blockCandidate).map { "（\($0)）" } ?? ""
    let forget = keys.text(.forgetCandidate).map { "（\($0)）" } ?? ""
    return "在候選字上按右鍵：「不要再推薦」\(block)之後不再自動選用這個字詞，列在這裡並標示「不推薦」，移除後恢復；"
        + "「忘記這個選字」\(forget)忘記選過它。5 秒內按 ⌃Z 或點候選字視窗的提示可以復原。"
}

// MARK: - 進階

private struct AppsPage: View {
    @ObservedObject var model: SettingsModel
    var body: some View {
        SettingsForm {
            ListSection(list: model.apps.list)
            FooterText("清單中的 App，組字顯示在候選字視窗上方。")
                .padding(.horizontal, RowMetrics.cardInset)
                .padding(.top, RowMetrics.footerGap - RowMetrics.sectionSpacing)
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
