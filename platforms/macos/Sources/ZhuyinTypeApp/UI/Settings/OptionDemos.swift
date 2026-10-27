import SwiftUI
import ZhuyinEngine

// What each option of the 一般 page does, as short looping demos for its ⓘ popover.  The texts are what the option
// really does (docs/product/DESIGN.md §2); keep them in step with the code.

enum OptionDemos {
    private static let start = DemoSegment("早安，", .document)

    // MARK: Space

    static func space(_ action: SpaceAction) -> [DemoFrame] {
        let last: DemoFrame
        switch action {
        case .openCandidates:
            last = DemoFrame(keys: ["空白"], line: [start, DemoSegment("今天", .focus)], list: ["今天", "金田", "今"],
                             caption: "再按一次：開啟候選字視窗", hold: 2.0)
        case .commit, .commitAndSpace:
            last = DemoFrame(keys: ["空白"], line: [DemoSegment("早安，今天 ", .document)], caption: "再按一次：送出並輸入空白",
                             hold: 2.0)
        case .insertSpace:
            last = DemoFrame(keys: ["空白"], line: [start, DemoSegment("今天 ")], caption: "再按一次：在組字中輸入空白，還不送出",
                             hold: 2.0)
        }
        return [
            DemoFrame(line: [start, DemoSegment("今"), DemoSegment("ㄊ")], keys: ["ㄊ"], caption: "打 ㄐㄧㄣ ㄊㄧㄢ", hold: 0.35),
            DemoFrame(line: [start, DemoSegment("今"), DemoSegment("ㄊㄧㄢ")], keys: ["ㄢ"], caption: "打 ㄐㄧㄣ ㄊㄧㄢ", hold: 1.0),
            DemoFrame(line: [start, DemoSegment("今天")], keys: ["空白"], caption: "完成一聲", hold: 1.2),
            last,
        ]
    }

    /// The option's name, as the 空白鍵 picker shows it.
    static func spaceLabel(_ action: SpaceAction) -> String {
        switch action {
        case .openCandidates: return "開啟候選字視窗"
        case .commit, .commitAndSpace: return "送出並輸入空白"
        case .insertSpace: return "輸入空白，不送出"
        }
    }

    static func spaceSubtitle(_ action: SpaceAction) -> String {
        switch action {
        case .openCandidates: return "打完注音後，再按空白開啟候選字視窗"
        case .commit, .commitAndSpace: return "打完注音後，再按空白送出組字並輸入空白"
        case .insertSpace: return "打完注音後，再按空白在組字中輸入空白，不送出"
        }
    }

    // MARK: Esc

    static func escape(clearsComposition: Bool) -> [DemoFrame] {
        [
            DemoFrame(line: [start, DemoSegment("今天"), DemoSegment("ㄑㄧ")], keys: ["ㄧ"], caption: "打到一半", hold: 1.1),
            DemoFrame(line: [start, DemoSegment("今天")], keys: ["Esc"], caption: "清除注音", hold: 1.2),
            clearsComposition
                ? DemoFrame(line: [start], keys: ["Esc"], caption: "再按一次：清除整段組字", hold: 2.0)
                : DemoFrame(line: [start, DemoSegment("今天")], keys: ["Esc"], caption: "再按一次：組字保留", hold: 2.0),
        ]
    }

    // MARK: Which character ↓ changes

    static func anchor(_ anchor: AnchorSetting) -> [DemoFrame] {
        let right = anchor == .after
        return [
            DemoFrame(line: [DemoSegment("今天"), DemoSegment("天氣")], caption: "游標在句尾：改最後的詞", hold: 1.4),
            DemoFrame(keys: ["←"], line: [DemoSegment("今天"), DemoSegment("天", right ? .marked : .target),
                                          DemoSegment("氣", right ? .target : .marked)], caret: 1,
                      caption: "游標在句中：底線標出要改的字", hold: 1.6),
            DemoFrame(keys: ["↓"], line: [DemoSegment("今天"), DemoSegment("天", right ? .marked : .focus),
                                          DemoSegment("氣", right ? .focus : .marked)], caret: 1,
                      list: right ? ["氣", "器", "汽"] : ["天", "添", "田"],
                      caption: right ? "改游標右邊的字" : "改游標左邊的字", hold: 2.0),
        ]
    }

