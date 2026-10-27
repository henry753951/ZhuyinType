import SwiftUI
import ZhuyinEngine

// The 視窗與提示 page's preview: one scene at a time, drawn with the real candidate window (CandidateContentView, the
// same view, fonts and sizes the floating window uses), with tabs for its states.  Left alone it plays the states one
// after another, each as a short flow (the list opening, ⇧↑ switching to corrections, a page turning with the real
// paging motion…).  Changing a setting below switches it to the state that shows that setting.  It holds still while
// the pointer is over it, for a while after the user picks a tab or changes a setting, and under Reduce Motion.

/// What the preview can show.
enum PreviewState: String, CaseIterable, Identifiable {
    case list, corrections, composing, paging, phrase, mode
    var id: String { rawValue }
    var title: String {
        switch self {
        case .list: return "候選字清單"
        case .corrections: return "修正清單"
        case .composing: return "組字中"
        case .paging: return "翻頁"
        case .phrase: return "加入自訂詞"
        case .mode: return "中英切換"
        }
    }
}

/// Which state the preview shows, which moment of its flow, and whether it plays on its own.
final class PreviewDirector: ObservableObject {
    @Published var state: PreviewState = .list
    /// The moment of the state's flow; it starts at the last, which shows the window.
    @Published var frame = Int.max
    /// The pointer is over the preview.
    @Published var hovered = false
    /// No autoplay until then: the user picked a tab or changed a setting.
    var heldUntil = Date.distantPast
    /// Every candidate of a page (not the compact few) until then: 每頁的候選字 just changed.
    @Published var fullPageUntil = Date.distantPast
    var showsFullPage: Bool { Date() < fullPageUntil }

    static let step: TimeInterval = 1.6
    static let hold: TimeInterval = 8

    /// Shows `state` (from its first moment, or its last, which shows the setting), and holds it a while.
    func show(_ state: PreviewState, last: Bool = true, animated: Bool = true) {
        heldUntil = Date().addingTimeInterval(Self.hold)
        let apply = { [self] in
            self.state = state
            frame = last ? Int.max : 0
        }
        if animated { withAnimation(.easeInOut(duration: 0.3)) { apply() } } else { apply() }
    }

    /// The next moment of the flow, or the next state.
    func advance(states: [PreviewState], frames: (PreviewState) -> Int) {
        guard !hovered, Date() >= heldUntil, !states.isEmpty else { return }
        let count = frames(state)
        let current = min(frame, max(0, count - 1))
        if current + 1 < count {
            frame = current + 1
        } else {
            let index = states.firstIndex(of: state).map { ($0 + 1) % states.count } ?? 0
            withAnimation(.easeInOut(duration: 0.35)) {
                state = states[index]
                frame = 0
            }
        }
    }
}

