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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var width: CGFloat = 520

    var body: some View {
        let flows = PreviewFlows(preferences: preferences)
        let states = flows.states
        let shown = states.contains(director.state) ? director.state : .list
        let pinned = DemoPinning.previewState
        let state = pinned?.state ?? shown
        let frames = flows.frames(state)
        let frame = min(pinned?.frame ?? director.frame, frames.count - 1)
        VStack(alignment: .leading, spacing: 10) {
            let scale = SceneLayout(scene: frames[frame], width: width, preferences: preferences, maxHeight: Self.maxHeight).scale
            PreviewTabs(states: states, selected: state) { director.show($0, last: false) }
            GeometryReader { proxy in
                let layout = SceneLayout(scene: frames[frame], width: proxy.size.width, preferences: preferences,
                                         maxHeight: Self.maxHeight)
                PreviewBlock(layout: layout.fitted(height: stageHeight(flows, states, width: proxy.size.width)),
                             preferences: preferences, animated: false, showsTitle: false)
                    .id(state)
                    .transition(.opacity)
                    .onAppear { width = proxy.size.width }
                    .onChange(of: proxy.size.width) { _, now in width = now }
            }
            .frame(height: stageHeight(flows, states, width: width))
            .overlay(alignment: .bottomTrailing) {
                if scale < 0.99 {
                    Text("縮小為 \(Int((scale * 100).rounded()))%").font(.caption2).foregroundStyle(.tertiary)
                        .padding(8)
                }
            }
        }
        .onHover { director.hovered = $0 }
        .task(id: reduceMotion) {
            // Autoplay; none under Reduce Motion (the tabs and the switch on a change still work, as a crossfade).
            guard !reduceMotion, pinned == nil, !SettingsWindowController.isPreview else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(PreviewDirector.step))
                let current = PreviewFlows(preferences: preferences)
                director.advance(states: current.states, frames: { current.frames($0).count })
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("候選字視窗預覽：\(state.title)")
    }

    /// The scene's tallest: a larger window (big type, many candidates) is drawn smaller as a whole.
    static let maxHeight: CGFloat = 280

    /// One height for every state and moment, so the page never moves while the preview plays.
    private func stageHeight(_ flows: PreviewFlows, _ states: [PreviewState], width: CGFloat) -> CGFloat {
        states.flatMap(flows.frames)
            .map { SceneLayout(scene: $0, width: width, preferences: preferences, maxHeight: Self.maxHeight).naturalHeight }
            .max() ?? 120
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

    init(preferences: Preferences) {
        self.preferences = preferences
        scenes = PreviewScenes(preferences: preferences)
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

    init(_ p: Preferences) {
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
