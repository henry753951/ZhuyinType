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
            case .phrases: PhrasesPane(model: model)
            case .learned: LearnedPane(model: model, preferences: preferences)
            case .advanced: AdvancedPane(model: model, preferences: preferences)
            case .about: AboutPane()
            }
        }
        .formStyle(.grouped)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }
}

/// A two-way binding to a `Preferences` property.
private func binding<Value>(_ preferences: Preferences, _ keyPath: ReferenceWritableKeyPath<Preferences, Value>) -> Binding<Value> {
    Binding(get: { preferences[keyPath: keyPath] }, set: { preferences[keyPath: keyPath] = $0 })
}

private struct GeneralPane: View {
    @ObservedObject var preferences: Preferences
    static let fuzzyPairs: [(label: String, set: FuzzySounds)] = [
        ("ㄗ 與 ㄓ", .zZh), ("ㄘ 與 ㄔ", .cCh), ("ㄙ 與 ㄕ", .sSh), ("ㄣ 與 ㄥ", .enEng), ("ㄢ 與 ㄤ", .anAng),
    ]

    var body: some View {
        Form {
            Section {
                Picker("空白鍵", selection: binding(preferences, \.spaceOpensCandidates)) {
                    Text("開啟候選字視窗").tag(true)
                    Text("送出文字").tag(false)
                }
                Picker("Esc", selection: binding(preferences, \.escapeClearsComposition)) {
                    Text("清除整段組字").tag(true)
                    Text("只清除注音").tag(false)
                }
            } header: {
                Text("按鍵")
            } footer: {
                FooterText("所有按鍵列在「快捷鍵」。")
            }
            Section {
                Picker("游標在句中時改", selection: binding(preferences, \.candidateAnchor)) {
                    Text("游標右邊的字").tag(AnchorSetting.after)
                    Text("游標左邊的字").tag(AnchorSetting.before)
                }
                Picker("按錯鍵修正", selection: binding(preferences, \.typoTolerance)) {
                    Text("關閉").tag(TypoSetting.off)
                    Text("標準").tag(TypoSetting.standard)
                    Text("積極").tag(TypoSetting.aggressive)
                }
                Toggle("參考游標前的文字", isOn: binding(preferences, \.usesPrecedingText))
            } header: {
                Text("選字")
            } footer: {
                FooterText("游標在句尾時改最後的字詞；在句中時，要改的字有底線。按錯相鄰的鍵或聲調時自動修正，候選字視窗只列照按鍵打出的字，修正的字按 Shift-↑ 才列出。選字參考游標前約 30 個字（包括同一個 App 裡 10 分鐘內打的字），只在這台 Mac 上使用。")
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
                Text("模糊音")
            } footer: {
                FooterText("開啟的兩個音互相通用。讀音完全符合的字詞仍排在前面，除非另一個詞明顯更常用。")
            }
            Section("中英切換") {
                Picker("切換鍵", selection: binding(preferences, \.shiftToggle)) {
                    Text("左右 Shift").tag(ShiftToggle.both)
                    Text("左 Shift").tag(ShiftToggle.left)
                    Text("右 Shift").tag(ShiftToggle.right)
                    Text("不使用").tag(ShiftToggle.none)
                }
                Toggle("切換時在游標旁顯示「中」或「英」", isOn: binding(preferences, \.showModeToast))
                Toggle("開始輸入時使用英文", isOn: binding(preferences, \.startInEnglish))
            }
        }
    }
}

/// Every key, most used first, in the words of what it does.  Follows the settings that change keys.
private struct KeysPane: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        let vertical = preferences.candidateLayout == .vertical
        let keys = preferences.selectionKeys == .digits ? "1–9" : "A S D F…"
        let shift: [String] = switch preferences.shiftToggle {
        case .both: ["Shift"]
        case .left: ["左 Shift"]
        case .right: ["右 Shift"]
        case .none: []
        }
        Form {
            Section("打字") {
                KeyRow("拼注音，空白是一聲", ["注音鍵"], ["6", "3", "4", "7"], joiner: "+")
                KeyRow("送出", ["Return"])
                KeyRow("刪掉注音符號，或游標前的字", ["⌫"])
                KeyRow("在組字中移動游標", ["←"], ["→"], joiner: "")
            }
            Section {
                KeyRow("開啟候選字", ["↓"], preferences.spaceOpensCandidates ? ["空白"] : [])
                KeyRow("選字", [keys])
                KeyRow("移動選取", vertical ? ["↑"] : ["←"], vertical ? ["↓"] : ["→"], joiner: "")
                if vertical { KeyRow("改前一個或後一個字", ["←"], ["→"], joiner: "") }
                KeyRow("翻頁", ["空白"], ["Tab"])
                KeyRow("關閉候選字", ["Esc"])
            } header: {
                Text("改字")
            } footer: {
                FooterText("游標在句尾時改最後的字詞；在句中時改有底線的字。")
            }
            Section("候選字視窗的其他清單") {
                KeyRow("只列一個字，再按一次還原", ["⇧", "↓"])
                KeyRow("列出按錯修正的字，再按一次還原", ["⇧", "↑"])
                KeyRow("打出注音本身", ["`"])
            }
            Section("其他") {
                if !shift.isEmpty { KeyRow("中英切換", shift.map { "單按 " + $0 }) }
                KeyRow("加入自訂詞：選取後按 Return", ["⇧", "←"], ["⇧", "→"], joiner: "")
                KeyRow(preferences.escapeClearsComposition ? "清除組字" : "清除注音", ["Esc"])
                KeyRow("刪掉一個詞", ["⌥", "⌫"])
                KeyRow("送出並換行", ["⇧", "Return"])
                KeyRow("中文標點：，。？！「」", ["⇧", "符號鍵"])
                KeyRow("鍵上印的符號：. / ; < >", ["⌥", "數字或符號鍵"])
                KeyRow("英文", ["Caps Lock"])
            }
        }
    }
}

