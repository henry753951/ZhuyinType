import SwiftUI
import ZhuyinEngine

// Still frames of the planned 設定嚮導, for the storyboard in docs/product/SETUP_WIZARD.md.  These are drawings for
// review only: nothing here is wired to settings or input.  The wizard itself is built after the flow is approved.

enum StoryboardFrame: String, CaseIterable {
    case welcome, preset, punctuation, paging, success, finish, keys
}

struct WizardStoryboard: View {
    let frame: StoryboardFrame
    @ObservedObject var preferences: Preferences

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().opacity(frame == .welcome ? 0 : 1)
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            footer
        }
        .frame(width: 720, height: 540)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    // MARK: Chrome

    private var step: (index: Int, count: Int)? {
        switch frame {
        case .welcome: return nil
        case .preset: return (1, 7)
        case .punctuation: return (3, 7)
        case .paging, .success: return (4, 7)
        case .finish, .keys: return (7, 7)
        }
    }

    @ViewBuilder private var header: some View {
        HStack(spacing: 12) {
            if frame != .welcome {
                Button { } label: { Label("上一步", systemImage: "chevron.left") }.buttonStyle(.borderless)
            }
            Spacer()
            if let step {
                HStack(spacing: 6) {
                    ForEach(1...step.count, id: \.self) { index in
                        Capsule()
                            .fill(index <= step.index ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.quaternary))
                            .frame(width: index == step.index ? 18 : 7, height: 7)
                    }
                }
            }
            Spacer()
            if frame != .finish && frame != .keys {
                Button("全部跳過") { }.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 44)
    }

    @ViewBuilder private var footer: some View {
        HStack {
            if [.punctuation, .paging].contains(frame) {
                Button("自行設定…") { }.buttonStyle(.link)
            }
            Spacer()
            switch frame {
            case .welcome: Button("開始") { }.keyboardShortcut(.defaultAction)
            case .preset: Button("下一步") { }.keyboardShortcut(.defaultAction)
            case .punctuation: Button("略過這一步") { }; Button("下一步") { }.disabled(true)
            case .paging: Button("略過這一步") { }; Button("下一步") { }.disabled(true)
            case .success: Button("下一步") { }.keyboardShortcut(.defaultAction)
            case .finish: Button("全部捨棄") { }; Button("套用") { }.keyboardShortcut(.defaultAction)
            case .keys: Button("完成") { }.keyboardShortcut(.defaultAction)
            }
        }
        .controlSize(.large)
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    @ViewBuilder private var content: some View {
        switch frame {
        case .welcome: welcome
        case .preset: preset
        case .punctuation: punctuation
        case .paging: paging(done: false)
        case .success: paging(done: true)
        case .finish: finish
        case .keys: keys
        }
    }

    // MARK: Frames

    private var welcome: some View {
        VStack(spacing: 14) {
            Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 72, height: 72).padding(.top, 28)
            Text("設定 ZhuyinType").font(.largeTitle.weight(.semibold))
            Text("選好你習慣的打法，並在這裡試打。所有設定之後都能在「設定」中更改。")
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 420)
            HStack(spacing: 12) {
                ChoiceCard(title: "快速設定", text: "標點、空白鍵、中英切換、選字與翻頁，約 2 分鐘", symbol: "bolt", selected: true)
                ChoiceCard(title: "完整導覽", text: "依序介紹每個功能，並在每一步試打", symbol: "map", selected: false)
            }
            .frame(maxWidth: 520)
            .padding(.top, 10)
        }
        .padding(.horizontal, 40)
    }

    private var preset: some View {
        VStack(alignment: .leading, spacing: 14) {
            Title(text: "你原本習慣哪一種注音輸入？", detail: "選一個最接近的習慣組合，之後每一步都可以再改。")
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                PresetCard(title: "Windows 注音的習慣", lines: ["單按 Shift 切換中英", "⌃ 加符號鍵打中文標點", "和目前的預設相同"], selected: true)
                PresetCard(title: "macOS 注音的習慣", lines: ["空白鍵開啟候選字視窗", "Caps Lock 切換中英", "單按 Shift 不切換"], selected: false)
                PresetCard(title: "ZhuyinType 預設", lines: ["⌃ 加符號鍵打中文標點", "空白鍵送出並輸入空白", "Shift 切換中英"], selected: false)
                PresetCard(title: "自訂", lines: ["從目前的設定開始", "每一步自己選"], selected: false)
            }
            Text("不改變設定").font(.callout).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 40)
        .padding(.top, 24)
    }

    private var punctuation: some View {
        VStack(alignment: .leading, spacing: 16) {
            Title(text: "標點", detail: "按住修飾鍵再按符號鍵打出標點。選一種按法，在下面試打。")
            Picker("標點按法", selection: .constant(1)) {
                Text("Windows：⌃ 打中文標點").tag(1)
                Text("macOS：⇧ 打中文標點").tag(2)
            }
            .pickerStyle(.segmented).labelsHidden()
            Task(text: "打出「好，謝謝。」", progress: "1 / 2")
            PracticeField(segments: [DemoSegment("好", .document), DemoSegment("，", .marked)], hint: ["⌃", ","])
            Text("⌃ 加 , 打出「，」；⇧ 加 , 打出鍵上的「<」。").font(.callout).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 40)
        .padding(.top, 24)
    }

    private func paging(done: Bool) -> some View {
        let words = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"]
        let list = CandidateList(items: words.map { Candidate(text: $0) }, selection: done ? 11 : 11, pageSize: 9, tolerant: false)
        return VStack(alignment: .leading, spacing: 14) {
            Title(text: "選字與翻頁", detail: "這些翻頁方式同時有效；不用的可以關掉。")
            HStack(alignment: .top, spacing: 18) {
                VStack(alignment: .leading, spacing: 8) {
                    StyleToggle(keys: "Tab", text: "Tab", on: true)
                    StyleToggle(keys: "fn↓", text: "Page Down（fn ↓）", on: true)
                    StyleToggle(keys: "⌥↓", text: "⌥↓", on: true)
                    StyleToggle(keys: "↓", text: "在頁底按 ↓ 翻到下一頁", on: true)
                    Picker("排列", selection: .constant(1)) { Text("直式").tag(1); Text("橫式").tag(2) }
                        .pickerStyle(.segmented).labelsHidden().frame(width: 160).padding(.top, 4)
                }
                .frame(width: 230, alignment: .leading)
                VStack(alignment: .leading, spacing: 10) {
                    Task(text: "打 ㄇㄧㄥˊ，翻到下一頁，選第 12 個字「暝」", progress: done ? "完成" : "第 2 頁")
                    if done {
                        PracticeField(segments: [DemoSegment("約了王", .document), DemoSegment("暝", .document)], done: true)
                        Label("完成", systemImage: "checkmark.circle.fill")
                            .font(.title3.weight(.semibold))
                            .foregroundStyle(Color(nsColor: .systemGreen))
                    } else {
                        PracticeField(segments: [DemoSegment("約了王", .document), DemoSegment("暝", .focus)], hint: ["fn", "↓"])
                        PanelSnapshot(list: list, zhuyinHint: "ㄇㄧㄥˊ", preferences: preferences).padding(.leading, 64)
                    }
                }
            }
        }
        .padding(.horizontal, 40)
        .padding(.top, 24)
    }

    private var finish: some View {
        VStack(alignment: .leading, spacing: 14) {
            Title(text: "確認要套用的設定", detail: "以下是這次改變的設定；可以在這裡改回原本的值或再調整。")
            VStack(spacing: 0) {
                ForEach(Array(changes.enumerated()), id: \.offset) { index, change in
                    if index > 0 { Divider().padding(.horizontal, 16) }
                    HStack(spacing: 10) {
                        Text(change.0).frame(width: 140, alignment: .leading)
                        Text(change.1).foregroundStyle(.secondary).strikethrough(true, color: .secondary)
                        Image(systemName: "arrow.right").font(.caption).foregroundStyle(.tertiary)
                        Text(change.2).fontWeight(.medium)
                        Spacer()
                        Button("改回") { }.buttonStyle(.link).font(.callout)
                    }
                    .padding(.horizontal, 16)
                    .frame(height: 40)
                }
            }
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.03)))
        }
        .padding(.horizontal, 40)
        .padding(.top, 24)
    }

    private var changes: [(String, String, String)] {
        [("按錯鍵修正", "標準", "積極"), ("翻頁", "Tab、fn↓、⌥↓", "Tab、fn↓"), ("記錄改字", "關閉", "開啟")]
    }

    private var keys: some View {
        VStack(alignment: .leading, spacing: 14) {
            Title(text: "你會用到的按鍵", detail: "依你的設定產生；之後可以在「介紹」再看一次。")
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 24), GridItem(.flexible(), spacing: 24)], alignment: .leading,
                      spacing: 10) {
                ForEach(Array(summary.enumerated()), id: \.offset) { _, item in
                    HStack(spacing: 10) {
                        HStack(spacing: 4) { ForEach(item.0, id: \.self) { KeyChip(text: $0) } }
                            .frame(width: 120, alignment: .leading)
                        Text(item.1)
                    }
                }
            }
        }
        .padding(.horizontal, 40)
        .padding(.top, 24)
    }

    private var summary: [([String], String)] {
        [(["空白"], "送出並輸入空白"), (["1–9"], "選字"), (["Tab", "fn↓"], "下一頁"), (["⇧Tab", "fn↑"], "上一頁"),
         (["⌃,"], "中文標點「，」"), (["Shift"], "切換中英"), (["⇧↑"], "列出修正的字"), (["⇧↓"], "只列一個字"),
         (["`"], "打出注音本身"), (["⇧←", "⇧→"], "選取，再按 Return 加入自訂詞"), (["⌥⌫"], "刪掉一個詞"), (["Esc"], "清除注音")]
    }
}

