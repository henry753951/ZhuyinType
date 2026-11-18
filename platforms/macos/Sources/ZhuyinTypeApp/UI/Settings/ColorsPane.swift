import AppKit
import SwiftUI

/// 色彩 (docs/product/SETTINGS_IA.md §2): the colours of the candidate window and the 中／英 badge, with a still preview
/// of both, and the system's display options ZhuyinType follows (read here, changed in System Settings).
struct ColorsPane: View {
    @ObservedObject var preferences: Preferences
    @State private var display = SystemDisplay.current

    var body: some View {
        SettingsForm {
            SettingsSection {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 16) { list; badge }
                    list
                }
                .frame(maxWidth: .infinity)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("預覽：候選字清單與中英提示")
            }
            SettingsSection("主題") {
                RegistryRow("panelAppearance", preferences)
                RegistryRow("accentColor", preferences)
            }
            SettingsSection("無障礙") {
                SettingRow(title: "增加對比", subtitle: "開啟時候選字視窗與中英提示改用不透明的背景，加上邊框",
                           value: display.increaseContrast ? "已開啟" : "關閉")
                SettingRow(title: "減少透明度", subtitle: "開啟時候選字視窗與中英提示改用不透明的背景",
                           value: display.reduceTransparency ? "已開啟" : "關閉")
                SettingRow(title: "減少動態效果", subtitle: "開啟時候選字視窗只淡入淡出，示範停在結果",
                           value: display.reduceMotion ? "已開啟" : "關閉")
            } footer: {
                VStack(alignment: .leading, spacing: 6) {
                    Text("這三項跟著系統設定，ZhuyinType 隨時跟著改。")
                    Button("打開輔助使用設定…") { SystemDisplay.openSettings() }
                        .buttonStyle(.link)
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
                                                        object: NSWorkspace.shared)) { _ in display = .current }
    }

    private var list: some View {
        CandidateScenePreview(preferences: preferences, scene: nil, width: 280)
    }

    private var badge: some View {
        CandidateScenePreview(preferences: preferences, scene: PreviewFlows(preferences: preferences).frames(.mode).last,
                              width: 200)
    }
}

/// The system's display options that change how ZhuyinType's floating windows look.
private struct SystemDisplay: Equatable {
    var increaseContrast: Bool
    var reduceTransparency: Bool
    var reduceMotion: Bool

    static var current: SystemDisplay {
        let workspace = NSWorkspace.shared
        return SystemDisplay(increaseContrast: workspace.accessibilityDisplayShouldIncreaseContrast,
                             reduceTransparency: workspace.accessibilityDisplayShouldReduceTransparency,
                             reduceMotion: workspace.accessibilityDisplayShouldReduceMotion)
    }

    /// System Settings › 輔助使用 › 顯示器.
    static func openSettings() {
        let urls = ["x-apple.systempreferences:com.apple.Accessibility-Settings.extension?Display",
                    "x-apple.systempreferences:com.apple.preference.universalaccess?Seeing_Display"]
        for text in urls {
            if let url = URL(string: text), NSWorkspace.shared.open(url) { return }
        }
    }
}
