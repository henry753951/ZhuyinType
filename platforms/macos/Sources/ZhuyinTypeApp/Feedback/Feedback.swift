import AppKit
import SwiftUI

/// 意見與問題: the GitHub issue forms opened in the browser (docs/product/DESIGN.md §4.5.2; the same as Linux and Windows,
/// `zt_core::feedback`).  The address carries ZhuyinType's version, the system's name and version, and for a removal the
/// reason picked: nothing typed, no words, no app names, no paths.  The user writes the rest and sends it on GitHub.
enum Feedback {
    static let newIssue = "https://github.com/henry753951/ZhuyinType/issues/new"
    static let sectionTitle = "意見與問題"
    static let place = SettingsPlace.section(.about, sectionTitle)
    /// The Windows uninstaller and 移除 ZhuyinType on every system offer these.
    static let uninstallReasons = ["轉換或選字不準", "不穩定、變慢或無法輸入", "缺少需要的功能", "操作習慣不同",
                                   "改用其他輸入法", "其他原因"]

    enum Topic: Equatable {
        case conversion, crash, other, suggestion, uninstall(reason: String)

        var template: String {
            switch self {
            case .conversion: return "conversion.yml"
            case .crash: return "crash.yml"
            case .other: return "other.yml"
            case .suggestion: return "suggestion.yml"
            case .uninstall: return "uninstall.yml"
            }
        }
    }

    static func url(_ topic: Topic, version: String, system: String) -> URL {
        var fields = [("template", topic.template), ("version", version), ("system", system)]
        if case .uninstall(let reason) = topic { fields.append(("reason", reason)) }
        // RFC 3986's unreserved characters only, so 「&」, 「+」 and Chinese stay inside their field.
        let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        let query = fields.map { "\($0.0)=" + ($0.1.addingPercentEncoding(withAllowedCharacters: unreserved) ?? "") }
        return URL(string: newIssue + "?" + query.joined(separator: "&"))!
    }

    /// 「macOS 27.0（26A123），arm64」.
    static var system: String {
        let os = ProcessInfo.processInfo.operatingSystemVersion
        let version = "\(os.majorVersion).\(os.minorVersion)" + (os.patchVersion > 0 ? ".\(os.patchVersion)" : "")
        let build = HealthFactsReader.sysctl("kern.osversion").map { "（\($0)）" } ?? ""
        return "macOS \(version)\(build)，\(HealthFactsReader.machine())"
    }

    static func open(_ topic: Topic) {
        NSWorkspace.shared.open(url(topic, version: ReleaseNotes.currentVersion ?? "開發版", system: system))
    }
}

/// 關於 › 意見與問題.
struct FeedbackSection: View {
    @State private var showingReport = false
    @State private var report: String?

    var body: some View {
        SettingsSection(Feedback.sectionTitle) {
            SettingLinkRow(title: "轉換或選字不對", subtitle: "打對了注音，卻出現不對的字") { Feedback.open(.conversion) }
            SettingLinkRow(title: "當機、沒反應或無法輸入", subtitle: "輸入法不動、打不出注音或切換不到") { Feedback.open(.crash) }
            SettingLinkRow(title: "其他問題", subtitle: "介面、設定、安裝或更新") { Feedback.open(.other) }
            SettingLinkRow(title: "建議", subtitle: "希望增加或改變的功能") { Feedback.open(.suggestion) }
            SettingLinkRow(title: "診斷資料", subtitle: "檢查結果與最近的錯誤；看過內容後，自己貼到回報裡") {
                report = nil
                showingReport = true
                HealthMonitor.shared.makeReport { report = $0 }
            }
        } footer: {
            FooterText("在瀏覽器開啟 GitHub 的回報頁面，填好版本與系統；內容由你填寫並送出，需要 GitHub 帳號。ZhuyinType 不會自動送出任何資料。")
        }
        .sheet(isPresented: $showingReport) { HealthReportSheet(text: report) }
    }
}