    // MARK: Typing corrections

    /// `correctionsKey`: nil when the corrections list is turned off, and the demo ends at the correction.
    static func typo(correctionsKey: String?) -> [DemoFrame] {
        var frames = [
            DemoFrame(keys: ["ㄍ"], line: [start, DemoSegment("ㄍ")], caption: "想打 ㄐ，按到旁邊的 ㄍ", hold: 1.2),
            DemoFrame(keys: ["ㄣ"], line: [start, DemoSegment("ㄍㄧㄣ")], caption: "想打 ㄐ，按到旁邊的 ㄍ", hold: 0.9),
            DemoFrame(keys: ["空白"], line: [start, DemoSegment("今")], caption: "自動改成「今」", hold: 1.4),
        ]
        if let correctionsKey {
            frames.append(DemoFrame(keys: [correctionsKey], line: [start, DemoSegment("今", .focus)],
                                    list: ["今 ㄐㄧㄣ", "金 ㄐㄧㄣ", "斤 ㄐㄧㄣ"], caption: "列出修正的字", hold: 2.0))
        }
        return frames
    }

    static func typoSubtitle(_ setting: TypoSetting) -> String {
        switch setting {
        case .off: return "照按下的鍵解讀注音"
        case .standard: return "修正按到相鄰的鍵、多按、漏按、順序與聲調"
        case .aggressive: return "比「標準」更容易採用修正"
        }
    }

    // MARK: Fuzzy sounds

    static let fuzzy: [DemoFrame] = [
        DemoFrame(keys: ["ㄙ"], line: [DemoSegment("今天很", .document), DemoSegment("ㄙ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 0.5),
        DemoFrame(keys: ["ㄨ"], line: [DemoSegment("今天很", .document), DemoSegment("ㄙㄨ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 0.9),
        DemoFrame(keys: ["ˊ"], line: [DemoSegment("今天很", .document), DemoSegment("舒服")],
                  caption: "開啟「ㄙ 與 ㄕ」：也打得出「舒服」", hold: 2.2),
    ]
}

extension DemoFrame {
    /// Keys first reads better at the call sites of the typing steps.
    init(line: [DemoSegment], keys: [String], caption: String, hold: Double) {
        self.init(keys: keys, line: line, caret: nil, list: nil, caption: caption, hold: hold)
    }
}

/// What Shift, Control and Option with a symbol key type, for both 標點按法 settings.
struct PunctuationTable: View {
    let current: PunctuationKeys
    private static let rows: [(keys: [String], windows: String, mac: String)] = [
        (["⇧", ","], "<", "，"),
        (["⇧", "/"], "?", "？"),
        (["⇧", "1"], "!", "！"),
        (["⇧", "-"], "_", "——"),
        (["⌃", ","], "，", "，"),
        (["⌥", ","], ",", ","),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 6) {
                GridRow {
                    Text("按鍵").foregroundStyle(.secondary)
                    column("Windows", .windows)
                    column("macOS", .mac)
                }
                .font(.callout)
                Divider().gridCellUnsizedAxes(.horizontal)
                ForEach(Array(Self.rows.enumerated()), id: \.offset) { _, row in
                    GridRow {
                        KeyCombo(keys: row.keys, size: 11)
                        output(row.windows, .windows)
                        output(row.mac, .mac)
                    }
                }
            }
            Text("⌃ 加符號鍵打中文標點、⌥ 加符號鍵打鍵上的符號，兩種按法都能用。")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func column(_ title: String, _ keys: PunctuationKeys) -> some View {
        Text(title)
            .fontWeight(keys == current ? .semibold : .regular)
            .foregroundStyle(keys == current ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.secondary))
            .frame(minWidth: 64, alignment: .leading)
    }

    private func output(_ text: String, _ keys: PunctuationKeys) -> some View {
        Text(text)
            .font(.system(size: 15, weight: keys == current ? .medium : .regular))
            .foregroundStyle(keys == current ? AnyShapeStyle(.primary) : AnyShapeStyle(.tertiary))
            .frame(minWidth: 64, alignment: .leading)
    }
}
