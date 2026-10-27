import AppKit
import SwiftUI
import ZhuyinCore

/// 介紹: the typing story, then the main features, each leading to its settings.
struct IntroPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            SettingsSection {
                VStack(spacing: 14) {
                    HStack(spacing: 14) {
                        Image(nsImage: NSApp.applicationIconImage)
                            .resizable()
                            .frame(width: 52, height: 52)
                        VStack(alignment: .leading, spacing: 3) {
                            Text("ZhuyinType").font(.title2.weight(.semibold))
                            Text("開源、在本機運作的注音輸入法").foregroundStyle(.secondary)
                        }
                        Spacer(minLength: 0)
                    }
                    IntroStage(preferences: preferences, pinned: DemoPinning.intro)
                }
                .padding(.vertical, 8)
            }
            SettingsSection("目前的設定") {
                SummaryRow(title: "空白鍵", value: preferences.spaceAction == .openCandidates ? "開啟候選字視窗" : "送出並輸入空白",
                           place: SettingsPlace(.typing, row: SettingsRow.space))
                SummaryRow(title: "標點按法", value: preferences.punctuationKeys == .windows ? "Windows" : "macOS",
                           place: SettingsPlace(.typing, row: SettingsRow.punctuationKeys))
                SummaryRow(title: "按錯鍵修正", value: ["關閉", "標準", "積極"][[TypoSetting.off, .standard, .aggressive]
                    .firstIndex(of: preferences.typoTolerance) ?? 1], place: SettingsPlace(.choosing, row: SettingsRow.typo))
                SummaryRow(title: "中英切換", value: CurrentKeys(preferences).modeKey.map { "按 \($0)" } ?? "不使用",
                           place: SettingsPlace(.typing, row: SettingsRow.shiftTap))
            }
            SettingsSection("功能") {
                FeatureGrid(features: IntroFeature.all(CurrentKeys(preferences), preferences: preferences))
                    .padding(.vertical, 6)
            }
        }
    }
}

struct IntroFeature: Identifiable {
    var id: String { title }
    var title: String
    var text: String
    var symbol: String
    var tint: Color
    var place: SettingsPlace

    static func all(_ keys: CurrentKeys, preferences: Preferences) -> [IntroFeature] {
        let punctuation = preferences.punctuationKeys == .windows
            ? "⇧ 加符號鍵打出鍵上的符號" + (keys.isOn(.controlPunctuation) ? "，⌃ 加符號鍵打中文標點" : "") + "，和 Windows 的注音相同。"
            : "⇧ 加符號鍵打出中文標點，和 macOS 的注音相同。"
        return [
            IntroFeature(title: "整句輸入", text: "一句話連續打完，依讀音與前後文選出最通順的寫法。", symbol: "text.word.spacing",
                         tint: .blue, place: SettingsPlace(.candidates)),
            IntroFeature(title: "按錯鍵也能打對", text: "按到相鄰的鍵、多按、漏按或順序打反時試著修正，照打的字優先。",
                         symbol: "wand.and.stars", tint: .indigo,
                         place: SettingsPlace(.choosing, row: SettingsRow.typo)),
            IntroFeature(title: "依前文選字", text: "參考游標前最多 \(ContextPane.readLimit) 個字，用語跟著偏向口語或書面。",
                         symbol: "text.alignleft", tint: .teal,
                         place: SettingsPlace(.choosing, subpage: .context)),
            IntroFeature(title: "記住你的用法", text: "選過的字詞下次直接採用；資料只存在這台 Mac。", symbol: "clock.arrow.circlepath",
                         tint: .purple, place: SettingsPlace(.words)),
            IntroFeature(title: "標點按法", text: punctuation, symbol: "quote.opening", tint: .orange,
                         place: SettingsPlace(.typing, row: SettingsRow.punctuationKeys)),
            IntroFeature(title: "改字記錄", text: "可以記錄改字的過程，用來改進選字；只存在這台 Mac。", symbol: "pencil.and.list.clipboard",
                         tint: .gray, place: SettingsPlace(.advanced)),
            IntroFeature(title: "使用分析", text: "打了多少字、改了幾次、輸入法反應多快。", symbol: "chart.bar.fill", tint: .green,
                         place: SettingsPlace(.analytics)),
        ]
    }
}

struct FeatureGrid: View {
    let features: [IntroFeature]
    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 12, alignment: .top), GridItem(.flexible(), spacing: 12, alignment: .top)],
                  alignment: .leading, spacing: 12) {
            ForEach(features) { feature in
                FeatureCard(feature: feature)
            }
        }
    }
}

private struct FeatureCard: View {
    let feature: IntroFeature
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: feature.symbol)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.white)
                    .frame(width: 24, height: 24)
                    .background(RoundedRectangle(cornerRadius: 6.5, style: .continuous).fill(feature.tint.gradient))
                Text(feature.title).font(.headline)
            }
            Text(feature.text)
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            PlaceLink(title: feature.place.subpage?.title ?? feature.place.pane.title, place: feature.place)
        }
        .padding(12)
        .frame(maxWidth: .infinity, minHeight: 128, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.quaternary.opacity(0.35)))
    }
}

/// A setting of another page and its value, leading there.
private struct SummaryRow: View {
    let title: String
    let value: String
    let place: SettingsPlace
    var body: some View {
        SettingLinkRow(title: title, value: value) {
            SettingsWindowController.shared.open(place)
        }
    }
}
