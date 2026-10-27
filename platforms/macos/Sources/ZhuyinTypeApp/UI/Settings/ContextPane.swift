import SwiftUI

/// 前文: what the input method reads before the cursor and keeps of what was sent, shown rather than described.
/// The numbers are ControllerCore's (`contextRead`, `contextKept`, `contextWindow`).
struct ContextPane: View {
    @ObservedObject var preferences: Preferences
    var pinnedScene: Int?

    static var readLimit: Int { ControllerCore.contextRead }
    static var keptLimit: Int { ControllerCore.contextKept }
    static var minutes: Int { Int(ControllerCore.contextWindow / 60) }

    var body: some View {
        let on = preferences.usesPrecedingText
        Form {
            Section {
                ContextIllustration(pinned: pinnedScene)
                    .padding(.vertical, 6)
            }
            Section {
                Toggle(isOn: binding(preferences, \.usesPrecedingText)) {
                    RowLabel("參考游標前的文字", "依前文選字，並調整成口語或書面的用語")
                }
            }
            Section("讀取與保留") {
                FactLabel(symbol: "text.viewfinder", title: "讀取", value: on ? "游標前最多 \(Self.readLimit) 個字" : "不讀取")
                FactLabel(symbol: "text.append", title: "送出的字",
                          value: on ? "接在前文後面，保留最近 \(Self.keptLimit) 個字" : "只接著這個輸入框剛送出的字")
                if on {
                    FactLabel(symbol: "rectangle.on.rectangle", title: "App 不提供前文時",
                              value: "沿用同一個 App \(Self.minutes) 分鐘內送出的字")
                }
                FactLabel(symbol: "lock.fill", title: "密碼欄位", value: "不讀取，也不學習")
                FactLabel(symbol: "memorychip", title: "存放位置", value: "只在記憶體中")
            }
        }
    }
}

/// A fact about the current setting: an icon and title on the leading side, the value on the trailing side.
struct FactLabel: View {
    let symbol: String
    let title: String
    let value: String
    var body: some View {
        LabeledContent {
            Text(value).foregroundStyle(.secondary).multilineTextAlignment(.trailing)
        } label: {
            Label {
                Text(title)
            } icon: {
                Image(systemName: symbol).foregroundStyle(Color.accentColor)
            }
        }
    }
}

/// Three short scenes: the text before the cursor being read, what was sent carrying over to another field of the
/// same app, and a password field that is never read.
struct ContextIllustration: View {
    var pinned: Int?
    @State private var index = 0
    @State private var playing = true
    @State private var started = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let holds: [Double] = [2.4, 2.6, 3.2, 2.8]
    private static let captions = [
        "開始組字時，讀取游標前最多 \(ContextPane.readLimit) 個字",
        "接著打的字，依前文選字",
        "App 不提供前文時，沿用同一個 App \(ContextPane.minutes) 分鐘內送出的字",
        "密碼欄位：不讀取，也不學習",
    ]

    var body: some View {
        let scene = min(pinned ?? index, Self.holds.count - 1)
        VStack(alignment: .leading, spacing: 10) {
            ZStack {
                switch scene {
                case 0, 1: readingScene(typed: scene == 1).transition(.opacity)
                case 2: carryOverScene.transition(.opacity)
                default: secureScene.transition(.opacity)
                }
            }
            .frame(maxWidth: .infinity, minHeight: 116, maxHeight: 116)
            .animation(reduceMotion || pinned != nil ? nil : .easeInOut(duration: 0.35), value: scene)
            HStack(spacing: 8) {
                if pinned == nil { PlayPauseButton(playing: $playing) }
                Text(Self.captions[scene])
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.85)
                Spacer(minLength: 0)
                HStack(spacing: 5) {
                    ForEach(0..<Self.holds.count, id: \.self) { dot in
                        Circle().fill(dot == scene ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.quaternary))
                            .frame(width: 6, height: 6)
                            .onTapGesture { index = dot }
                    }
                }
                .accessibilityHidden(true)
            }
        }
        .modifier(StoryClock(index: $index, holds: Self.holds, playing: playing && pinned == nil))
        .onAppear {
            guard !started else { return }
            started = true
            if reduceMotion { playing = false }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Self.captions.joined(separator: "。"))
    }

    // MARK: Scenes

    private func card<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            content()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color(nsColor: .textBackgroundColor)))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary, lineWidth: 1))
    }

    private func readingScene(typed: Bool) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "arrow.down.left").font(.caption)
                Text("最多 \(ContextPane.readLimit) 個字").font(.caption.weight(.medium))
            }
            .foregroundStyle(Color.accentColor)
            .padding(.leading, 12)
            card("備忘錄") {
                DemoLine(segments: [DemoSegment("讀書會改到星期四晚上，地點一樣在", .highlight)]
                         + (typed ? [DemoSegment("圖書館")] : []),
                         fontSize: 14, caretBlinks: pinned == nil)
            }
        }
    }

    private var carryOverScene: some View {
        HStack(spacing: 10) {
            card("輸入框 A") {
                DemoLine(segments: [DemoSegment("地點一樣在圖書館", .highlight)], fontSize: 13, showsCaret: false)
            }
            VStack(spacing: 2) {
                Image(systemName: "arrow.right").font(.system(size: 15, weight: .semibold))
                Text("\(ContextPane.minutes) 分鐘內").font(.caption2)
            }
            .foregroundStyle(Color.accentColor)
            .fixedSize()
            card("輸入框 B") {
                DemoLine(segments: [DemoSegment("七點見")], fontSize: 13, caretBlinks: pinned == nil)
            }
        }
    }

    private var secureScene: some View {
        card("密碼") {
            HStack(spacing: 8) {
                Image(systemName: "lock.fill").foregroundStyle(.secondary)
                Text("••••••••").font(.system(size: 14)).foregroundStyle(.primary)
                Spacer(minLength: 0)
                ForEach(["不讀取", "不學習"], id: \.self) { tag in
                    Text(tag)
                        .font(.caption)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2)
                        .background(Capsule().fill(.quaternary.opacity(0.7)))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.top, 18)
    }
}
