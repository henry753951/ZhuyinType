import AppKit
import SwiftUI
import ZhuyinCore

struct SettingsPaneView: View {
    let pane: SettingsPane
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences = .shared

    var body: some View {
        Group {
            switch pane {
            case .general: GeneralPane(preferences: preferences)
            case .keys: KeysPane(preferences: preferences)
            case .candidates: CandidatesPane(preferences: preferences)
            case .context: ContextPane(preferences: preferences, pinnedScene: DemoPinning.context)
            case .phrases: PhrasesPane(model: model, preferences: preferences)
            case .learned: LearnedPane(model: model, preferences: preferences)
            case .advanced: AdvancedPane(model: model, preferences: preferences)
            case .about: AboutPane()
            }
        }
        .formStyle(.grouped)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }
}

/// Fixed frames for the demos, so the UI previews show a chosen moment.  Nil while the app runs.
enum DemoPinning {
    /// Draw keycaps at rest, without their press.
    static var still = false
    static var context: Int?
    static var mini: Int?
}

/// A setting with its title and a one-line description on the leading side; the control and an ⓘ button on the
/// trailing side.  Rows without an ⓘ keep its place, so the controls of a section line up.
private struct OptionRow<Info: View, Control: View>: View {
    let title: String
    let subtitle: String?
    @ViewBuilder let info: () -> Info
    @ViewBuilder let control: () -> Control

    var body: some View {
        LabeledContent {
            HStack(spacing: 6) {
                control()
                info().frame(width: 20)
            }
        } label: {
            RowLabel(title, subtitle)
        }
    }
}

// MARK: - 一般

private struct GeneralPane: View {
    @ObservedObject var preferences: Preferences
    static let fuzzyPairs: [(label: String, set: FuzzySounds)] = [
        ("ㄗ 與 ㄓ", .zZh), ("ㄘ 與 ㄔ", .cCh), ("ㄙ 與 ㄕ", .sSh), ("ㄣ 與 ㄥ", .enEng), ("ㄢ 與 ㄤ", .anAng),
    ]

