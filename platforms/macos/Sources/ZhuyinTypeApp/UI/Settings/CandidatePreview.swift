import AppKit
import SwiftUI
import ZhuyinEngine

/// The 候選字視窗 page's preview: still scenes side by side, each a line of a document with the key just pressed
/// beside it and the real candidate window (or 中/英 badge) under it.  Every setting on the page redraws every scene at
/// once, and the part a setting changes can be highlighted.
///
/// Several still scenes rather than one playing scene: each setting shows its effect at once, without waiting for
/// the moment of a scenario where it matters, and the corrections list, the phrase notice and the badge can be
/// compared with the list next to them.
struct CandidatePreview: View {
    @ObservedObject var preferences: Preferences
    /// False for the UI previews: keycaps drawn at rest.
    var animated = true
    @State private var width: CGFloat = 520
    static let spacing: CGFloat = 12

    var body: some View {
        // The rows take the width offered (a GeometryReader never grows with its content); the height is what the
        // rows need at that width.
        GeometryReader { proxy in
            VStack(alignment: .leading, spacing: Self.rowSpacing) {
                ForEach(Array(rows(width: proxy.size.width).enumerated()), id: \.offset) { _, row in
                    self.row(row.scenes, widths: row.widths)
                }
            }
            .frame(width: proxy.size.width, alignment: .leading)
            .onAppear { width = proxy.size.width }
            .onChange(of: proxy.size.width) { _, now in width = now }
        }
        .frame(height: height(width: width))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("候選字視窗預覽")
    }

    static let rowSpacing: CGFloat = 14
    /// The scene title's line above each card.
    static let titleHeight: CGFloat = 22

    /// The scenes row by row, with each card's width, for `width`.
    private func rows(width: CGFloat) -> [(scenes: [PreviewScene], widths: [CGFloat])] {
        let scenes = PreviewScenes(preferences: preferences)
        let vertical = preferences.candidateLayout == .vertical
        let half = (width - Self.spacing) / 2
        // The phrase notice is the widest of the small scenes.
        let narrow = (width - Self.spacing * 2) * 0.29
        let wide = width - Self.spacing * 2 - narrow * 2
        var rows: [(scenes: [PreviewScene], widths: [CGFloat])] = []
        if vertical, let corrections = scenes.corrections {
            rows.append(([scenes.list, corrections], [half, half]))
        } else {
            rows.append(([scenes.list], [vertical ? half : width]))
            if let corrections = scenes.corrections { rows.append(([corrections], [width])) }
        }
        let small = [scenes.composing] + [scenes.phrase, scenes.mode].compactMap { $0 }
        rows.append((small, small.count == 3 ? [narrow, wide, narrow] : Array(repeating: (width - Self.spacing) / 2, count: small.count)))
        return rows
    }

    private func height(width: CGFloat) -> CGFloat {
        let rows = rows(width: width)
        let heights = rows.map { row in
            zip(row.scenes, row.widths).map { SceneLayout(scene: $0, width: $1, preferences: preferences).naturalHeight }.max() ?? 0
        }
        return heights.reduce(0) { $0 + Self.titleHeight + $1 } + CGFloat(max(0, rows.count - 1)) * Self.rowSpacing
    }

    private func row(_ scenes: [PreviewScene], widths: [CGFloat]) -> some View {
        let layouts = zip(scenes, widths).map { SceneLayout(scene: $0, width: $1, preferences: preferences) }
        let height = layouts.map(\.naturalHeight).max() ?? 0
        return HStack(alignment: .top, spacing: Self.spacing) {
            ForEach(Array(layouts.enumerated()), id: \.offset) { _, layout in
                PreviewBlock(layout: layout.fitted(height: height), preferences: preferences, animated: animated)
                    .frame(width: layout.width)
            }
        }
    }
}

// MARK: - Scenes

/// One still moment: the text on the line, the keys that led to it, and the window or badge it shows.
struct PreviewScene {
    var title: String
    var committed: String
    var marked: [DemoSegment] = []
    var keys: [String]
    var note: String?
    var list: CandidateList?
    var zhuyinHint: String?
    var notice: String?
    var badge: String?
    /// Shown instead of the badge when it is turned off.
    var badgeOff = false
}

/// The five scenes for the current settings.  The keys come from the settings, never from here.
struct PreviewScenes {
    /// Nil for a scene whose feature is turned off on the 快捷鍵 page.
    let composing, list: PreviewScene
    let corrections, phrase, mode: PreviewScene?

    private static let start = "約了"
    private static let hint = "ㄒㄧㄠˇ ㄇㄧㄥˊ"
    private static let words = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"]