struct UnifiedPreview: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var director: PreviewDirector
    /// The state tabs above the scene (the wizard's lessons show one state, without them).
    var tabs = true
    /// The scene's tallest; a larger window is drawn smaller as a whole.
    var maxHeight: CGFloat = UnifiedPreview.defaultMaxHeight
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var width: CGFloat = 520

    var body: some View {
        let flows = PreviewFlows(preferences: preferences, pageLimit: director.showsFullPage ? nil : Self.pageLimit)
        let states = flows.states
        let shown = states.contains(director.state) ? director.state : .list
        let pinned = DemoPinning.previewState
        let state = pinned?.state ?? shown
        let frames = flows.frames(state)
        let frame = min(pinned?.frame ?? director.frame, frames.count - 1)
        VStack(alignment: .leading, spacing: 10) {
            if tabs { PreviewTabs(states: states, selected: state) { director.show($0, last: false) } }
            GeometryReader { proxy in
                // The scene as wide as the window needs (a horizontal list takes the whole width); the state's
                // description beside it in the room left.
                let sceneWidth = sceneWidth(flows, states, in: proxy.size.width)
                let layout = SceneLayout(scene: frames[frame], width: sceneWidth, preferences: preferences, maxHeight: maxHeight)
                HStack(alignment: .top, spacing: 18) {
                    PreviewBlock(layout: layout.fitted(height: stageHeight(flows, states, width: sceneWidth)),
                                 preferences: preferences, animated: false, showsTitle: false)
                        .frame(width: sceneWidth)
                        .id(state)
                        .transition(.opacity)
                    if proxy.size.width - sceneWidth > 150 {
                        StateNote(state: state, preferences: preferences)
                            .id("note-" + state.rawValue)
                            .transition(.opacity)
                    }
                }
                .onAppear { width = proxy.size.width }
                .onChange(of: proxy.size.width) { _, now in width = now }
            }
            .frame(height: stageHeight(flows, states, width: sceneWidth(flows, states, in: width)))
        }
        .onHover { director.hovered = $0 }
        .task(id: reduceMotion) {
            // Autoplay; none under Reduce Motion (the tabs and the switch on a change still work, as a crossfade).
            guard !reduceMotion, pinned == nil, !SettingsWindowController.isPreview else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(PreviewDirector.step))
                let current = PreviewFlows(preferences: preferences, pageLimit: director.showsFullPage ? nil : Self.pageLimit)
                director.advance(states: current.states, frames: { current.frames($0).count })
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("候選字視窗預覽：\(state.title)")
    }

    /// The candidates the preview lists a page: fewer than the window does, so the preview stays short; all of them
    /// for a while after 每頁的候選字 changes.
    static let pageLimit = 4
    /// The scene's tallest: a larger window (big type) is drawn smaller as a whole.
    static let defaultMaxHeight: CGFloat = 300

    /// The width the scenes need: the widest window plus the line's inset, at most the room there is.
    private func sceneWidth(_ flows: PreviewFlows, _ states: [PreviewState], in available: CGFloat) -> CGFloat {
        guard preferences.candidateLayout == .vertical else { return available }
        let needed = states.flatMap(flows.frames).compactMap { scene -> CGFloat? in
            let layout = SceneLayout(scene: scene, width: available, preferences: preferences, maxHeight: maxHeight)
            return layout.panel.map { $0.maxX + 16 }
        }.max() ?? 300
        return min(available, max(300, needed))
    }

    /// One height for every state and moment, so the page never moves while the preview plays.
    private func stageHeight(_ flows: PreviewFlows, _ states: [PreviewState], width: CGFloat) -> CGFloat {
        states.flatMap(flows.frames)
            .map { SceneLayout(scene: $0, width: width, preferences: preferences, maxHeight: maxHeight).naturalHeight }
            .max() ?? 120
    }
}

/// What the state shows, beside the scene: its name and the keys that lead to it, from the current settings.
private struct StateNote: View {
    let state: PreviewState
    @ObservedObject var preferences: Preferences
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(state.title).font(.headline)
            Text(Self.text(state, preferences)).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, 10)
        .frame(maxWidth: 260, alignment: .leading)
    }

    static func text(_ state: PreviewState, _ p: Preferences) -> String {
        let keys = CurrentKeys(p)
        switch state {
        case .list:
            return "按 ↓\(p.spaceAction == .openCandidates ? "或空白" : "") 開啟；\(keys.selection) 選字，Return 選取反白的字。"
        case .corrections:
            return (keys.text(.correctionsList).map { "按 \($0) " } ?? "") + "列出按錯修正與模糊音的字" + (p.showsAnnotations ? "，修正的讀音以灰字標出。" : "。")
        case .composing: return "打完一個注音就轉成中文，整句一起轉換；要改的字可以之後再選。"
        case .paging:
            let next = (p.bindings[.nextPage] ?? []).prefix(3).map(\.compact).joined(separator: "、")
            return (next.isEmpty ? "清單中按空白翻頁" : "按 \(next) 翻到下一頁") + (p.panelAnimation ? "，舊的一頁移出、新的一頁移入。" : "。")
        case .phrase: return "選取 1 到 6 個字，再按 Return 加入自訂詞。"
        case .mode: return keys.modeKey.map { "按 \($0) 切換中英" + (p.showModeToast ? "，游標旁顯示「中」或「英」。" : "。") } ?? "沒有設定切換中英的按鍵。"
        }
    }
}