    var body: some View {
        let corrections = preferences.shortcuts[.correctionsList]?.compact ?? "⇧↑"
        Form {
            Section("按鍵") {
                OptionRow(title: "空白鍵", subtitle: OptionDemos.spaceSubtitle(preferences.spaceAction)) {
                    InfoButton(title: "空白鍵") { SpaceInfo(current: preferences.spaceAction) }
                } control: {
                    Picker("空白鍵", selection: binding(preferences, \.spaceAction)) {
                        Text("開啟候選字視窗").tag(SpaceAction.openCandidates)
                        Text("送出文字").tag(SpaceAction.commit)
                        Text("送出並輸入空白").tag(SpaceAction.commitAndSpace)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                OptionRow(title: "Esc", subtitle: preferences.escapeClearsComposition
                          ? "先清除注音，再按一次清除整段組字" : "只清除還沒打完的注音") {
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
                OptionRow(title: "標點按法", subtitle: preferences.punctuationKeys == .windows
                          ? "⇧ 加符號鍵打出鍵上的符號，例如 <" : "⇧ 加符號鍵打出中文標點，例如 ，") {
                    InfoButton(title: "標點按法") { PunctuationTable(current: preferences.punctuationKeys) }
                } control: {
                    Picker("標點按法", selection: binding(preferences, \.punctuationKeys)) {
                        Text("Windows").tag(PunctuationKeys.windows)
                        Text("macOS").tag(PunctuationKeys.mac)
                    }
                    .labelsHidden()
                    .fixedSize()
                }
            }
            Section("選字") {
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
                OptionRow(title: "按錯鍵修正", subtitle: OptionDemos.typoSubtitle(preferences.typoTolerance)) {
                    InfoButton(title: "按錯鍵修正") {
                        TypoInfo(current: preferences.typoTolerance, correctionsKey: corrections)
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
                PaneLink(title: "參考游標前的文字", subtitle: "依前文選字；只在記憶體中",
                         value: preferences.usesPrecedingText ? "開啟" : "關閉", pane: .context)
            }
            Section {
                ForEach(Self.fuzzyPairs, id: \.label) { pair in
                    Toggle(pair.label, isOn: Binding(
                        get: { preferences.fuzzySounds.contains(pair.set) },
                        set: { on in
                            var sounds = preferences.fuzzySounds
                            if on { sounds.insert(pair.set) } else { sounds.remove(pair.set) }
                            preferences.fuzzySounds = sounds
                        }))
                }
            } header: {
                HStack {
                    Text("模糊音")
                    Spacer()
                    InfoButton(title: "模糊音") { FuzzyInfo() }
                }
            } footer: {
                FooterText("開啟的兩個音互相通用。")
            }
            Section("中英切換") {
                Picker("切換鍵", selection: binding(preferences, \.shiftToggle)) {
                    Text("左右 Shift").tag(ShiftToggle.both)
                    Text("左 Shift").tag(ShiftToggle.left)
                    Text("右 Shift").tag(ShiftToggle.right)
                    Text("不使用").tag(ShiftToggle.none)
                }
                Toggle(isOn: binding(preferences, \.showModeToast)) {
                    RowLabel("中英提示", CurrentKeys(preferences).shiftTap.map { "單按 \($0) 切換時，在游標旁顯示「中」或「英」" }
                             ?? "按 Caps Lock 時，在游標旁顯示「中」或「英」")
                }
                Toggle("開始輸入時使用英文", isOn: binding(preferences, \.startInEnglish))
            }
        }
    }
}

/// A row that opens another page, like the rows of System Settings that end in a chevron.
private struct PaneLink: View {
    let title: String
    let subtitle: String
    let value: String
    let pane: SettingsPane

    var body: some View {
        Button {
            SettingsWindowController.shared.select(pane)
        } label: {
            LabeledContent {
                HStack(spacing: 6) {
                    Text(value).foregroundStyle(.secondary)
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.tertiary)
                }
            } label: {
                RowLabel(title, subtitle)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityHint("開啟「\(pane.title)」")
    }
}

struct SpaceInfo: View {
    let current: SpaceAction
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("打注音時，空白是一聲；打完以後，空白依這個設定。")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ChoiceDemo(title: "開啟候選字視窗", selected: current == .openCandidates,
                       frames: OptionDemos.space(.openCandidates), pinned: DemoPinning.mini)
            ChoiceDemo(title: "送出文字", selected: current == .commit,
                       frames: OptionDemos.space(.commit), pinned: DemoPinning.mini)
            ChoiceDemo(title: "送出並輸入空白", selected: current == .commitAndSpace,
                       frames: OptionDemos.space(.commitAndSpace), pinned: DemoPinning.mini)
        }
    }
}

struct TypoInfo: View {
    let current: TypoSetting
    let correctionsKey: String
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            MiniDemo(frames: OptionDemos.typo(correctionsKey: correctionsKey), restingFrame: 2, pinned: DemoPinning.mini)
            VStack(alignment: .leading, spacing: 6) {
                ForEach([TypoSetting.off, .standard, .aggressive]) { setting in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(setting == .off ? "關閉" : setting == .standard ? "標準" : "積極")
                            .fontWeight(setting == current ? .semibold : .regular)
                            .frame(width: 34, alignment: .leading)
                        Text(OptionDemos.typoSubtitle(setting))
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .font(.callout)
                }
            }
            Text("照按鍵打出的字優先；修正過的字也在 \(correctionsKey) 的清單中。")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

struct FuzzyInfo: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            MiniDemo(frames: OptionDemos.fuzzy, pinned: DemoPinning.mini)
            Text("兩個詞同樣常用時，讀音完全符合的排在前面。")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: - 快捷鍵

/// Every key, most used first, in the words of what it does.  Typing keys are fixed; the commands below them can be
/// rebound (click the keycaps, press the new keys) and restored to their defaults.
private struct KeysPane: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        let vertical = preferences.candidateLayout == .vertical
        let pageKeys = preferences.pageSelectionKeys.map { String($0).uppercased() }
        let keys = preferences.selectionKeys == .digits ? "1–\(pageKeys.count)" : pageKeys.joined(separator: " ")
        let shift: [String] = switch preferences.shiftToggle {
        case .both: ["Shift"]
        case .left: ["左 Shift"]
        case .right: ["右 Shift"]
        case .none: []
        }
        let space = preferences.spaceAction
        Form {
            Section("打字") {
                KeyRow("拼注音", ["注音鍵"])
                KeyRow("聲調：二聲、三聲、四聲、輕聲", ["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"])
                KeyRow("一聲（完成這個音）", ["空白"])
                KeyRow("送出", ["Return"], space == .commit ? ["空白"] : [])
                if space == .commitAndSpace { KeyRow("送出並輸入空白", ["空白"]) }
                KeyRow("刪掉注音符號，或游標前的字", ["⌫"])
                KeyRow("在組字中移動游標", ["←"], ["→"], joiner: "")
            }
            Section {
                KeyRow("開啟候選字", ["↓"], space == .openCandidates ? ["空白"] : [])
                KeyRow("選字", [keys])
                KeyRow("移動選取", vertical ? ["↑"] : ["←"], vertical ? ["↓"] : ["→"], joiner: "")
                if vertical { KeyRow("改前一個或後一個字", ["←"], ["→"], joiner: "") }
                KeyRow("下一頁", ["空白"], CurrentKeys(preferences).caps(.nextPage))
                KeyRow("上一頁", CurrentKeys(preferences).caps(.previousPage))
                if !vertical { KeyRow("上一頁、下一頁", ["↑"], ["↓"], joiner: "") }
                KeyRow("關閉候選字", ["Esc"])
            } header: {
                Text("改字")
            } footer: {
                FooterText("游標在句尾時改最後的字詞；在句中時改有底線的字。")
            }
            Section {
                ForEach(ShortcutCommand.allCases) { command in
                    ShortcutRecorder(preferences: preferences, command: command)
                }
            } header: {
                Text("可以自訂的按鍵")
            } footer: {
                HStack(alignment: .firstTextBaseline) {
                    FooterText("點按鍵，再按下新的組合；Esc 取消。")
                    Spacer()
                    Button("還原預設值") { preferences.resetShortcuts() }
                        .disabled(preferences.shortcuts == ShortcutCommand.defaults)
                }
            }
            Section("其他") {
                if !shift.isEmpty { KeyRow("中英切換", shift.map { "單按 " + $0 }) }
                KeyRow(preferences.escapeClearsComposition ? "清除組字" : "清除注音", ["Esc"])
                if preferences.punctuationKeys == .windows {
                    KeyRow("中文標點：，。、？！：；『』", ["⌃", "符號鍵"])
                    KeyRow("鍵上的符號：< > _ ? !", ["⇧", "符號鍵"])
                } else {
                    KeyRow("中文標點：，。？！：『』——", ["⇧", "符號鍵"])
                    KeyRow("鍵上的符號：. / ; < >", ["⌥", "數字或符號鍵"])
                }
                KeyRow("恢復剛打出注音的字（緊接著按）", CurrentKeys(preferences).caps(.typeZhuyin), ["Esc"])
                KeyRow("「 」 、", ["[", "]", "'"])
                KeyRow("英文", ["Caps Lock"])
            }
        }
    }
}

/// A rebindable command: its keycaps are a button that records the next key pressed.
private struct ShortcutRecorder: View {
    @ObservedObject var preferences: Preferences
    let command: ShortcutCommand
    @State private var recording = false
    @State private var problem: String?
    @State private var monitor: Any?

    var body: some View {
        let shortcut = preferences.shortcuts[command] ?? command.defaultShortcut
        LabeledContent {
            HStack(spacing: 8) {
                if let problem { Text(problem).font(.callout).foregroundStyle(.red) }
                Button(action: toggle) {
                    if recording {
                        Text("按下新的按鍵…")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(Color.accentColor)
                            .padding(.horizontal, 8)
                            .frame(minHeight: 20)
                            .background(RoundedRectangle(cornerRadius: 5).strokeBorder(Color.accentColor, lineWidth: 1))
                    } else {
                        KeyCombo(keys: shortcut.keycaps)
                    }
                }
                .buttonStyle(.plain)
                .help("點一下，再按下新的按鍵")
                .accessibilityLabel("\(command.title)：\(shortcut.keycaps.joined(separator: " "))")
            }
        } label: {
            Text(command.title)
        }
        .onDisappear(perform: stop)
    }

    private func toggle() { recording ? stop() : start() }

    private func start() {
        problem = nil
        recording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            defer { stop() }
            let flags = event.modifierFlags.intersection([.control, .option, .shift, .command])
            if event.keyCode == 53 && flags.isEmpty { return nil }
            guard let shortcut = Shortcut(keyCode: event.keyCode, modifiers: event.modifierFlags) else {
                problem = "這個鍵不能用"
                return nil
            }
            if let reason = shortcut.reservedReason {
                problem = reason
            } else if let other = preferences.shortcuts.first(where: { $0.key != command && $0.value == shortcut })?.key {
                problem = "已用於「\(other.title)」"
            } else {
                var all = preferences.shortcuts
                all[command] = shortcut
                preferences.shortcuts = all
            }
            return nil
        }
    }

    private func stop() {
        recording = false
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}

/// What a key does on the leading side; its keys as keycaps on the trailing side.  A second group is an alternative.
private struct KeyRow: View {
    let action: String
    let keys: [String]
    let alternative: [String]
    /// Between the two groups: "或" for alternatives, "" for a pair such as ← →.
    let joiner: String
    init(_ action: String, _ keys: [String], _ alternative: [String] = [], joiner: String = "或") {
        self.action = action
        self.keys = keys
        self.alternative = alternative
        self.joiner = joiner
    }
    var body: some View {
        LabeledContent(action) {
            HStack(spacing: 6) {
                KeyCombo(keys: keys)
                if !alternative.isEmpty {
                    if !joiner.isEmpty { Text(joiner).foregroundStyle(.tertiary) }
                    KeyCombo(keys: alternative)
                }
            }
        }
    }
}

// MARK: - 候選字視窗

private struct CandidatesPane: View {
    @ObservedObject var preferences: Preferences
    /// The parts of the preview the pointed-at setting changes.
    @State private var hovered: Set<PreviewPart> = []
    /// The parts a setting just changed, highlighted for a moment.
    @State private var pulsed: Set<PreviewPart> = []
    @State private var pulseGeneration = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let window: Set<PreviewPart> = [.window, .badge]

    var body: some View {
        let keys = CurrentKeys(preferences)
        let zhuyin = keys.text(.typeZhuyin)
        let corrections = keys.text(.correctionsList)
        let single = keys.text(.singleCharacterList)
        let vertical = preferences.candidateLayout == .vertical
        Form {
            Section {
                CandidatePreview(preferences: preferences, highlighted: hovered.union(pulsed),
                                 animated: !DemoPinning.still)
                    .padding(.vertical, 6)
            }
            Section("版面") {
                Picker(selection: binding(preferences, \.candidateLayout)) {
                    Text("直式").tag(CandidateLayout.vertical)
                    Text("橫式").tag(CandidateLayout.horizontal)
                } label: {
                    RowLabel("排列", vertical ? "↑ ↓ 移動反白，← → 改前一個或後一個字" : "← → 移動反白，↑ ↓ 翻頁")
                }
                .pickerStyle(.segmented)
                .pointsAt([.window], $hovered)
                LabeledContent("字級") {
                    HStack(spacing: 8) {
                        Text("字").font(.system(size: 11)).foregroundStyle(.secondary)
                        Slider(value: Binding(get: { preferences.candidateFontSize },
                                              set: { preferences.candidateFontSize = $0.rounded() }), in: 13...28)
                            .frame(maxWidth: 200)
                            .accessibilityValue("\(Int(preferences.candidateFontSize)) pt")
                        Text("字").font(.system(size: 17)).foregroundStyle(.secondary)
                        Text("\(Int(preferences.candidateFontSize)) pt")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 40, alignment: .trailing)
                    }
                }
                .pointsAt([.part(.rows)], $hovered)
                LabeledContent {
                    Stepper(value: binding(preferences, \.candidatesPerPage), in: 3...9) {
                        Text("\(preferences.candidatesPerPage) 個").monospacedDigit()
                    }
                } label: {
                    RowLabel("每頁的候選字", "用 \(keys.selection) 選字；空白或 \(keys.text(.nextPage)) 翻到下一頁")
                }
                .pointsAt([.part(.keyColumn), .part(.pages)], $hovered)
                Picker("選字鍵", selection: binding(preferences, \.selectionKeys)) {
                    Text("1 2 3 4 5 6 7 8 9").tag(SelectionKeySet.digits)
                    Text("A S D F G H J K L").tag(SelectionKeySet.homeRow)
                }
                .pointsAt([.part(.keyColumn)], $hovered)
                Picker("間距", selection: binding(preferences, \.candidateDensity)) {
                    Text("標準").tag(CandidateDensity.standard)
                    Text("緊湊").tag(CandidateDensity.compact)
                }
                .pickerStyle(.segmented)
                .pointsAt([.part(.rows)], $hovered)
            }
            Section("內容") {
                Toggle(isOn: binding(preferences, \.showsZhuyinHint)) {
                    RowLabel("顯示注音", "列在清單上方；點一下或按 \(zhuyin) 打出注音本身，緊接著再按一次或按 Esc 恢復")
                }
                .pointsAt([.part(.zhuyinRow)], $hovered)
                Toggle(isOn: binding(preferences, \.showsAnnotations)) {
                    RowLabel("標出修正的讀音", "修正過的字旁邊以灰字標出注音，例如 \(corrections) 列出的字")
                }
                .pointsAt([.part(.annotations)], $hovered)
                Toggle(isOn: binding(preferences, \.showsListSwitches)) {
                    RowLabel("單字與修正按鈕", "\(single) 只列一個字、\(corrections) 列出修正的字；關閉時只在使用中顯示")
                }
                .pointsAt([.part(.switches)], $hovered)
            }
            Section("位置與外觀") {
                Picker(selection: binding(preferences, \.candidatePlacement)) {
                    Text("游標下方").tag(CandidatePlacement.below)
                    Text("游標上方").tag(CandidatePlacement.above)
                } label: {
                    RowLabel("位置", "空間不足時換到另一邊，一律留在螢幕內")
                }
                .pointsAt(Self.window, $hovered)
                Picker(selection: binding(preferences, \.panelAppearance)) {
                    Text("跟隨系統").tag(PanelAppearance.system)
                    Text("淺色").tag(PanelAppearance.light)
                    Text("深色").tag(PanelAppearance.dark)
                } label: {
                    RowLabel("外觀", "也套用到「中」「英」提示")
                }
                .pointsAt(Self.window, $hovered)
                Toggle(isOn: binding(preferences, \.panelAnimation)) {
                    RowLabel("動畫", reduceMotion ? "候選字視窗淡入；已開啟「減少動態效果」，不滑動"
                                              : "候選字視窗淡入，「中」「英」提示淡出")
                }
                .pointsAt(Self.window, $hovered)
            }
        }
        .onChange(of: preferences.candidateLayout) { _, _ in pulse([.window]) }
        .onChange(of: preferences.candidateFontSize) { _, _ in pulse([.part(.rows)]) }
        .onChange(of: preferences.candidatesPerPage) { _, _ in pulse([.part(.keyColumn), .part(.pages)]) }
        .onChange(of: preferences.selectionKeys) { _, _ in pulse([.part(.keyColumn)]) }
        .onChange(of: preferences.candidateDensity) { _, _ in pulse([.part(.rows)]) }
        .onChange(of: preferences.showsZhuyinHint) { _, _ in pulse([.part(.zhuyinRow)]) }
        .onChange(of: preferences.showsAnnotations) { _, _ in pulse([.part(.annotations)]) }
        .onChange(of: preferences.showsListSwitches) { _, _ in pulse([.part(.switches)]) }
        .onChange(of: preferences.candidatePlacement) { _, _ in pulse(Self.window) }
        .onChange(of: preferences.panelAppearance) { _, _ in pulse(Self.window) }
    }

    /// Highlights `parts` for a moment after their setting changed.
    private func pulse(_ parts: Set<PreviewPart>) {
        pulseGeneration += 1
        let generation = pulseGeneration
        pulsed = parts
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.9) {
            if pulseGeneration == generation { pulsed = [] }
        }
    }
}

private extension View {
    /// While the pointer is over this setting, the preview highlights `parts`.
    func pointsAt(_ parts: Set<PreviewPart>, _ hovered: Binding<Set<PreviewPart>>) -> some View {
        onHover { inside in
            if inside { hovered.wrappedValue = parts } else if hovered.wrappedValue == parts { hovered.wrappedValue = [] }
        }
    }
}

// MARK: - 自訂詞、記住的字詞

private struct PhrasesPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences

