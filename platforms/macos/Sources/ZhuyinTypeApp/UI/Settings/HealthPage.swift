import AppKit
import SwiftUI
import UniformTypeIdentifiers

// 關於 › 檢查輸入法: the checks (Health/HealthChecks.swift) as rows with their buttons, and 回報問題.  Views only; the
// checks and the report text come from HealthMonitor.

struct HealthPage: View {
    @ObservedObject var monitor: HealthMonitor = .shared
    @State private var showingReport = false
    @State private var reportText: String?

    var body: some View {
        SettingsForm {
            SettingsSection {
                HealthSummary(monitor: monitor)
            }
            SettingsSection("檢查項目") {
                ForEach(monitor.checks) { HealthRow(check: $0) }
            }
            SettingsSection {
                SettingRow(title: "回報問題", subtitle: "把檢查結果、版本與最近的錯誤整理成一段文字，可以拷貝或存成檔案") {
                    Button("回報問題…") {
                        reportText = nil
                        showingReport = true
                        monitor.makeReport { reportText = $0 }
                    }
                }
            } footer: {
                FooterText("內容不含你打的字、自訂詞或記住的字詞，也不會自動送出。")
            }
        }
        .onAppear { monitor.refreshIfNeeded() }
        .sheet(isPresented: $showingReport) { HealthReportSheet(text: reportText) }
    }
}

/// The page's top: how many items need attention, when the checks ran, and 「重新檢查」.
private struct HealthSummary: View {
    @ObservedObject var monitor: HealthMonitor

    var body: some View {
        let attention = monitor.needsAttention
        HStack(spacing: 12) {
            Image(systemName: attention ? "exclamationmark.triangle.fill" : "checkmark.seal.fill")
                .font(.system(size: 26))
                .foregroundStyle(attention ? Color.orange : Color.green)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 3) {
                Text(monitor.summary).font(.headline)
                if let checkedAt = monitor.checkedAt {
                    Text("檢查時間：" + checkedAt.formatted(date: .omitted, time: .shortened))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button("重新檢查") { monitor.refresh() }
        }
        .accessibilityElement(children: .combine)
    }
}

/// One check: its status icon, title and sentence, and its buttons below.
struct HealthRow: View {
    let check: HealthCheck

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            HealthStatusIcon(status: check.status)
            VStack(alignment: .leading, spacing: 3) {
                Text(check.title)
                Text(check.explanation)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
                if !check.actions.isEmpty {
                    HStack(spacing: 8) {
                        ForEach(check.actions, id: \.self) { action in
                            Button(action.title) { HealthActions.perform(action) }
                        }
                    }
                    .padding(.top, 5)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(minHeight: RowMetrics.minHeight)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(check.status.label)：\(check.title)")
    }
}

struct HealthStatusIcon: View {
    let status: HealthStatus

    var body: some View {
        Image(systemName: symbol)
            .foregroundStyle(color)
            .frame(width: 18)
            .help(status.label)
            .accessibilityLabel(status.label)
    }

    private var symbol: String {
        switch status {
        case .ok: return "checkmark.circle.fill"
        case .info: return "info.circle.fill"
        case .warning: return "exclamationmark.triangle.fill"
        case .error: return "xmark.octagon.fill"
        }
    }

    private var color: Color {
        switch status {
        case .ok: return .green
        case .info: return .blue
        case .warning: return .orange
        case .error: return .red
        }
    }
}

/// 回報問題's sheet: the exact text, read-only and selectable, to copy or save.  Nothing is sent anywhere.
struct HealthReportSheet: View {
    /// Nil while the report is being put together (at most a moment).
    let text: String?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                Text("問題回報").font(.headline)
                Text("回報問題時，把這段文字貼上或附上檔案。").font(.callout).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            Divider()
            Group {
                if let text {
                    ScrollView {
                        Text(text)
                            .font(.system(size: 12, design: .monospaced))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(16)
                    }
                } else {
                    ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: .textBackgroundColor))
            Divider()
            HStack(spacing: 8) {
                Button("拷貝") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text ?? "", forType: .string)
                }
                .disabled(text == nil)
                Button("儲存為檔案…") { text.map(HealthReportSheet.save) }
                    .disabled(text == nil)
                Spacer()
                Button("關閉") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
        .frame(minWidth: 520, idealWidth: 620, minHeight: 400, idealHeight: 520)
    }

    private static func save(_ text: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "ZhuyinType 問題回報.txt"
        panel.allowedContentTypes = [.plainText]
        let write: (NSApplication.ModalResponse) -> Void = { response in
            guard response == .OK, let url = panel.url else { return }
            do { try text.write(to: url, atomically: true, encoding: .utf8) } catch {
                let alert = NSAlert()
                alert.messageText = "無法儲存"
                alert.informativeText = error.localizedDescription
                alert.runModal()
            }
        }
        if let window = NSApp.keyWindow { panel.beginSheetModal(for: window, completionHandler: write) }
        else { panel.begin(completionHandler: write) }
    }
}
