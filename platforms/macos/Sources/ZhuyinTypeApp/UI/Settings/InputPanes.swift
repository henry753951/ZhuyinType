import AppKit
import SwiftUI
import ZhuyinEngine

// The pages of the 輸入 and 選字與字詞 groups (docs/product/SETTINGS_IA.md §2): 打字, 標點與符號, 中英切換, 選字, 修正 and
// 字詞.  Each page holds one thing; a feature's switch, keys and explanation stay together.

// MARK: - 打字

struct TypingPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        SettingsForm {
            SettingsSection("空白與 Esc") {
                RegistryRow("spaceAction", preferences, accessory: .info(title: "空白鍵", content: AnyView(SpaceInfo(current: preferences.spaceAction))))
                RegistryRow("escape", preferences, accessory: .info(title: "Esc", content: AnyView(EscapeInfo(clears: preferences.escapeClearsComposition))))
            }
            SettingsSection("編輯組字") {
                FeatureRow(preferences: preferences, title: "送出並換行", subtitle: "送出組字，App 再換行",
                           commands: [.commitWithNewline], anchor: SettingsRow.newline)
                FeatureRow(preferences: preferences, title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音",
                           feature: .deleteWord, commands: [.deleteWord], anchor: SettingsRow.deleteWord)
                RegistryRow("keys.insertSpace", preferences)
                DrillRow(model: model, title: "所有打字的按鍵", subtitle: "注音鍵、聲調、⌫、方向鍵等固定的按鍵",
                         value: "", subpage: .typingKeys)
            } footer: {
                KeysOverviewLink(section: .typing)
            }
        }
    }
}

// MARK: - 標點與符號

struct PunctuationPane: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        SettingsForm {
            SettingsSection("標點") {
                RegistryRow("punctuationKeys", preferences, accessory: .info(title: "標點按法", content: AnyView(PunctuationTable(current: preferences.punctuationKeys))))
                RegistryRow("keys.togglePunctuationWidth", preferences,
                            accessory: .info(title: "標點全半形", content: AnyView(PunctuationWidthInfo(preferences: preferences))))
            } footer: {
                FooterText("中文預設全形，英數與空白維持半形。英文模式使用半形。")
            }
            if SymbolSources.isAvailable {
                SettingsSection("符號與 Emoji") {
                    FeatureRow(preferences: preferences, title: "符號與 Emoji 面板", subtitle: "開啟面板，用注音或英文搜尋符號與 Emoji",
                               feature: .symbolPicker, commands: [.symbolPicker], anchor: SettingsRow.symbolPicker)
                }
            }
            SettingsSection("修飾鍵打標點") {
                FeatureRow(preferences: preferences, title: "⌃ 加符號鍵打中文標點", subtitle: "，。、？！：；『』，兩種標點按法都能用",
                           feature: .controlPunctuation, anchor: SettingsRow.controlPunctuation)
                FeatureRow(preferences: preferences, title: "⌥ 加數字或符號鍵打鍵上的符號", subtitle: ". / ; < >，兩種標點按法都能用",
                           feature: .optionSymbols, anchor: SettingsRow.optionSymbols)
            }
            SettingsSection("固定的按鍵") {
                SettingRow(title: "「 」 、", subtitle: "固定") { KeyCombo(keys: ["[", "]", "'"]) }
                SettingRow(title: "列出同類的標點", subtitle: "在標點上按") { KeyCombo(keys: ["↓"]) }
            } footer: {
                KeysOverviewLink(section: .punctuation)
            }
        }
    }
}

// MARK: - 中英切換

struct ModePane: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        SettingsForm {
            SettingsSection("切換鍵") {
                RegistryRow("shiftSwitch", preferences)
                if preferences.shiftSwitchEnabled {
                    SettingRow(title: "使用的 Shift", indent: true, anchor: SettingsRow.shiftSide) {
                        RowPicker(title: "使用的 Shift", selection: Binding(
                            get: { preferences.shiftToggle == .none ? .both : preferences.shiftToggle },
                            set: { preferences.shiftToggle = $0 }),
                                  options: [("左右 Shift", ShiftToggle.both), ("左 Shift", .left), ("右 Shift", .right)])
                    }
                }
                FeatureRow(preferences: preferences, title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出",
                           feature: .capsLockEnglish, anchor: SettingsRow.capsLock)
            }
            SettingsSection("預設與提示") {
                RegistryRow("startInEnglish", preferences)
                RegistryRow("showModeToast", preferences)
            }
            SettingsSection("快速設定") {
                RegistryRow("keys.quickSettings", preferences)
            } footer: {
                VStack(alignment: .leading, spacing: 6) {
                    Text("輸入法選單也有快速設定的四個項目。其中三項也在各自的頁面：")
                        .fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: 14) {
                        PlaceLink(title: "標點全半形", place: ShortcutCommand.togglePunctuationWidth.place)
                        PlaceLink(title: "繁／簡", place: SettingsPlace(.choosing, row: SettingsRow.outputScript))
                        PlaceLink(title: "暫停學習", place: SettingsPlace(.words, row: SettingsRow.pauseLearning))
                    }
                    KeysOverviewLink(section: .mode)
                }
            }
        }
    }
}

// MARK: - 選字

