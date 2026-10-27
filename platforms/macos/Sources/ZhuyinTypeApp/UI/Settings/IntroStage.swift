import AppKit
import SwiftUI
import ZhuyinEngine

/// The 介紹 page's story: a document where typing plays in a loop (Zhuyin typed, the sentence converts, ↓ opens the
/// list, the corrections list, a pick, a phrase added, the text sent, the 中/英 badge), drawn with the real candidate
/// window and badge views and the current settings and keys.  The key just pressed sits beside the caret.  Under Reduce
/// Motion it rests on the open list until played.
struct IntroStage: View {
    @ObservedObject var preferences: Preferences
    /// Show one step and stay there (UI previews).
    var pinned: Int?
    @State private var index = 0
    @State private var playing = true
    @State private var started = false
    @State private var toastVisible = false
    @State private var width: CGFloat = 520
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let lineFont = NSFont.systemFont(ofSize: 16)
    private static let margin: CGFloat = 20

    var body: some View {
        let script = IntroScript(preferences: preferences)
        let steps = script.steps
        let step = steps[min(pinned ?? index, steps.count - 1)]
        let current = min(pinned ?? index, steps.count - 1)
        VStack(spacing: 8) {
            // The stage takes the width offered (a GeometryReader never grows with its content) and is as tall as the
            // layout for that width needs.
            GeometryReader { proxy in
                stage(step: step, script: script, width: proxy.size.width, trigger: current)
                    .onAppear { width = proxy.size.width }
                    .onChange(of: proxy.size.width) { _, now in width = now }
            }
            .frame(height: script.layout(step: step, width: width).height)
                .overlay(alignment: .topTrailing) {
                    if pinned == nil { PlayPauseButton(playing: $playing).padding(8) }
                }
            Text(step.caption)
                .font(.callout)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity)
                .animation(nil, value: current)
        }
        .modifier(StoryClock(index: $index, holds: steps.map(\.hold), playing: playing && pinned == nil))
        .onAppear {
            guard !started else { return }
            started = true
            if reduceMotion {
                playing = false
                index = script.restingStep
            }
        }
        .onChange(of: playing) { _, now in
            if !now { index = script.restingStep }
        }
        .onChange(of: step.toast) { _, toast in showToast(toast != nil) }
        .onAppear { if pinned != nil { toastVisible = step.toast != nil } }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("候選字視窗預覽")
    }

    @ViewBuilder
    private func stage(step: IntroStep, script: IntroScript, width: CGFloat, trigger: Int) -> some View {
        let geometry = script.layout(step: step, width: width)
        ZStack(alignment: .topLeading) {
            DocumentBackdrop(lineTop: geometry.lineTop)
            DemoLine(segments: [DemoSegment(step.committed, .document)] + step.marked, fontSize: Self.lineFont.pointSize,
                     caretBlinks: pinned == nil)
                .offset(x: Self.margin, y: geometry.lineTop)
            if !step.keys.isEmpty {
                PressedKeycap(keys: step.keys, animated: pinned == nil, trigger: trigger)
                    .offset(x: geometry.keycapX, y: geometry.lineTop - 1)
            }
            if let panel = geometry.panel {
                let natural = geometry.naturalPanelSize
                CandidateWindowView(step: step, style: script.style, keys: script.keys, maxWidth: IntroScript.unbounded,
                                    appearance: preferences.panelAppearance)
                    .frame(width: natural.width, height: natural.height)
                    .scaleEffect(geometry.scale, anchor: .topLeading)
                    .frame(width: panel.width, height: panel.height, alignment: .topLeading)
                    .offset(x: panel.minX, y: panel.minY)
                    .transition(panelTransition(above: script.above))
            }
            if toastVisible || (pinned != nil && step.toast != nil) {
                ModeBadgeRepresentable(text: step.toast ?? "英", appearance: preferences.panelAppearance)
                    .frame(width: 52, height: 48)
                    .offset(x: geometry.toast.minX, y: geometry.toast.minY)
                    .transition(.opacity)
            }
        }
        .frame(width: width, height: geometry.height, alignment: .topLeading)
        .overlay(alignment: .topTrailing) {
            if geometry.scale < 0.99 {
                Text("縮小為 \(Int((geometry.scale * 100).rounded()))%")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .animation(panelAnimation, value: geometry.panel != nil)
    }

    private var panelAnimation: Animation? {
        preferences.panelAnimation ? .easeOut(duration: PanelMotion.appear) : nil
    }

    private func panelTransition(above: Bool) -> AnyTransition {
        // Like the real window: it fades (and slides, without Reduce Motion) in, and goes at once.
        guard preferences.panelAnimation else { return .identity }
        let insertion: AnyTransition = reduceMotion
            ? .opacity : .opacity.combined(with: .offset(y: above ? PanelMotion.slide : -PanelMotion.slide))
        return .asymmetric(insertion: insertion, removal: .identity)
    }

    private func showToast(_ show: Bool) {
        guard show else { toastVisible = false; return }
        toastVisible = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(ModeToast.duration * 1_000_000_000))
            withAnimation(preferences.panelAnimation ? .easeIn(duration: ModeToast.fade) : nil) { toastVisible = false }
        }
    }
}