    var body: some View {
        let shortcuts = preferences.shortcuts
        let left = shortcuts[.selectLeft]?.compact ?? "⇧←"
        let right = shortcuts[.selectRight]?.compact ?? "⇧→"
        Form {
            Section {
                if model.engine == nil {
                    LoadingRow()
                } else {
                    ListEditor(view: model.phrases.view)
                        .frame(minHeight: 340, maxHeight: .infinity)
                        .listRowInsets(EdgeInsets())
                }
            } footer: {
                FooterText("自訂詞在選字時優先列出；打字時用 \(left) \(right) 選取字詞，再按 Return 也能加入。")
            }
        }
    }
}

private struct LearnedPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences

    var body: some View {
        Form {
            Section {
                OptionRow(title: "記住選過的字詞", subtitle: "下次打同樣的讀音時排在前面；只存在這台 Mac") {
                    InfoButton(title: "記住的字詞") { LearnedInfo() }
                } control: {
                    Toggle("記住選過的字詞", isOn: binding(preferences, \.learningEnabled))
                        .labelsHidden()
                        .toggleStyle(.switch)
                }
            }
            Section {
                if model.engine == nil {
                    LoadingRow()
                } else {
                    ListEditor(view: model.learned.view)
                        .frame(minHeight: 300, maxHeight: .infinity)
                        .listRowInsets(EdgeInsets())
                }
            }
        }
    }
}

