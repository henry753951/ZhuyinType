import AppKit
import SwiftUI
import ZhuyinEngine

/// Fixed frames for the demos, so the UI previews show a chosen moment.  Nil while the app runs.
enum DemoPinning {
    /// Draw keycaps at rest, without their press.
    static var still = false
    static var context: Int?
    static var intro: Int?
    /// A conflict message to show on a row of the 快捷鍵 page, by row.
    /// The 快捷鍵 page's fixed keys shown, not folded.
    static var showsFixedKeys = false
    /// The arrival tint at a fixed strength, for frames of its pulse.
    static var arrivalOpacity: Double?
    /// States of shortcut fields to draw, by command.
    static var fieldStates: [ShortcutCommand: ShortcutFieldPreview] = [:]
    /// A push into the current sub-page (or a pop out of it) drawn still at `progress` (0…1).
    static var navigation: (push: Bool, progress: Double)?
    /// The 視窗與提示 preview drawn at this state and moment of its flow.
    static var previewState: (state: PreviewState, frame: Int)?
    /// The floating preview after a change, drawn at this point (window coordinates, top left).
    static var flashAt: CGPoint?
    /// The setup wizard's step transition drawn still: the step it leaves, and the progress (0…1).
    static var wizardTransition: (from: WizardStep, progress: Double)?
    /// Sampling the live animations for the UI previews: the preview plays as in the app.
    static var live = false
    /// A row drawn as if the pointer were over it.
    static var hoveredRow: String?
    /// A list row being removed, drawn still at `progress` (0: in place, 1: gone).
    static var removal: (id: String, progress: Double)?
    static var mini: Int?
}

struct SpaceInfo: View {
    let current: SpaceAction
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("打注音時，空白是一聲；打完以後，空白依這個設定。清單開著時，空白翻到下一頁；選字用選字鍵或 Return。")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(Preferences.spaceActions, id: \.self) { action in
                ChoiceDemo(title: OptionDemos.spaceLabel(action), selected: current == action || current == .commit && action == .commitAndSpace,
                           frames: OptionDemos.space(action), pinned: DemoPinning.mini)
            }
        }
    }
}

struct TypoInfo: View {
    let current: TypoSetting
    /// Nil when the corrections list is turned off.
    let correctionsKey: String?
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
            Text(correctionsKey.map { "照按鍵打出的字優先；修正過的字也在 \($0) 的清單中。" } ?? "照按鍵打出的字優先。")
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

struct AdvancedPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var preferences: Preferences
    @State private var confirmingReset = false
    @State private var confirmingClear = false