struct ChoosingPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        SettingsForm {
            SettingsSection("轉換") {
                RegistryRow("candidateAnchor", preferences, accessory: .info(title: "游標在句中時改", content: AnyView(AnchorInfo(current: preferences.candidateAnchor))))
                RegistryRow("characterForms", preferences, segmented: true)
                RegistryRow("outputScript", preferences, segmented: true)
            }
            SettingsSection("前文") {
                DrillRow(model: model, title: "參考游標前的文字", subtitle: "依前文選字；只在記憶體中",
                         value: preferences.usesPrecedingText ? "開啟" : "關閉", subpage: .context, anchor: SettingsRow.context)
            }
            SettingsSection("聯想詞") {
                RegistryRow("associatedPhrases", preferences)
            }
        }
    }
}

// MARK: - 修正

struct CorrectionsPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel

    var body: some View {
        let fuzzy = [FuzzySounds.zZh, .cCh, .sSh, .enEng, .anAng].filter { preferences.fuzzySounds.contains($0) }.count
        SettingsForm {
            SettingsSection("按錯鍵") {
                RegistryRow("typoTolerance", preferences, accessory: .info(title: "按錯鍵修正", content: AnyView(TypoInfo(
                    current: preferences.typoTolerance, correctionsKey: CurrentKeys(preferences).text(.correctionsList)))))
                RegistryRow("readingChannel", preferences)
                DrillRow(model: model, title: "模糊音", subtitle: "ㄗ／ㄓ、ㄣ／ㄥ 等容易混的音",
                         value: fuzzy == 0 ? "關閉" : "已開啟 \(fuzzy) 項", subpage: .fuzzy, anchor: SettingsRow.fuzzy)
            }
            SettingsSection("修正過的字") {
                RegistryRow("showCorrections", preferences)
                RegistryRow("undoCorrectionWithBackspace", preferences)
                FeatureRow(preferences: preferences, title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原",
                           feature: .correctionsList, commands: [.correctionsList], anchor: SettingsRow.corrections)
            }
            SettingsSection("改字") {
                FeatureRow(preferences: preferences, title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復",
                           feature: .typeZhuyin, commands: [.typeZhuyin], anchor: SettingsRow.typeZhuyin)
                FeatureRow(preferences: preferences, title: "送出後刪掉重打",
                           subtitle: "送出後 30 秒內用 ⌫ 刪掉再打，算作改字並接著前文選字",
                           feature: .deleteAfterCommit, anchor: SettingsRow.deleteAfterCommit)
            } footer: {
                KeysOverviewLink(section: .corrections)
            }
        }
    }
}

// MARK: - 字詞

struct WordsPane: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var model: SettingsModel
    @State private var counts: (learned: Int, phrases: Int)?
    /// 暫停學習 lives in memory (`QuickSettings`), shared with the input menu and 快速設定.
    @State private var paused = QuickSettings.learningPaused

    var body: some View {
        SettingsForm {
            SettingsSection("記住") {
                RegistryRow("learningEnabled", preferences, accessory: .info(title: "記住的字詞", content: AnyView(LearnedInfo())))
                SettingRow(title: "暫停學習", subtitle: "照常打字，但不記住選過的字詞、不讀取前文；關閉、重新登入或更新後恢復",
                           dimmed: !preferences.learningEnabled, anchor: SettingsRow.pauseLearning) {
                    RowSwitch(title: "暫停學習", isOn: Binding(get: { paused }, set: { QuickSettings.learningPaused = $0 }))
                }
                DrillRow(model: model, title: "記住的字詞", subtitle: "檢視、移除、匯出或匯入",
                         value: counts.map { "\($0.learned.formatted()) 個" } ?? "", subpage: .learnedWords)
            }
            SettingsSection("自訂詞") {
                DrillRow(model: model, title: "自訂詞", subtitle: "選字時優先列出",
                         value: counts.map { "\($0.phrases.formatted()) 個" } ?? "", subpage: .phrases)
                FeatureRow(preferences: preferences, title: "選取加入自訂詞", subtitle: "在組字中選取 1 到 6 個字，再按 Return",
                           feature: .selectPhrase, commands: [.selectLeft, .selectRight], anchor: SettingsRow.selectPhrase)
                RegistryRow("keys.addSelectedText", preferences)
            }
            SettingsSection("在候選字視窗中") {
                RegistryRow("keys.blockCandidate", preferences)
                RegistryRow("keys.forgetCandidate", preferences)
                RegistryRow("keys.addCandidatePhrase", preferences)
            } footer: {
                VStack(alignment: .leading, spacing: 6) {
                    Text("也可以在候選字上按右鍵。5 秒內按 ⌃Z 或點提示列的「復原」還原。")
                        .fixedSize(horizontal: false, vertical: true)
                    KeysOverviewLink(section: .words)
                }
            }
        }
        .onAppear(perform: count)
        .onChange(of: model.engine != nil) { _, _ in count() }
        .onReceive(NotificationCenter.default.publisher(for: QuickSettings.didChange)) { _ in
            paused = QuickSettings.learningPaused
        }
    }

    private func count() {
        guard let engine = model.engine else { return }
        counts = (engine.learnedEntries().count, engine.userPhrases().count)
    }
}