/// The states as small tabs above the scene.
private struct PreviewTabs: View {
    let states: [PreviewState]
    let selected: PreviewState
    let select: (PreviewState) -> Void
    var body: some View {
        TrailingFlow(spacing: 6, lineSpacing: 6, leading: true) {
            ForEach(states) { state in
                Button { select(state) } label: {
                    Text(state.title)
                        .font(.callout.weight(state == selected ? .semibold : .regular))
                        .foregroundStyle(state == selected ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.secondary))
                        .padding(.horizontal, 10)
                        .frame(height: 24)
                        .background(Capsule().fill(state == selected ? Color.accentColor.opacity(0.14) : Color.primary.opacity(0.05)))
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(state == selected ? .isSelected : [])
            }
        }
    }
}

/// Each state's flow, as the scenes of the real window it passes through, for the current settings.
struct PreviewFlows {
    let scenes: PreviewScenes
    private let preferences: Preferences

    init(preferences: Preferences, pageLimit: Int? = nil) {
        self.preferences = preferences
        scenes = PreviewScenes(preferences: preferences, pageLimit: pageLimit)
    }

    /// The states the settings allow (a list whose key is turned off has no state).
    var states: [PreviewState] {
        PreviewState.allCases.filter { !frames($0).isEmpty }
    }

    func frames(_ state: PreviewState) -> [PreviewScene] {
        switch state {
        case .list:
            var before = scenes.list
            before.list = nil
            before.zhuyinHint = nil
            return [before, scenes.list]
        case .corrections:
            guard let corrections = scenes.corrections else { return [] }
            return [scenes.list, corrections]
        case .composing:
            var start = scenes.composing
            start.marked = [DemoSegment("王"), DemoSegment("ㄒ")]
            start.keys = ["ㄒ"]
            return [start, scenes.composing]
        case .paging:
            guard let list = scenes.list.list, list.pageCount > 1, preferences.isEnabled(KeyFeature.pageKeys) else { return [] }
            var next = scenes.list
            next.list?.selection = list.pageSize
            next.keys = CurrentKeys(preferences).caps(.nextPage) ?? ["Tab"]
            next.note = nil
            return [scenes.list, next]
        case .phrase:
            guard let phrase = scenes.phrase else { return [] }
            var selecting = phrase
            selecting.notice = nil
            selecting.note = "×1"
            selecting.marked = [DemoSegment("王"), DemoSegment("小明", .focus)]
            return [selecting, phrase]
        case .mode:
            guard let mode = scenes.mode else { return [] }
            var before = mode
            before.badge = nil
            before.badgeOff = false
            before.keys = []
            return [before, mode]
        }
    }

    /// The state that shows a change between two sets of settings, or nil when nothing the preview shows changed.
    static func state(showing old: PreviewSettings, _ new: PreviewSettings) -> PreviewState? {
        if old.corrections != new.corrections { return .corrections }
        if old.paging != new.paging { return .paging }
        if old.mode != new.mode { return .mode }
        if old.list != new.list { return .list }
        return nil
    }
}

/// The settings each state shows, to tell which one a change belongs to.
struct PreviewSettings: Equatable {
    var list: [String]
    var corrections: [String]
    var paging: [String]
    var mode: [String]
    var perPage: Int

    init(_ p: Preferences) {
        perPage = p.candidatesPerPage
        let keys = CurrentKeys(p)
        list = [p.candidateLayout.rawValue, "\(p.candidateFontSize)", p.candidateDensity.rawValue, "\(p.candidatesPerPage)",
                p.selectionKeys.rawValue, "\(p.showsZhuyinHint)", "\(p.isEnabled(KeyFeature.zhuyinRowClick))", "\(p.candidatePlacement)",
                "\(p.panelAppearance)", "\(p.isEnabled(KeyFeature.singleCharacterList))", keys.text(.singleCharacterList) ?? ""]
        corrections = ["\(p.showsAnnotations)", "\(p.showsListSwitches)", "\(p.isEnabled(KeyFeature.correctionsList))",
                       keys.text(.correctionsList) ?? ""]
        paging = ["\(p.panelAnimation)", "\(p.pageAtListEdge)", "\(p.isEnabled(KeyFeature.pageKeys))", keys.text(.nextPage) ?? "",
                  keys.text(.previousPage) ?? ""]
        mode = ["\(p.showModeToast)", keys.modeKey ?? ""]
    }
}