/// What is remembered and for how long (docs/product/DESIGN.md §2.5).
struct LearnedInfo: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            FactRow(symbol: "checkmark.circle", text: "選過的字詞：同樣的讀音直接採用")
            FactRow(symbol: "link", text: "常接在一起的詞：送出時學習，選字時參考")
            FactRow(symbol: "keyboard", text: "按錯習慣：常按錯的鍵更容易修正")
            FactRow(symbol: "clock", text: "近期與長期分開記，半衰期 8 小時與 90 天")
            FactRow(symbol: "lock", text: "密碼欄位中暫停學習")
            FactRow(symbol: "nosign", text: "設為「不記住」的字詞之後不再學")
        }
    }
}

// MARK: - 進階、關於

private struct AdvancedPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences
    @State private var confirmingReset = false
    @State private var confirmingClear = false

    var body: some View {
        Form {
            Section {
                OptionRow(title: "記錄改字", subtitle: "改字時記下打的鍵與改前改後的字，用來改進選字") {
                    InfoButton(title: "記錄改字") { CorrectionLogInfo(keys: CurrentKeys(preferences)) }
                } control: {
                    Toggle("記錄改字", isOn: binding(preferences, \.recordsCorrections))
                        .labelsHidden()
                        .toggleStyle(.switch)
                }
                OptionRow(title: "一併記錄游標前的文字", subtitle: "重播記錄時使用同樣的前文") {
                    Color.clear.frame(height: 1)
                } control: {
                    Toggle("一併記錄游標前的文字", isOn: binding(preferences, \.recordsCorrectionContext))
                        .labelsHidden()
                        .toggleStyle(.switch)
                }
                .disabled(!preferences.recordsCorrections)
                LabeledContent {
                    HStack(spacing: 8) {
                        Button("在 Finder 中顯示") { CorrectionLogActions.reveal() }
                        Button("匯出…") { CorrectionLogActions.export() }
                            .disabled(model.correctionCount == 0)
                        Button("清除…") { confirmingClear = true }
                            .disabled(model.correctionCount == 0)
                    }
                } label: {
                    Text("\(model.correctionCount) 筆記錄").monospacedDigit()
                }
            } header: {
                Text("改字記錄")
            } footer: {
                FooterText("記錄只存在這台 Mac；密碼欄位中不記錄。")
            }
            Section {
                ListEditor(view: model.apps.view)
                    .frame(height: 200)
                    .listRowInsets(EdgeInsets())
            } header: {
                Text("在候選字視窗顯示組字的 App")
            } footer: {
                FooterText("清單中的 App，組字顯示在候選字視窗上方。")
            }
            Section {
                LabeledContent {
                    Button("回復…") { confirmingReset = true }
                } label: {
                    RowLabel("回復預設設定", "自訂詞與記住的字詞會保留")
                }
            }
        }
        .alert("要回復所有設定的預設值嗎？", isPresented: $confirmingReset) {
            Button("回復", role: .destructive) {
                preferences.resetToDefaults()
                model.apps.reload()
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("自訂詞與記住的字詞會保留。")
        }
        .alert("要清除所有改字記錄嗎？", isPresented: $confirmingClear) {
            Button("清除", role: .destructive) { CorrectionLog.shared.clear {} }
            Button("取消", role: .cancel) {}
        } message: {
            Text("清除後無法復原。")
        }
    }
}