/// A plain document behind the line, with a few grey lines of text so the window's material has something to show.
private struct DocumentBackdrop: View {
    let lineTop: CGFloat
    var body: some View {
        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 10).fill(Color(nsColor: .textBackgroundColor))
            VStack(alignment: .leading, spacing: 11) {
                ForEach([0.92, 0.74, 0.86, 0.55, 0.8, 0.66, 0.9, 0.5, 0.78, 0.6, 0.84, 0.7], id: \.self) { fraction in
                    GeometryReader { proxy in
                        Capsule().fill(.quaternary.opacity(0.55))
                            .frame(width: proxy.size.width * fraction, height: 7)
                    }
                    .frame(height: 7)
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 14)
            .mask(Rectangle().padding(.top, 0))
            .opacity(0.9)
            .allowsHitTesting(false)
            // Keep the typed line itself clear of the grey lines.
            Rectangle().fill(Color(nsColor: .textBackgroundColor))
                .frame(height: 30)
                .offset(y: lineTop - 5)
        }
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.quaternary, lineWidth: 1))
    }
}

// MARK: - Script

struct IntroStep: Equatable {
    var committed: String
    var marked: [DemoSegment] = []
    var list: CandidateList?
    var zhuyinHint: String?
    var notice: String?
    var toast: String?
    var keys: [String] = []
    var caption: String
    var hold: Double
}

/// The scenario and the stage's geometry for the current settings.
struct IntroScript {
    let style: CandidateStyle
    let keys: [Character]
    let above: Bool
    let showsToast: Bool
    /// Nil when the feature is turned off: its steps are left out.
    let selectLeft: [String]?
    let corrections: [String]?
    let pickKey: String
    let modeKey: String?
    let opensWithSpace: Bool

    init(preferences: Preferences) {
        style = preferences.candidateStyle
        keys = preferences.pageSelectionKeys
        above = preferences.candidatePlacement == .above
        showsToast = preferences.showModeToast
        let current = CurrentKeys(preferences)
        selectLeft = current.caps(.selectLeft)
        corrections = current.caps(.correctionsList)
        pickKey = keys.count > 1 ? String(keys[1]).uppercased() : "2"
        modeKey = current.modeKey
        opensWithSpace = preferences.spaceAction == .openCandidates
    }

    private static let start = "週末約了"
    private static let hint = "ㄒㄧㄠˇ ㄇㄧㄥˊ"
    private static let words = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"]