/// 更新內容: what changed since the version the user updated from.
struct ReleaseNotesPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    /// The UI previews show these instead of the app's own.
    static var sample: [ReleaseNotes]?

    private var releases: [ReleaseNotes] {
        if let sample = Self.sample { return sample }
        guard let current = ReleaseNotes.currentVersion else { return [] }
        return ReleaseNotesProgress(defaults: UserDefaults.standard).shown(ReleaseNotes.all, current: current)
    }

    var body: some View {
        SettingsForm {
            if releases.isEmpty {
                SettingsSection { SettingRow(title: "這個版本的更新內容在發布頁面") }
            }
            ForEach(releases, id: \.version) { release in
                SettingsSection("ZhuyinType \(release.version)") {
                    if release.sections.isEmpty { SettingRow(title: "這一版沒有 macOS 的變更") }
                    ForEach(release.sections, id: \.title) { section in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(section.title).font(.callout.weight(.semibold)).foregroundStyle(.secondary)
                            ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in
                                HStack(alignment: .firstTextBaseline, spacing: 8) {
                                    Text("・").foregroundStyle(.tertiary).accessibilityHidden(true)
                                    VStack(alignment: .leading, spacing: 2) {
                                        if let title = item.title { Text(title).fontWeight(.semibold) }
                                        Text(item.text).fixedSize(horizontal: false, vertical: true)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                }
                            }
                        }
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                } footer: {
                    FooterText(release.date)
                }
            }
            SettingsSection {
                RegistryRow("showUpdateNotes", preferences)
                SettingLinkRow(title: "完整的發布說明", subtitle: "每一版的變更、已知問題與準確率，在 GitHub 上") {
                    NSWorkspace.shared.open(ReleaseNotes.page)
                }
                SettingLinkRow(title: "回報問題或建議") { SettingsWindowController.shared.open(Feedback.place) }
            }
        }
        .onAppear { if !SettingsWindowController.isPreview { UpdateNotice.shared.seen() } }
    }
}

/// 移除 ZhuyinType: how to, where the personal data is, and the reason if the user wants to say.
struct RemovalPane: View {
    /// What removes this copy: the package installs for the whole Mac, a build from source for this user.
    static func command(bundle: URL) -> String {
        bundle.path.hasPrefix("/Library/")
            ? "sudo rm -rf \"/Library/Input Methods/ZhuyinType.app\" && sudo pkgutil --forget org.zhuyintype.pkg"
            : "rm -rf ~/Library/Input\\ Methods/ZhuyinType.app"
    }

    var body: some View {
        let command = Self.command(bundle: Bundle.main.bundleURL)
        SettingsForm {
            SettingsSection("移除") {
                SettingRow(title: "1. 移除輸入方式", subtitle: "在「鍵盤 › 輸入方式」選取 ZhuyinType，按「－」") {
                    Button("開啟鍵盤設定…") { HealthActions.perform(.openKeyboardSettings) }
                }
                SettingRow(title: "2. 刪除 App", subtitle: command) {
                    Button("拷貝指令") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(command, forType: .string)
                    }
                }
                SettingRow(title: "3. 登出再登入")
                SettingRow(title: "個人資料", subtitle: "自訂詞與記住的字詞會保留，要一併刪除時請移除這個檔案夾") {
                    Button("在 Finder 中顯示") { NSWorkspace.shared.activateFileViewerSelecting([EngineHost.userDirectory]) }
                }
            } footer: {
                FooterText("指令在「終端機」執行" + (command.hasPrefix("sudo") ? "；輸入法安裝在整台 Mac，刪除時需要管理者密碼。" : "。"))
            }
            SettingsSection("告訴我們原因（選填）") {
                ForEach(Feedback.uninstallReasons, id: \.self) { reason in
                    SettingLinkRow(title: reason) { Feedback.open(.uninstall(reason: reason)) }
                }
            } footer: {
                FooterText("在瀏覽器開啟 GitHub 的頁面，內容由你填寫並送出。")
            }
        }
    }
}