// MARK: - Pieces

private struct Title: View {
    let text: String
    let detail: String
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(text).font(.title2.weight(.semibold))
            Text(detail).foregroundStyle(.secondary)
        }
    }
}

private struct ChoiceCard: View {
    let title: String
    let text: String
    let symbol: String
    let selected: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Image(systemName: symbol).font(.title3).foregroundStyle(Color.accentColor)
            Text(title).font(.headline)
            Text(text).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, minHeight: 96, alignment: .topLeading)
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.03)))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous)
            .strokeBorder(selected ? Color.accentColor : Color.primary.opacity(0.08), lineWidth: selected ? 2 : 1))
    }
}

private struct PresetCard: View {
    let title: String
    let lines: [String]
    let selected: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.headline)
                Spacer()
                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(selected ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
            }
            ForEach(lines, id: \.self) { Text($0).font(.callout).foregroundStyle(.secondary) }
        }
        .frame(maxWidth: .infinity, minHeight: 104, alignment: .topLeading)
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.03)))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous)
            .strokeBorder(selected ? Color.accentColor : Color.primary.opacity(0.08), lineWidth: selected ? 2 : 1))
    }
}

private struct Task: View {
    let text: String
    let progress: String
    var body: some View {
        HStack {
            Label(text, systemImage: "keyboard").font(.headline)
            Spacer()
            Text(progress).font(.callout).foregroundStyle(.secondary)
        }
    }
}

private struct PracticeField: View {
    let segments: [DemoSegment]
    var hint: [String] = []
    var done = false
    var body: some View {
        HStack(spacing: 10) {
            DemoLine(segments: segments, fontSize: 18, caretBlinks: false)
            if !hint.isEmpty { PressedKeycap(keys: hint, animated: false) }
            Spacer()
        }
        .padding(.horizontal, 14)
        .frame(height: 48)
        .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(Color(nsColor: .textBackgroundColor)))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous)
            .strokeBorder(done ? Color(nsColor: .systemGreen) : Color.accentColor.opacity(0.6), lineWidth: done ? 2 : 1.5))
    }
}

private struct StyleToggle: View {
    let keys: String
    let text: String
    let on: Bool
    var body: some View {
        HStack(spacing: 8) {
            Toggle(text, isOn: .constant(on)).toggleStyle(.checkbox).labelsHidden()
            KeyChip(text: keys)
            Text(text).font(.callout).lineLimit(1)
        }
    }
}