/// What a key does on the leading side; its keys as keycaps on the trailing side.  A second group is an alternative.
private struct KeyRow: View {
    let action: String
    let keys: [String]
    let alternative: [String]
    /// Between the two groups: "或" for alternatives, "" for a pair such as ← →, "+" for keys used together.
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

private struct KeyCombo: View {
    let keys: [String]
    var body: some View {
        HStack(spacing: 3) {
            ForEach(keys, id: \.self) { key in
                Text(key)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 6)
                    .frame(minWidth: 22, minHeight: 20)
                    .background(RoundedRectangle(cornerRadius: 5).fill(.quaternary.opacity(0.6)))
                    .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(.quaternary, lineWidth: 0.5))
            }
        }
    }
}

private struct CandidatesPane: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        Form {
            Section {
                Picker("排列", selection: binding(preferences, \.candidateLayout)) {
                    Text("直式").tag(CandidateLayout.vertical)
                    Text("橫式").tag(CandidateLayout.horizontal)
                }
                .pickerStyle(.segmented)
                LabeledContent("字級") {
                    HStack(spacing: 10) {
                        Slider(value: Binding(get: { preferences.candidateFontSize },
                                              set: { preferences.candidateFontSize = $0.rounded() }), in: 13...28)
                            .frame(maxWidth: 220)
                        Text("\(Int(preferences.candidateFontSize)) pt")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 42, alignment: .trailing)
                    }
                }
                Picker("選字鍵", selection: binding(preferences, \.selectionKeys)) {
                    Text("1 2 3 4 5 6 7 8 9").tag(SelectionKeySet.digits)
                    Text("A S D F G H J K L").tag(SelectionKeySet.homeRow)
                }
            }
            Section("預覽") {
                HStack {
                    CandidatePreview(layout: preferences.candidateLayout, fontSize: preferences.candidateFontSize,
                                     keys: Array(preferences.selectionKeys.rawValue))
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 6)
            }
        }
    }
}

/// The real candidate window surface, so the preview matches what appears while typing.
private struct CandidatePreview: NSViewRepresentable {
    let layout: CandidateLayout
    let fontSize: Double
    let keys: [Character]

    private static let list = CandidateList(items: ["今天", "金天", "今", "斤", "巾"].map { Candidate(text: $0) }, pageSize: 9)

    func makeNSView(context: Context) -> InputSurface { InputSurface(content: CandidateContentView(frame: .zero)) }

    func updateNSView(_ surface: InputSurface, context: Context) {
        layOut(surface)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: InputSurface, context: Context) -> CGSize? {
        layOut(nsView)
    }

    @discardableResult
    private func layOut(_ surface: InputSurface) -> CGSize {
        let size = (surface.content as! CandidateContentView)
            .update(list: Self.list, preview: nil, keys: keys, layout: layout, fontSize: fontSize, maxWidth: 460)
        surface.fitContent()
        return size
    }
}

private struct PhrasesPane: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
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
                FooterText("自訂詞在選字時優先列出。")
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
                Toggle("記住選過的字詞", isOn: binding(preferences, \.learningEnabled))
            } footer: {
                FooterText("選過的字詞下次會排在前面。資料只存在這台 Mac。")
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

private struct AdvancedPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences
    @State private var confirmingReset = false

    var body: some View {
        Form {
            Section {
                ListEditor(view: model.apps.view)
                    .frame(height: 200)
                    .listRowInsets(EdgeInsets())
            } header: {
                Text("在候選字視窗顯示組字的 App")
            } footer: {
                FooterText("清單中的 App，組字會顯示在候選字視窗上方。")
            }
            Section {
                LabeledContent("回復預設設定") {
                    Button("回復…") { confirmingReset = true }
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
    }
}

private struct AboutPane: View {
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

private struct FooterText: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}
