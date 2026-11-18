import AppKit
import SwiftUI
import ZhuyinEngine

/// 介紹: the typing story, then the main features, each leading to its settings.
struct IntroPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        SettingsForm {
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
            }
            SettingsSection("目前的設定") {
                SummaryRow(title: "空白鍵", value: OptionDemos.spaceLabel(preferences.spaceAction),
                           place: SettingsLinks.Summary.space)
                SummaryRow(title: "標點按法", value: preferences.punctuationKeys == .windows ? "Windows" : "macOS",
                           place: SettingsLinks.Summary.punctuation)
                SummaryRow(title: "按錯鍵修正", value: ["關閉", "標準", "積極"][[TypoSetting.off, .standard, .aggressive]
                    .firstIndex(of: preferences.typoTolerance) ?? 1], place: SettingsLinks.Summary.typo)
                SummaryRow(title: "中英切換", value: CurrentKeys(preferences).modeKey.map { "按 \($0)" } ?? "不使用",
                           place: SettingsLinks.Summary.mode)
            }
            SettingsSection("設定嚮導") {
                SettingLinkRow(title: "開始設定嚮導", subtitle: "選擇習慣的打法，並在嚮導中試打") {
                    WizardWindowController.shared.show(.quick)
                }
                if WizardProgress.hasNewLessons {
                    SettingLinkRow(title: "新功能導覽", subtitle: "只介紹這一版新增的功能") {
                        WizardWindowController.shared.show(.newSince(WizardProgress.seenVersion ?? "0"))
                    }
                }
                SettingLinkRow(title: "你會用到的按鍵", subtitle: "依目前的設定列出按鍵") {
                    WizardWindowController.shared.show(.keysOnly)
                }
            }
            SettingsSection("功能", card: false) {
                FeatureGrid(features: IntroFeature.all(CurrentKeys(preferences), preferences: preferences))
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
            ? "⇧ 加符號鍵打出鍵上的符號" + (keys.isOn(.controlPunctuation) ? "，⌃ 加符號鍵打中文標點" : "") + "。標點預設全形，可切換全半形。"
            : "⇧ 加符號鍵打出中文標點。標點預設全形，可切換全半形。"
        let wordKeys = [keys.text(.blockCandidate), keys.text(.addCandidatePhrase)].compactMap { $0 }
        let quick = keys.text(.quickSettings).map { "按 \($0) " } ?? "從輸入法選單"
        // What the card says about corrections follows 標示自動修正 and ⌫ 還原自動修正 (Linux/Windows `intro.rs`).
        let corrections = switch (preferences.showCorrections, preferences.undoCorrectionWithBackspace) {
        case (true, true): "；修正過的字加點線，按 ⌫ 改回"
        case (true, false): "；修正過的字加點線"
        case (false, true): "；按 ⌫ 改回修正"
        case (false, false): ""
        }
        var features = [
            IntroFeature(title: "整句輸入", text: "一句話連續打完，依讀音與前後文選出最通順的寫法。", symbol: "text.word.spacing",
                         tint: .blue, place: .section(.choosing, "轉換")),
            IntroFeature(title: "按錯鍵也能打對", text: "按到相鄰的鍵、多按、漏按或順序打反時試著修正，照打的字優先\(corrections)。",
                         symbol: "wand.and.stars", tint: .indigo, place: SettingsPlace(.corrections, row: SettingsRow.typo)),
            IntroFeature(title: "依前文選字", text: "參考游標前最多 \(ContextPane.readLimit) 個字，並接續同一個 App 最近 \(ContextPane.minutes) 分鐘的前文。",
                         symbol: "text.alignleft", tint: .teal, place: SettingsPlace(.choosing, row: SettingsRow.context)),
            IntroFeature(title: "聯想詞", text: "送出後列出常接在後面的詞，按 ↓ 進入清單再選；預設關閉。", symbol: "text.append",
                         tint: .cyan, place: SettingsPlace(.choosing, row: SettingsRow.associatedPhrases)),
            IntroFeature(title: "記住你的用法", text: "選過的字詞下次直接採用；資料只存在這台 Mac，也可以暫停學習。", symbol: "clock.arrow.circlepath",
                         tint: .purple, place: SettingsPlace(.words, row: SettingsRow.learning)),
            IntroFeature(title: "在候選字視窗管理字詞",
                         text: "在候選字上按右鍵" + (wordKeys.isEmpty ? "" : "或按 " + wordKeys.joined(separator: "、"))
                             + "，不再推薦某個字詞或加入自訂詞；App 裡選取的字也能加入。",
                         symbol: "character.book.closed", tint: .orange, place: .section(.words, "在候選字視窗中")),
            IntroFeature(title: "標點與全半形", text: punctuation + (keys.text(.togglePunctuationWidth).map { "按 \($0) 切換。" } ?? "可在「標點與符號」設定快捷鍵。"), symbol: "quote.opening", tint: .orange,
                         place: ShortcutCommand.togglePunctuationWidth.place),
        ]
        if SymbolSources.isAvailable {
            features.append(IntroFeature(title: "符號與 Emoji",
                                         text: (keys.text(.symbolPicker).map { "按 \($0) " } ?? "") + "開啟面板，用注音或英文搜尋符號與 Emoji。",
                                         symbol: "face.smiling", tint: .yellow, place: SettingsPlace(.punctuation, row: SettingsRow.symbolPicker)))
        }
        features += [
            IntroFeature(title: "快速設定", text: quick + "在游標旁切換英文、半形標點、簡體與暫停學習。", symbol: "switch.2",
                         tint: .blue, place: ShortcutCommand.quickSettings.place),
            IntroFeature(title: "色彩與無障礙", text: "候選字視窗的淺色、深色與強調色；跟著系統的增加對比與減少動態效果。",
                         symbol: "paintpalette", tint: .pink, place: SettingsPlace(.colors)),
            IntroFeature(title: "檢查輸入法", text: "找出無法打字的常見原因並修正；回報問題時整理成一段可以拷貝的文字。",
                         symbol: "stethoscope", tint: .green, place: SettingsPlace(.health)),
            IntroFeature(title: "改字記錄", text: "可以記錄改字的過程，用來改進選字；只存在這台 Mac。", symbol: "pencil.and.list.clipboard",
                         tint: .gray, place: .section(.advanced, "改字記錄")),
            IntroFeature(title: "使用分析", text: "打了多少字、改了幾次、輸入法反應多快。", symbol: "chart.bar.fill", tint: .green,
                         place: SettingsPlace(.analytics)),
        ]
        return features
    }
}

struct FeatureGrid: View {
    let features: [IntroFeature]
    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: RowMetrics.tileSpacing, alignment: .top),
                            GridItem(.flexible(), spacing: RowMetrics.tileSpacing, alignment: .top)],
                  alignment: .leading, spacing: RowMetrics.tileSpacing) {
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
        .frame(maxWidth: .infinity, minHeight: 100, alignment: .topLeading)
        .padding(RowMetrics.tilePadding)
        .background(RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous).fill(Color.primary.opacity(0.03)))
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
