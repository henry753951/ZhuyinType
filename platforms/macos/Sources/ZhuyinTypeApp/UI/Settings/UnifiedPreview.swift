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
    /// No moving to another state until then: the user just picked a tab or changed a setting.  Short, so the
    /// carousel goes on soon after.
    var heldUntil = Date.distantPast
    /// A state the preview stays on, playing its flow (a wizard lesson's demo); nil plays every state in turn.
    var pinnedState: PreviewState?
    /// Every candidate of a page (not the compact few) until then: 每頁的候選字 just changed.
    @Published var fullPageUntil = Date.distantPast
    var showsFullPage: Bool { Date() < fullPageUntil }
    /// No moving at all until then: the user is pressing or dragging in the preview.
    var pausedUntil = Date.distantPast
    /// A name for the self-tests, which read the states it went through.
    let name: String?

    init(name: String? = nil) { self.name = name }

    static var step: TimeInterval = 1.4
    static let hold: TimeInterval = 2
    /// The states each named preview has shown, for the self-tests.
    private(set) static var history: [String: [PreviewState]] = [:]

    /// Shows `state` (from its first moment, or its last, which shows the setting); the carousel goes on after a
    /// short hold.
    func show(_ state: PreviewState, last: Bool = true, animated: Bool = true) {
        heldUntil = Date().addingTimeInterval(Self.hold)
        let apply = { [self] in
            self.state = state
            frame = last ? Int.max : 0
        }
        if animated { withAnimation(.easeInOut(duration: 0.3)) { apply() } } else { apply() }
        record()
    }

    /// Keeps the preview on `state`, playing its flow over and over (a wizard lesson's demo).
    func pin(_ state: PreviewState) {
        pinnedState = state
        self.state = state
        frame = 0
    }

    /// The next moment of the flow; after its last, the next state (or this one again, when it is pinned or the user
    /// has just picked it).  Nothing pauses it but the user's own clicks: the pointer resting over it does not.
    func advance(states: [PreviewState], frames: (PreviewState) -> Int) {
        guard !states.isEmpty, Date() >= pausedUntil else { return }
        let count = frames(state)
        let current = min(frame, max(0, count - 1))
        if current + 1 < count {
            withAnimation(.easeOut(duration: 0.25)) { frame = current + 1 }
        } else if pinnedState != nil || Date() < heldUntil || !states.contains(where: { $0 != state }) {
            withAnimation(.easeOut(duration: 0.25)) { frame = 0 }
        } else {
            let index = states.firstIndex(of: state).map { ($0 + 1) % states.count } ?? 0
            withAnimation(.spring(response: 0.45, dampingFraction: 0.88)) {
                state = states[index]
                frame = 0
            }
            record()
        }
        ticks += 1
    }

    static func clearHistory(_ name: String) { history[name] = nil }

    /// The user pressed or dragged in the preview: it stays as it is, and goes on a moment after they let go.
    func interact() { pausedUntil = Date().addingTimeInterval(Self.hold) }

    private func record() {
        guard let name else { return }
        Self.history[name, default: []].append(state)
    }

    /// How many moments have played (the self-tests check that the preview moves).
    private(set) var ticks = 0
}

struct UnifiedPreview: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var director: PreviewDirector
    /// The state tabs above the scene (the wizard's lessons show one state, without them).
    var tabs = true
    /// The scene's tallest.  The window is never drawn smaller: a page of fewer candidates fits instead.
    var maxHeight: CGFloat = UnifiedPreview.defaultMaxHeight
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var width: CGFloat = 520

    var body: some View {
        let flows = fittingFlows(in: width)
        let states = flows.states
        let shown = states.contains(director.state) ? director.state : .list
        let pinned = DemoPinning.previewState
        let state = pinned?.state ?? shown
        let frames = flows.frames(state)
        let frame = min(pinned?.frame ?? director.frame, frames.count - 1)
        let sceneWidth = sceneWidth(flows, states, in: width)
        VStack(alignment: .leading, spacing: 10) {
            if tabs { PreviewTabs(states: states, selected: state) { director.show($0, last: false) } }
            GeometryReader { proxy in
                // The scene as wide as the window needs (a horizontal list takes the whole width); the state's
                // description beside it in the room left.
                let layout = SceneLayout(scene: frames[frame], width: sceneWidth, preferences: preferences)
                HStack(alignment: .top, spacing: 18) {
                    PreviewBlock(layout: layout.fitted(height: stageHeight(flows, [state], width: sceneWidth)),
                                 preferences: preferences, animated: !DemoPinning.still && !reduceMotion, showsTitle: false)
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
            // The box fits the state shown (a line and a badge, or the list), and grows or shrinks with it.
            .frame(height: stageHeight(flows, [state], width: sceneWidth))
            .animation(reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.45, dampingFraction: 0.85), value: state)
        }
        // Only a press or a drag pauses it (and it goes on about 2 s later); the pointer resting over it does not.
        .simultaneousGesture(DragGesture(minimumDistance: 0).onChanged { _ in director.interact() }
            .onEnded { _ in director.interact() })
        .task(id: reduceMotion) {
            // Autoplay; none under Reduce Motion (the tabs and the switch on a change still work, as a crossfade).
            guard !reduceMotion, pinned == nil, !SettingsWindowController.isPreview || DemoPinning.live else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(PreviewDirector.step))
                let current = fittingFlows(in: width)
                director.advance(states: current.states, frames: { current.frames($0).count })
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("候選字視窗預覽：\(state.title)")
    }

    /// The candidates the preview lists a page: fewer than the window does, so the preview stays short; all of them
    /// for a while after 每頁的候選字 changes.
    static let pageLimit = 4
    static let defaultMaxHeight: CGFloat = 300

    /// The flows with as many candidates a page as fit `available` and `maxHeight` at 100 % (the compact few, or the
    /// whole page just after 每頁的候選字 changed); at least two.
    func fittingFlows(in available: CGFloat) -> PreviewFlows {
        let most = director.showsFullPage ? preferences.pageSelectionKeys.count : Self.pageLimit
        var limit = max(2, most)
        while true {
            let flows = PreviewFlows(preferences: preferences, pageLimit: limit)
            if limit <= 2 || fits(flows, in: available) { return flows }
            limit -= 1
        }
    }

    private func fits(_ flows: PreviewFlows, in available: CGFloat) -> Bool {
        flows.states.flatMap(flows.frames).allSatisfy { scene in
            let layout = SceneLayout(scene: scene, width: available, preferences: preferences)
            return !layout.overflows && layout.naturalHeight <= maxHeight
        }
    }

    /// The width the scenes need: the widest window plus the line's inset, at most the room there is.
    private func sceneWidth(_ flows: PreviewFlows, _ states: [PreviewState], in available: CGFloat) -> CGFloat {
        guard preferences.candidateLayout == .vertical else { return available }
        let needed = states.flatMap(flows.frames).compactMap { scene -> CGFloat? in
            let layout = SceneLayout(scene: scene, width: available, preferences: preferences)
            return layout.panel.map { $0.maxX + 16 }
        }.max() ?? 300
        return min(available, max(300, needed))
    }

    /// The height of the state's tallest moment, so the box does not move while its flow plays.
    private func stageHeight(_ flows: PreviewFlows, _ states: [PreviewState], width: CGFloat) -> CGFloat {
        states.flatMap(flows.frames)
            .map { SceneLayout(scene: $0, width: width, preferences: preferences).naturalHeight }
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
            var back = mode
            if mode.badge != nil { back.badge = "中" }
            return [before, mode, back]
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