    var body: some View {
        SettingsForm {
            SettingsSection("改字記錄") {
                RegistryRow("recordsCorrections", preferences, accessory: .info(title: "記錄改字", content: AnyView(CorrectionLogInfo(keys: CurrentKeys(preferences)))))
                SettingRow(title: "一併記錄游標前的文字", subtitle: "重播記錄時使用同樣的前文", indent: true,
                           dimmed: !preferences.recordsCorrections) {
                    RowSwitch(title: "一併記錄游標前的文字", isOn: binding(preferences, \.recordsCorrectionContext))
                        .disabled(!preferences.recordsCorrections)
                }
                SettingRow(title: "\(model.correctionCount) 筆記錄") {
                    Button("在 Finder 中顯示") { CorrectionLogActions.reveal() }
                    Button("匯出…") { CorrectionLogActions.export() }
                        .disabled(model.correctionCount == 0)
                    Button("清除…") { confirmingClear = true }
                        .disabled(model.correctionCount == 0)
                }
            } footer: {
                FooterText("記錄只存在這台 Mac；密碼欄位中不記錄。")
            }
            SettingsSection("相容性") {
                DrillRow(model: model, title: "在候選字視窗顯示組字的 App", subtitle: "少數 App 需要",
                         value: preferences.popupCompositionApps.isEmpty ? "沒有" : "\(preferences.popupCompositionApps.count) 個",
                         subpage: .apps)
            }
            SettingsSection {
                SettingRow(title: "回復預設設定", subtitle: "自訂詞與記住的字詞會保留") {
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
        .alert("要清除所有改字記錄嗎？", isPresented: $confirmingClear) {
            Button("清除", role: .destructive) { CorrectionLog.shared.clear { error in
                    if let error { NSApp.presentError(error) }
                } }
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
                    text: (["從清單選字"] + [keys.text(.correctionsList).map { "\($0) 修正" },
                                         keys.text(.singleCharacterList).map { "\($0) 單字" }].compactMap { $0 })
                        .joined(separator: "、"))
            FactRow(symbol: "delete.left", text: "⌫ 或 Esc 刪掉後重打；送出後 30 秒內刪掉重打")
            FactRow(symbol: "character.cursor.ibeam",
                    text: ([keys.text(.typeZhuyin).map { "\($0) 打出注音" },
                            keys.text(.selectLeft).flatMap { left in keys.text(.selectRight).map { "\(left) \($0) 加入自訂詞" } },
                            "在句中修改"].compactMap { $0 }).joined(separator: "、"))
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

struct AboutPane: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject private var updater = Updater.shared
    @State private var document: LicenseDocument?

    private var sourceURL: URL {
        let ref = (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String).map { "v\($0)" } ?? "main"
        return URL(string: "https://github.com/henry753951/ZhuyinType/tree/\(ref)")!
    }

    private var version: String {
        let info = Bundle.main.infoDictionary
        guard let short = info?["CFBundleShortVersionString"] as? String else { return "開發版" }
        let build = info?["CFBundleVersion"] as? String
        return build.map { "\(short)（\($0)）" } ?? short
    }

    var body: some View {
        SettingsForm {
            SettingsSection {
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
            }
            if updater.isAvailable {
                SettingsSection("更新") {
                    SettingRow(title: "自動檢查更新", subtitle: "每天一次；更新檔都經過簽章驗證") {
                        RowSwitch(title: "自動檢查更新", isOn: Binding(get: { updater.automaticallyChecks },
                                                                  set: { updater.automaticallyChecks = $0 }))
                    }
                    SettingRow(title: "自動安裝更新", subtitle: "在組字結束後換上新版本", dimmed: !updater.automaticallyChecks) {
                        RowSwitch(title: "自動安裝更新", isOn: Binding(get: { updater.automaticallyInstalls },
                                                                  set: { updater.automaticallyInstalls = $0 }))
                            .disabled(!updater.automaticallyChecks)
                    }
                    if let pending = updater.pendingVersion {
                        SettingRow(title: "已下載", value: "\(pending)，組字結束後安裝")
                    }
                    SettingRow(title: "檢查更新", subtitle: updater.lastCheck.map {
                        "上次檢查：" + $0.formatted(date: .abbreviated, time: .shortened)
                    }) {
                        Button("立即檢查…") { updater.checkNow() }
                    }
                }
            }
            SettingsSection {
                SettingRow(title: "程式授權") {
                    Button("AGPL 3.0") { document = LicenseDocument(file: "AGPL-3.0.txt", entry: "ZhuyinType") }
                        .buttonStyle(.link)
                }
                SettingRow(title: "原始碼") {
                    Link("查看此版本", destination: sourceURL)
                }
                DrillRow(model: model, title: "授權與資料來源", subtitle: "程式庫、詞庫、資料集與模型的授權與署名",
                         value: "", subpage: .sources)
            }
        }
        .sheet(item: $document) { DocumentSheet(document: $0) }
    }
}


private struct SourceList: Decodable {
    struct Group: Decodable, Identifiable { let id: String; let title: String }
    struct Entry: Decodable, Identifiable {
        let id: String, group: String, name: String, use: String, distribution: String, license: String
        let copyright: String?, url: URL?, version: String?, changes: String?
        let licenseFiles: [String]
        let notices: [String]?
        /// The clients that ship it ("macos", "linux", "windows"); every client when absent.
        let platforms: [String]?
        var onMac: Bool { platforms?.contains("macos") ?? true }
    }
    let version: Int
    let groups: [Group]
    let entries: [Entry]
}

struct SourcesPane: View {
    private let resources = Bundle.main.resourceURL
    private let sources = SourcesPane.load()
    @State private var document: LicenseDocument?

    // Read once per pane: the list is static data in the bundle.
    private static func load() -> SourceList? {
        let resources = Bundle.main.resourceURL
        guard let url = resources?.appendingPathComponent("Licenses/sources.json"),
              let data = try? Data(contentsOf: url),
              let list = try? JSONDecoder().decode(SourceList.self, from: data), list.version == 1,
              Set(list.groups.map(\.id)).count == list.groups.count,
              Set(list.entries.map(\.id)).count == list.entries.count,
              list.entries.allSatisfy({ entry in list.groups.contains { $0.id == entry.group } &&
                  ["bundled", "training", "evaluation"].contains(entry.distribution) }) else { return nil }
        return list
    }

    var body: some View {
        SettingsForm {
            if let sources {
                // Entries shipped only by the Linux and Windows client (Qt) are left out here.
                ForEach(sources.groups.filter { group in sources.entries.contains { $0.group == group.id && $0.onMac } }) { group in
                    SettingsSection(group.title) {
                        ForEach(sources.entries.filter { $0.group == group.id && $0.onMac }) { entry in
                            DisclosureGroup {
                                VStack(alignment: .leading, spacing: 8) {
                                    if let copyright = entry.copyright { Text(copyright) }
                                    if let version = entry.version { Text("版本：\(version)") }
                                    if let changes = entry.changes { Text("修改：\(changes)") }
                                    if let url = entry.url { Link("來源", destination: url) }
                                    ForEach(entry.licenseFiles, id: \.self) { file in
                                        fileButton(file, prefix: "Licenses/", entry: entry.name)
                                    }
                                    ForEach(entry.notices ?? [], id: \.self) { file in
                                        fileButton(file, prefix: "", entry: entry.name)
                                    }
                                }.frame(maxWidth: .infinity, alignment: .leading).padding(.top, 8)
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(entry.name)
                                    Text("\(entry.use)・\(entry.license)・\(distribution(entry.distribution))")
                                        .foregroundStyle(.secondary).font(.callout)
                                }
                            }
                        }
                    }
                }
            } else {
                SettingsSection {
                    SettingRow(title: "無法讀取授權與資料來源") {
                        Button("開啟授權資料夾") {
                            if let resources { NSWorkspace.shared.open(resources.appendingPathComponent("Licenses")) }
                        }
                    }
                }
            }
        }
        .sheet(item: $document) { DocumentSheet(document: $0) }
    }

    private func distribution(_ value: String) -> String {
        switch value {
        case "bundled": return "隨 App 提供"
        case "training": return "用於模型訓練"
        default: return "僅用於評測"
        }
    }

    /// A licence or notice opens in the app (DocumentSheet); archives and data lists are shown in Finder.
    private func fileButton(_ file: String, prefix: String, entry: String) -> some View {
        Button(file) {
            guard let url = resources?.appendingPathComponent(prefix + file) else { return }
            if let document = LicenseDocument(url: url, entry: entry) { self.document = document }
            else { NSWorkspace.shared.activateFileViewerSelecting([url]) }
        }
    }
}

/// A licence or notice read inside the app: the same viewer as the Linux and Windows client's DocumentViewer.
struct LicenseDocument: Identifiable {
    let url: URL
    let entry: String
    let text: String
    var id: URL { url }
    var name: String { url.lastPathComponent }
    var markdown: Bool { url.pathExtension.lowercased() == "md" }

    /// Nil for a file that is not text to read here (an archive, a data list) or cannot be read.
    init?(url: URL, entry: String) {
        let ext = url.pathExtension.lowercased()
        guard ["txt", "md", ""].contains(ext), let data = try? Data(contentsOf: url), data.count <= 8 << 20,
              let text = String(data: data, encoding: .utf8) else { return nil }
        self.url = url
        self.entry = entry
        self.text = text
    }

    /// A file of the bundle's Licenses folder.
    init?(file: String, entry: String) {
        guard let url = Bundle.main.resourceURL?.appendingPathComponent("Licenses/" + file) else { return nil }
        self.init(url: url, entry: entry)
    }
}

/// The sheet a licence opens in: the file name and what it belongs to, the text selectable (monospaced for a
/// licence, formatted for Markdown), and 拷貝全文, 在 Finder 中顯示, 完成.
struct DocumentSheet: View {
    let document: LicenseDocument
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                Text(document.name).font(.headline).lineLimit(1).truncationMode(.middle)
                Text(document.entry).font(.callout).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            Divider()
            DocumentTextView(text: document.text, markdown: document.markdown)
            Divider()
            HStack(spacing: 8) {
                Button("拷貝全文") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(document.text, forType: .string)
                }
                Button("在 Finder 中顯示") { NSWorkspace.shared.activateFileViewerSelecting([document.url]) }
                Spacer()
                Button("完成") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
        .frame(minWidth: 520, idealWidth: 680, minHeight: 420, idealHeight: 620)
    }
}

/// Long text in an AppKit text view: selectable, scrolling, fast for the Rust crates' hundred-kilobyte list.
private struct DocumentTextView: NSViewRepresentable {
    let text: String
    let markdown: Bool

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        scroll.drawsBackground = false
        if let view = scroll.documentView as? NSTextView {
            view.isEditable = false
            view.isSelectable = true
            view.drawsBackground = false
            view.textContainerInset = NSSize(width: 16, height: 14)
            view.setAccessibilityLabel("全文")
        }
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? NSTextView else { return }
        view.textStorage?.setAttributedString(attributed)
    }

    private var attributed: NSAttributedString {
        if markdown, let parsed = try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            let result = NSMutableAttributedString(parsed)
            result.addAttributes([.font: NSFont.systemFont(ofSize: NSFont.systemFontSize),
                                  .foregroundColor: NSColor.labelColor], range: NSRange(location: 0, length: result.length))
            return result
        }
        return NSAttributedString(string: text, attributes: [
            .font: NSFont.monospacedSystemFont(ofSize: NSFont.smallSystemFontSize, weight: .regular),
            .foregroundColor: NSColor.labelColor,
        ])
    }
}