/// What the correction log records (docs/product/DESIGN.md §2.7), with the current keys.
struct CorrectionLogInfo: View {
    let keys: CurrentKeys
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            FactRow(symbol: "checkmark.circle", text: "只記錄有改字的組字")
            FactRow(symbol: "keyboard", text: "打的鍵與按鍵間隔，以及每一步畫面上的字")
            FactRow(symbol: "list.bullet.rectangle",
                    text: "從清單選字，包括 \(keys.text(.correctionsList)) 修正與 \(keys.text(.singleCharacterList)) 單字")
            FactRow(symbol: "delete.left", text: "⌫ 或 Esc 刪掉後重打；送出後 30 秒內刪掉重打")
            FactRow(symbol: "character.cursor.ibeam",
                    text: "\(keys.text(.typeZhuyin)) 打出注音、\(keys.text(.selectLeft)) \(keys.text(.selectRight)) 加入自訂詞、在句中修改")
            FactRow(symbol: "text.alignleft", text: "游標前的文字只在開啟第二個選項時記錄")
            FactRow(symbol: "lock", text: "密碼欄位中不記錄；記錄中沒有 App 的名稱")
            FactRow(symbol: "internaldrive", text: "存在這台 Mac 的 corrections.jsonl，可以匯出")
        }
    }
}