    private func list(selection: Int, tolerant: Bool) -> CandidateList {
        var items = Self.words.map { Candidate(text: $0) }
        if tolerant { items.insert(Candidate(text: "校名", annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ"), at: 0) }
        return CandidateList(items: items, selection: selection, pageSize: keys.count, tolerant: tolerant)
    }

    /// Where the scenario rests while paused: the list open.
    var restingStep: Int { 13 }

    var steps: [IntroStep] {
        let typing = "打注音：ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ"
        let w = DemoSegment("王")
        let s = Self.start
        var steps: [IntroStep] = [
            IntroStep(committed: s, caption: "在文件中打字", hold: 1.0),
            IntroStep(committed: s, marked: [DemoSegment("ㄨ")], keys: ["ㄨ"], caption: typing, hold: 0.18),
            IntroStep(committed: s, marked: [DemoSegment("ㄨㄤ")], keys: ["ㄤ"], caption: typing, hold: 0.18),
            IntroStep(committed: s, marked: [w], keys: ["ˊ"], caption: typing, hold: 0.3),
            IntroStep(committed: s, marked: [w, DemoSegment("ㄒ")], keys: ["ㄒ"], caption: typing, hold: 0.16),
            IntroStep(committed: s, marked: [w, DemoSegment("ㄒㄧ")], keys: ["ㄧ"], caption: typing, hold: 0.16),
            IntroStep(committed: s, marked: [w, DemoSegment("ㄒㄧㄠ")], keys: ["ㄠ"], caption: typing, hold: 0.18),
            IntroStep(committed: s, marked: [w, DemoSegment("小")], keys: ["ˇ"], caption: typing, hold: 0.3),
            IntroStep(committed: s, marked: [w, DemoSegment("小"), DemoSegment("ㄇ")], keys: ["ㄇ"], caption: typing, hold: 0.16),
            IntroStep(committed: s, marked: [w, DemoSegment("小"), DemoSegment("ㄇㄧ")], keys: ["ㄧ"], caption: typing, hold: 0.16),
            IntroStep(committed: s, marked: [w, DemoSegment("小"), DemoSegment("ㄇㄧㄥ")], keys: ["ㄥ"], caption: typing, hold: 0.18),
            IntroStep(committed: s, marked: [w, DemoSegment("曉明")], keys: ["ˊ"], caption: "整句隨打隨選", hold: 1.3),
            IntroStep(committed: s, marked: [w, DemoSegment("曉明", .focus)], list: list(selection: 0, tolerant: false),
                      zhuyinHint: Self.hint, keys: ["↓"],
                      caption: opensWithSpace ? "按 ↓ 或空白，開啟候選字視窗" : "按 ↓，開啟候選字視窗", hold: 1.4),
            IntroStep(committed: s, marked: [w, DemoSegment("曉明", .focus)], list: list(selection: 1, tolerant: false),
                      zhuyinHint: Self.hint, keys: [style.layout == .vertical ? "↓" : "→"], caption: "移動反白", hold: 1.0),
        ]
        if let corrections {
            steps += [
                IntroStep(committed: s, marked: [w, DemoSegment("曉明", .focus)], list: list(selection: 0, tolerant: true),
                          zhuyinHint: Self.hint, keys: corrections, caption: "列出按錯修正的字", hold: 1.8),
                IntroStep(committed: s, marked: [w, DemoSegment("曉明", .focus)], list: list(selection: 1, tolerant: false),
                          zhuyinHint: Self.hint, keys: corrections, caption: "再按一次，回到一般清單", hold: 1.0),
            ]
        }
        steps.append(IntroStep(committed: s, marked: [w, DemoSegment("小明")], keys: [pickKey], caption: "選字", hold: 1.2))
        if let selectLeft {
            steps += [
                IntroStep(committed: s, marked: [w, DemoSegment("小"), DemoSegment("明", .focus)], notice: "Enter 加入自訂詞：明",
                          keys: selectLeft, caption: "選取字詞", hold: 0.45),
                IntroStep(committed: s, marked: [w, DemoSegment("小明", .focus)], notice: "Enter 加入自訂詞：小明",
                          keys: selectLeft, caption: "選取字詞", hold: 0.45),
                IntroStep(committed: s, marked: [DemoSegment("王小明", .focus)], notice: "Enter 加入自訂詞：王小明",
                          keys: selectLeft, caption: "選取字詞", hold: 1.4),
                IntroStep(committed: s, marked: [DemoSegment("王小明")], notice: "已加入自訂詞：王小明",
                          keys: ["Return"], caption: "加入自訂詞，下次直接打得出", hold: 1.6),
            ]
        }
        steps.append(IntroStep(committed: s + "王小明", keys: ["Return"], caption: "送出", hold: 1.0))
        if showsToast, let modeKey {
            steps.append(IntroStep(committed: s + "王小明", toast: "英", keys: [modeKey],
                                   caption: "按 \(modeKey) 切換中英", hold: 1.6))
        }
        steps.append(IntroStep(committed: s + "王小明", caption: "", hold: 0.8))
        return steps
    }

    /// The panel is measured at its natural size; a window wider than the stage is drawn smaller, not squeezed.
    static let unbounded: CGFloat = 4000

    /// The panel's natural size for `step`, measured with the real view.
    func panelSize(_ step: IntroStep) -> NSSize? {
        guard step.list != nil || step.notice != nil else { return nil }
        return PanelMeasure.measure(list: step.list, keys: keys, style: style, notice: step.notice,
                                    zhuyinHint: step.zhuyinHint).size
    }

    private var lineHeight: CGFloat { ceil(IntroStage.lineFont.pointSize * 1.3) }

    struct Geometry {
        var height: CGFloat
        var lineTop: CGFloat
        var scale: CGFloat
        var naturalPanelSize: CGSize
        var panel: CGRect?
        var toast: CGRect
        var keycapX: CGFloat
    }

    /// Where the line, the window and the badge go: the window lined up with the composition, below (or above) the
    /// line, kept inside the stage as the real one is kept on screen.  The stage is tall enough for the largest
    /// window of the scenario; a window wider than the stage is drawn smaller.
    func layout(step: IntroStep, width: CGFloat) -> Geometry {
        func fit(_ size: NSSize) -> CGFloat { min(1, max(0.3, (width - 16) / max(1, size.width))) }
        let tallest = steps.compactMap(panelSize).map { $0.height * fit($0) }.max() ?? 0
        let height = 18 + lineHeight + InputPanelPlacement.gap + tallest + 22
        let lineTop = above ? height - 18 - lineHeight : 18
        let font = IntroStage.lineFont
        func measure(_ text: String) -> CGFloat {
            (text.replacingOccurrences(of: " ", with: "\u{2002}") as NSString).size(withAttributes: [.font: font]).width
        }
        let start = 20 + measure(step.committed)
        let markedWidth = step.marked.reduce(0) { $0 + measure($1.text) } + CGFloat(max(0, step.marked.count - 1)) * 1.5
        var panel: CGRect?
        var natural = CGSize.zero
        var scale: CGFloat = 1
        if let size = panelSize(step) {
            natural = size
            scale = fit(size)
            let scaled = CGSize(width: size.width * scale, height: size.height * scale)
            let inset = PanelMeasure.lastTextInset * scale
            let x = min(max(start - inset, 8), max(8, width - scaled.width - 8))
            let y = above ? lineTop - InputPanelPlacement.gap - scaled.height : lineTop + lineHeight + InputPanelPlacement.gap
            panel = CGRect(x: x, y: y, width: scaled.width, height: scaled.height)
        }
        let caretX = start + markedWidth
        let toastY = above ? lineTop - InputPanelPlacement.gap - 48 : lineTop + lineHeight + InputPanelPlacement.gap
        let toast = CGRect(x: min(caretX, width - 60), y: toastY, width: 52, height: 48)
        return Geometry(height: height, lineTop: lineTop, scale: scale, naturalPanelSize: natural, panel: panel, toast: toast,
                        keycapX: caretX + 9)
    }
}

/// The real candidate window content on its real surface.
private struct CandidateWindowView: NSViewRepresentable {
    let step: IntroStep
    let style: CandidateStyle
    let keys: [Character]
    let maxWidth: CGFloat
    let appearance: PanelAppearance

    func makeNSView(context: Context) -> InputSurface { InputSurface(content: CandidateContentView(frame: .zero)) }

    func updateNSView(_ surface: InputSurface, context: Context) {
        surface.appearance = appearance.appearance
        let content = surface.content as! CandidateContentView
        content.update(list: step.list, preview: nil, keys: keys, style: style, maxWidth: maxWidth, notice: step.notice,
                       zhuyinHint: step.zhuyinHint)
        surface.fitContent()
    }
}

private struct ModeBadgeRepresentable: NSViewRepresentable {
    let text: String
    let appearance: PanelAppearance

    func makeNSView(context: Context) -> InputSurface { InputSurface.modeBadge() }

    func updateNSView(_ surface: InputSurface, context: Context) {
        surface.appearance = appearance.appearance
        (surface.content as? ModeBadgeView)?.label.stringValue = text
    }
}