    init(preferences: Preferences) {
        let keys = CurrentKeys(preferences)
        let pageSize = preferences.pageSelectionKeys.count
        func candidates(tolerant: Bool) -> CandidateList {
            var items = Self.words.map { Candidate(text: $0) }
            if tolerant {
                items.insert(contentsOf: [Candidate(text: "校名", annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ"),
                                          Candidate(text: "孝明", annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ")], at: 0)
            }
            return CandidateList(items: items, selection: 0, pageSize: pageSize, tolerant: tolerant)
        }
        let w = DemoSegment("王")
        composing = PreviewScene(title: "組字中", committed: "",
                                 marked: [w, DemoSegment("小"), DemoSegment("ㄇㄧㄥ")], keys: ["ㄥ"])
        list = PreviewScene(title: "候選字清單", committed: Self.start, marked: [w, DemoSegment("曉明", .focus)],
                            keys: ["↓"], note: preferences.spaceAction == .openCandidates ? "或 空白" : nil,
                            list: candidates(tolerant: false), zhuyinHint: Self.hint)
        corrections = keys.caps(.correctionsList).map {
            PreviewScene(title: "修正清單", committed: Self.start, marked: [w, DemoSegment("曉明", .focus)],
                         keys: $0, list: candidates(tolerant: true), zhuyinHint: Self.hint)
        }
        phrase = keys.caps(.selectLeft).map {
            PreviewScene(title: "加入自訂詞", committed: Self.start, marked: [DemoSegment("王小明", .focus)],
                         keys: $0, note: "×3", notice: "Enter 加入自訂詞：王小明")
        }
        mode = keys.modeKey.map {
            PreviewScene(title: "中英切換", committed: "王小明", keys: [$0],
                         badge: preferences.showModeToast ? "英" : nil, badgeOff: !preferences.showModeToast)
        }
    }
}

// MARK: - Layout of one scene

/// Where the line, the keycap and the window go in a scene's card, at the card's width: the window lined up with
/// the composition below (or above) the line, as the real one is; a window wider than the card is drawn smaller.
struct SceneLayout {
    private(set) var scene: PreviewScene
    private(set) var width: CGFloat
    private(set) var above: Bool
    var height: CGFloat = 0
    var lineTop: CGFloat = 0
    /// The window at its natural size and the scale it is drawn at.
    var panelSize: CGSize?
    var scale: CGFloat = 1
    var panel: CGRect?
    var badge: CGRect?
    var keycapX: CGFloat = 0

    static let font = NSFont.systemFont(ofSize: 15)
    static let lineHeight: CGFloat = 20
    static let inset: CGFloat = 12
    private static let badgeSize = ModeBadgeView.size

    var naturalHeight: CGFloat {
        let content = panel.map(\.height) ?? badge.map(\.height) ?? (scene.badgeOff ? 16 : 0)
        return Self.inset * 2 + Self.lineHeight + (content > 0 ? InputPanelPlacement.gap + content : 0)
    }

    init(scene: PreviewScene, width: CGFloat, preferences: Preferences) {
        self.scene = scene
        self.width = width
        above = preferences.candidatePlacement == .above
        func measure(_ text: String) -> CGFloat {
            (text as NSString).size(withAttributes: [.font: Self.font]).width
        }
        let start = Self.inset + measure(scene.committed)
        let caret = start + scene.marked.reduce(0) { $0 + measure($1.text) } + CGFloat(max(0, scene.marked.count - 1)) * 1.5
        keycapX = caret + 7
        if scene.list != nil || scene.notice != nil {
            let measured = PanelMeasure.measure(list: scene.list, keys: preferences.pageSelectionKeys,
                                                style: preferences.candidateStyle, notice: scene.notice,
                                                zhuyinHint: scene.zhuyinHint)
            panelSize = measured.size
            scale = min(1, max(0.3, (width - 12) / max(1, measured.size.width)))
            let size = CGSize(width: measured.size.width * scale, height: measured.size.height * scale)
            let x = min(max(start - measured.textInset * scale, 6), max(6, width - size.width - 6))
            panel = CGRect(origin: CGPoint(x: x, y: 0), size: size)
        }
        if scene.badge != nil {
            badge = CGRect(origin: CGPoint(x: min(max(caret, 6), max(6, width - Self.badgeSize.width - 6)), y: 0),
                           size: Self.badgeSize)
        }
        self = fitted(height: naturalHeight)
    }

    /// The same scene in a card `height` tall (the tallest of its row): the line at the top, or at the bottom when
    /// the window opens above it.
    func fitted(height: CGFloat) -> SceneLayout {
        var copy = self
        copy.height = height
        copy.lineTop = above ? height - Self.inset - Self.lineHeight : Self.inset
        let below = copy.lineTop + Self.lineHeight + InputPanelPlacement.gap
        if let panel {
            copy.panel?.origin.y = above ? copy.lineTop - InputPanelPlacement.gap - panel.height : below
        }
        if let badge {
            copy.badge?.origin.y = above ? copy.lineTop - InputPanelPlacement.gap - badge.height : below
        }
        return copy
    }
}

/// Measures the candidate window offscreen with the view the window itself uses.
enum PanelMeasure {
    private static let content = CandidateContentView(frame: .zero)

    static func measure(list: CandidateList?, keys: [Character], style: CandidateStyle, notice: String?,
                        zhuyinHint: String?) -> (size: CGSize, textInset: CGFloat) {
        let size = content.update(list: list, preview: nil, keys: keys, style: style, maxWidth: 4000, notice: notice,
                                  zhuyinHint: zhuyinHint)
        return (size, content.candidateTextInset)
    }

    /// The text inset of the window measured last.
    static var lastTextInset: CGFloat { content.candidateTextInset
    }
}

// MARK: - One block

private struct PreviewBlock: View {
    let layout: SceneLayout
    @ObservedObject var preferences: Preferences
    let animated: Bool

    var body: some View {
        let scene = layout.scene
        VStack(alignment: .leading, spacing: 6) {
            // The title, and on the right how much a window too large for the block is shrunk (never over the scene).
            HStack(alignment: .lastTextBaseline, spacing: 6) {
                Text(scene.title)
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if layout.scale < 0.99 {
                    Text("縮小為 \(Int((layout.scale * 100).rounded()))%")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }
            .frame(width: layout.width, height: CandidatePreview.titleHeight - 6, alignment: .bottomLeading)
            ZStack(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color(nsColor: .textBackgroundColor))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(.quaternary, lineWidth: 1))
                DemoLine(segments: [DemoSegment(scene.committed, .document)] + scene.marked,
                         fontSize: SceneLayout.font.pointSize, caretBlinks: false)
                    .offset(x: SceneLayout.inset, y: layout.lineTop)
                PressedKeycap(keys: scene.keys, note: scene.note, animated: animated)
                    .offset(x: layout.keycapX, y: layout.lineTop - 1)
                if let panel = layout.panel, let natural = layout.panelSize {
                    CandidateWindowView(scene: scene, style: preferences.candidateStyle,
                                        keys: preferences.pageSelectionKeys, appearance: preferences.panelAppearance)
                        .frame(width: natural.width, height: natural.height)
                        .scaleEffect(layout.scale, anchor: .topLeading)
                        .frame(width: panel.width, height: panel.height, alignment: .topLeading)
                        .offset(x: panel.minX, y: panel.minY)
                }
                if let badge = layout.badge, let text = scene.badge {
                    ModeBadgeRepresentable(text: text, appearance: preferences.panelAppearance)
                        .frame(width: badge.width, height: badge.height)
                        .offset(x: badge.minX, y: badge.minY)
                }
                if scene.badgeOff {
                    Text("不顯示提示")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .offset(x: SceneLayout.inset,
                                y: layout.above ? layout.lineTop - 20 : layout.lineTop + SceneLayout.lineHeight + 4)
                }
            }
            .frame(width: layout.width, height: layout.height, alignment: .topLeading)
            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        }
    }
}

/// The real candidate window content on its real surface.
private struct CandidateWindowView: NSViewRepresentable {
    let scene: PreviewScene
    let style: CandidateStyle
    let keys: [Character]
    let appearance: PanelAppearance

    /// An illustration: its rows never show the pointer.
    func makeNSView(context: Context) -> InputSurface {
        let content = CandidateContentView(frame: .zero)
        content.hoverEnabled = false
        return InputSurface(content: content)
    }

    func updateNSView(_ surface: InputSurface, context: Context) {
        surface.appearance = appearance.appearance
        let content = surface.content as! CandidateContentView
        content.update(list: scene.list, preview: nil, keys: keys, style: style, maxWidth: 4000, notice: scene.notice,
                       zhuyinHint: scene.zhuyinHint)
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

/// One scene of the preview (the candidate list) on its own, for the floating preview after a change.
struct CandidateScenePreview: View {
    @ObservedObject var preferences: Preferences
    var width: CGFloat = 300
    var body: some View {
        let layout = SceneLayout(scene: PreviewScenes(preferences: preferences).list, width: width, preferences: preferences)
        PreviewBlock(layout: layout.fitted(height: layout.naturalHeight), preferences: preferences, animated: false)
            .frame(width: width, height: CandidatePreview.titleHeight + layout.naturalHeight)
    }
}

/// The real candidate window for `list`, drawn still at its own size (the wizard storyboard).
struct PanelSnapshot: View {
    let list: CandidateList
    var zhuyinHint: String?
    @ObservedObject var preferences: Preferences
    var body: some View {
        let style = preferences.candidateStyle
        let keys = preferences.pageSelectionKeys
        let size = PanelMeasure.measure(list: list, keys: keys, style: style, notice: nil, zhuyinHint: zhuyinHint).size
        CandidateWindowView(scene: PreviewScene(title: "", committed: "", keys: [], list: list, zhuyinHint: zhuyinHint),
                            style: style, keys: keys, appearance: preferences.panelAppearance)
            .frame(width: size.width, height: size.height)
    }
}