/// The buttons of the correction log section.
enum CorrectionLogActions {
    static func reveal() {
        let log = CorrectionLog.shared
        log.flush()
        if FileManager.default.fileExists(atPath: log.url.path) {
            NSWorkspace.shared.activateFileViewerSelecting([log.url])
        } else {
            NSWorkspace.shared.open(log.url.deletingLastPathComponent())
        }
    }

    static func export() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "ZhuyinType 改字記錄.jsonl"
        let save: (NSApplication.ModalResponse) -> Void = { response in
            guard response == .OK, let url = panel.url else { return }
            do { try CorrectionLog.shared.export(to: url) } catch {
                let alert = NSAlert()
                alert.messageText = "無法匯出"
                alert.informativeText = error.localizedDescription
                alert.runModal()
            }
        }
        if let window = NSApp.keyWindow { panel.beginSheetModal(for: window, completionHandler: save) }
        else { panel.begin(completionHandler: save) }
    }
}

private struct AboutPane: View {
    @ObservedObject private var updater = Updater.shared

    private var version: String {
        let info = Bundle.main.infoDictionary
        guard let short = info?["CFBundleShortVersionString"] as? String else { return "開發版" }
        let build = info?["CFBundleVersion"] as? String
        return build.map { "\(short)（\($0)）" } ?? short
    }

