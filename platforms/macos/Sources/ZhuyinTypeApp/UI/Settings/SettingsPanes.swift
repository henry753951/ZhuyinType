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
                SettingRow(title: "記錄改字", subtitle: "改字時記下打的鍵與改前改後的字，用來改進選字",
                           accessory: .info(title: "記錄改字", content: AnyView(CorrectionLogInfo(keys: CurrentKeys(preferences))))) {
                    RowSwitch(title: "記錄改字", isOn: binding(preferences, \.recordsCorrections))
                }
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
    @ObservedObject private var updater = Updater.shared

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
                SettingRow(title: "程式授權", value: "AGPL 3.0")
                SettingRow(title: "詞庫授權", value: "CC BY-SA 4.0")
                SettingRow(title: "詞庫資料", value: "CC-CEDICT、Unicode Unihan、wordfreq")
            }
        }
    }
}
