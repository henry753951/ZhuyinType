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

    var body: some View {
        Form {
            Section("按鍵") {
                Picker("空白鍵", selection: binding(preferences, \.spaceOpensCandidates)) {
                    Text("開啟候選字視窗").tag(true)
                    Text("送出文字").tag(false)
                }
                Picker("Esc", selection: binding(preferences, \.escapeClearsComposition)) {
                    Text("清除整段組字").tag(true)
                    Text("只清除注音").tag(false)
                }
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