    var body: some View {
        Form {
            Section {
                VStack(spacing: 6) {
                    Image(nsImage: NSApp.applicationIconImage)
                        .resizable()
                        .frame(width: 80, height: 80)
                    Text("ZhuyinType")
                        .font(.title2.weight(.semibold))
                    Text("版本 \(version)")
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
            }
            if updater.isAvailable {
                Section("更新") {
                    Toggle(isOn: Binding(get: { updater.automaticallyChecks }, set: { updater.automaticallyChecks = $0 })) {
                        RowLabel("自動檢查更新", "每天一次；更新檔都經過簽章驗證")
                    }
                    Toggle(isOn: Binding(get: { updater.automaticallyInstalls }, set: { updater.automaticallyInstalls = $0 })) {
                        RowLabel("自動安裝更新", "在組字結束後換上新版本")
                    }
                    .disabled(!updater.automaticallyChecks)
                    if let pending = updater.pendingVersion {
                        LabeledContent("已下載", value: "\(pending)，組字結束後安裝")
                    }
                    LabeledContent {
                        Button("立即檢查…") { updater.checkNow() }
                    } label: {
                        RowLabel("檢查更新", updater.lastCheck.map {
                            "上次檢查：" + $0.formatted(date: .abbreviated, time: .shortened)
                        })
                    }
                }
            }
            Section {
                LabeledContent("程式授權", value: "AGPL 3.0")
                LabeledContent("詞庫授權", value: "CC BY-SA 4.0")
                LabeledContent("詞庫資料", value: "CC-CEDICT、Unicode Unihan、wordfreq")
            }
        }
    }
}

private struct LoadingRow: View {
    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text("正在載入詞庫…").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 120)
    }
}
