import SwiftUI
import ZhuyinEngine

// The setup wizard's window content, quiet as Apple's own setup screens: the window background with a faint wash of
// the accent colour at the top, faint grouped surfaces, the accent colour only for the main button, the selection and
// the progress, and type and spacing doing the rest.  A header (上一步, progress, 全部跳過); the step; its buttons.
//
// One idea a page: a title, one short sentence, the choice, one live demo.  Nothing scrolls: every page fits the
// window at its smallest (`WizardLayout.minSize`), which `WizardFit` measures and the UI previews and self-tests check.
//
// Steps move in from the side they come from and their parts arrive one after another (10 pt, 40 ms apart, soft
// springs); the welcome arrives slower (the icon, then the title); a finished practice turns green with a check; the
// last step draws its check.  Under Reduce Motion everything crossfades.

/// The window's sizes: the wizard is laid out to fit the smallest without scrolling.
enum WizardLayout {
    static let minSize = NSSize(width: 640, height: 640)
    static let defaultSize = NSSize(width: 680, height: 700)
    static let maxSize = NSSize(width: 680, height: 720)
    static let margin: CGFloat = 32
    static let headerHeight: CGFloat = 52
}

/// How tall each page's content is against the room it has, for the checks that nothing is cut off.
enum WizardFit {
    struct Record { let content: CGFloat; let room: CGFloat; var overflow: CGFloat { content - room } }
    private(set) static var latest: [String: Record] = [:]
    static func record(_ name: String, content: CGFloat, room: CGFloat) { latest[name] = Record(content: content, room: room) }
    static func reset() { latest = [:] }
    /// The pages whose content is taller than their room.
    static var overflows: [String] {
        latest.filter { $0.value.overflow > 0.5 }.sorted { $0.key < $1.key }
            .map { "\($0.key): \(Int($0.value.content.rounded())) pt in \(Int($0.value.room.rounded())) pt" }
    }
}

struct WizardView: View {
    @ObservedObject var model: WizardModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            header
            ZStack {
                if let pinned = DemoPinning.wizardTransition {
                    // The UI previews: the step transition drawn still at `progress`.
                    let p = pinned.progress
                    stepView(pinned.from).modifier(WizardSlide(x: -14 * p, opacity: max(0, 1 - p * 1.6)))
                    stepView(model.step).modifier(WizardSlide(x: 28 * (1 - p), opacity: p))
                } else {
                    stepView(model.step)
                        .id(model.step.id)
                        .transition(transition)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .clipped()
        }
        .frame(minWidth: WizardLayout.minSize.width, minHeight: WizardLayout.minSize.height)
        .background(WizardBackdrop())
    }

    private var transition: AnyTransition {
        if reduceMotion { return .opacity }
        let forward = model.direction > 0
        return .asymmetric(insertion: .modifier(active: WizardSlide(x: forward ? 16 : -16, opacity: 0), identity: WizardSlide(x: 0, opacity: 1)),
                           removal: .modifier(active: WizardSlide(x: forward ? -8 : 8, opacity: 0), identity: WizardSlide(x: 0, opacity: 1)))
    }

    private var header: some View {
        HStack(spacing: 12) {
            if model.step != .welcome, model.index > 0, model.step != .keys {
                Button { model.back() } label: { Label("上一步", systemImage: "chevron.left") }
                    .buttonStyle(.borderless)
                    .keyboardShortcut("[", modifiers: .command)
            }
            Spacer()
            if model.step != .welcome, model.progress.count > 1 { WizardProgressBar(model: model) }
            Spacer()
            if model.step != .finish, model.step != .keys {
                Button("全部跳過") { model.skipAll() }.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        }
        .padding(.leading, 84)
        .padding(.trailing, 24)
        .frame(height: WizardLayout.headerHeight)
    }

    @ViewBuilder private func stepView(_ step: WizardStep) -> some View {
        switch step {
        case .welcome: WelcomeStep(model: model)
        case .preset: PresetStep(model: model)
        case .lesson(let id): if let lesson = Lesson.all.first(where: { $0.id == id }) { LessonStep(model: model, lesson: lesson) }
        case .finish: FinishStep(model: model)
        case .keys: KeySummaryStep(model: model)
        }
    }
}

private struct WizardSlide: ViewModifier {
    let x: CGFloat
    let opacity: Double
    func body(content: Content) -> some View { content.offset(x: x).opacity(opacity) }
}

/// Where the user is: a segment a step, filled up to this one, and the step's name with 「2 / 7」 under it.  A lesson
/// whose practice is done shows its segment complete.
private struct WizardProgressBar: View {
    @ObservedObject var model: WizardModel
    var body: some View {
        let progress = model.progress
        let counted = model.steps.filter { $0 != .welcome }
        VStack(spacing: 5) {
            HStack(spacing: 4) {
                ForEach(Array(counted.enumerated()), id: \.offset) { offset, step in
                    let number = offset + 1
                    Capsule()
                        .fill(number <= progress.index ? AnyShapeStyle(Color.accentColor.opacity(number == progress.index ? 1 : 0.55))
                                                       : AnyShapeStyle(Color.primary.opacity(0.12)))
                        .frame(width: (number == progress.index ? 1.8 : 1) * segment(counted.count), height: 4)
                }
            }
            Text("\(name(model.step)) · \(progress.index) / \(progress.count)")
                .font(.caption)
                .foregroundStyle(.secondary)
                .contentTransition(.numericText())
        }
        .animation(WizardMotion.success, value: progress.index)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(name(model.step))，第 \(progress.index) 步，共 \(progress.count) 步")
    }

    /// Shorter segments for a longer tour, so the bar stays about 220 pt.
    private func segment(_ count: Int) -> CGFloat { min(14, max(7, 220 / CGFloat(max(1, count)) - 4)) }

    private func name(_ step: WizardStep) -> String {
        switch step {
        case .welcome: return "歡迎"
        case .preset: return "習慣"
        case .lesson(let id): return Lesson.all.first { $0.id == id }?.title ?? ""
        case .finish: return "確認"
        case .keys: return "按鍵"
        }
    }
}

// MARK: - Backdrop, surfaces and entrances

/// The window's backdrop: the window background with a very faint wash of the accent colour from the top (about 5 %),
/// and in Dark Mode a soft highlight along the top edge.  Nothing saturated, no glow.
struct WizardBackdrop: View {
    @Environment(\.colorScheme) private var colorScheme
    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)
            LinearGradient(colors: [Color.accentColor.opacity(colorScheme == .dark ? 0.06 : 0.05), Color.accentColor.opacity(0)],
                           startPoint: .top, endPoint: UnitPoint(x: 0.5, y: 0.55))
            if colorScheme == .dark {
                LinearGradient(colors: [Color.white.opacity(0.03), .clear], startPoint: .top, endPoint: UnitPoint(x: 0.5, y: 0.2))
            }
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }
}

/// A quiet surface, as the grouped boxes of System Settings: a faint fill and a hairline.  `tint` (the selection)
/// washes it with the accent colour.
struct Glass: View {
    var cornerRadius: CGFloat = 12
    var tint: Color?
    var body: some View {
        let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
        shape.fill(tint.map { $0.opacity(0.07) } ?? Color.primary.opacity(0.04))
            .overlay(shape.strokeBorder(Color.primary.opacity(0.07), lineWidth: 0.5))
    }
}

/// How many step entrances have played (the self-tests check that they do).
enum WizardEntrance {
    static var played = 0
}

/// A part of a step arriving: it rises a little (or grows, from `scale`) and fades in, `delay` after the step.
private struct Entrance: ViewModifier {
    let delay: Double
    let appeared: Bool
    var rise: CGFloat = 10
    var scale: CGFloat = 1
    var response: Double = 0.42
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    func body(content: Content) -> some View {
        content
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared || reduceMotion ? 0 : rise)
            .scaleEffect(appeared || reduceMotion ? 1 : scale)
            .animation(reduceMotion ? .easeOut(duration: 0.2).delay(delay * 0.5)
                       : .spring(response: response, dampingFraction: 0.9).delay(delay), value: appeared)
    }
}

extension View {
    /// The `order`th part of a step: 40 ms after the one before.
    fileprivate func entrance(_ order: Int, _ appeared: Bool) -> some View {
        modifier(Entrance(delay: 0.05 + Double(order) * 0.04, appeared: appeared))
    }
    /// A part arriving at a moment of its own (the welcome's slower ceremony).
    fileprivate func arrival(at delay: Double, _ appeared: Bool, rise: CGFloat = 8, scale: CGFloat = 1, response: Double = 0.5) -> some View {
        modifier(Entrance(delay: delay, appeared: appeared, rise: rise, scale: scale, response: response))
    }
}

/// A step's parts arrive when it appears.
private struct Arrival: ViewModifier {
    @Binding var appeared: Bool
    func body(content: Content) -> some View {
        content.onAppear {
            guard !appeared else { return }
            WizardEntrance.played += 1
            DispatchQueue.main.async { appeared = true }
        }
    }
}

// MARK: - Pieces

private struct StepTitle: View {
    let title: String
    let detail: String
    var size: CGFloat = 26
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.system(size: size, weight: .semibold)).fixedSize(horizontal: false, vertical: true)
            Text(detail).font(.title3).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct StepFooter<Leading: View, Trailing: View>: View {
    @ViewBuilder let leading: () -> Leading
    @ViewBuilder let trailing: () -> Trailing
    var body: some View {
        HStack(spacing: 10) {
            leading()
            Spacer()
            trailing()
        }
        .controlSize(.large)
        .padding(.horizontal, 28)
        .frame(height: StepFooterMetrics.height)
    }
}

private enum StepFooterMetrics { static let height: CGFloat = 64 }

/// The step's main action: prominent, in the accent colour.
private struct PrimaryButton: View {
    let title: String
    var enabled = true
    let action: () -> Void
    var body: some View {
        Button(action: action) { Text(title).frame(minWidth: 84) }
            .buttonStyle(.borderedProminent)
            .tint(.accentColor)
            .keyboardShortcut(.defaultAction)
            .disabled(!enabled)
    }
}

/// A secondary action beside the primary one.
private struct SecondaryButton: View {
    let title: String
    let action: () -> Void
    var body: some View { Button(title, action: action).buttonStyle(.bordered) }
}

/// The height a page's content has (the window less the header, the footer and the top inset).
private struct StepRoomKey: EnvironmentKey { static let defaultValue: CGFloat = 520 }
extension EnvironmentValues {
    fileprivate var stepRoom: CGFloat {
        get { self[StepRoomKey.self] }
        set { self[StepRoomKey.self] = newValue }
    }
}

/// Reads the page's room inside the page (a step's own environment is outside its `StepPage`).
private struct RoomReader<Content: View>: View {
    @Environment(\.stepRoom) private var room
    @ViewBuilder let content: (CGFloat) -> Content
    var body: some View { content(room) }
}

private struct StepContentHeight: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

/// A step's page: its content at its own height from the top, never scrolled, and its footer.  The content's height
/// against the room is recorded in `WizardFit`.
private struct StepPage<Content: View, Footer: View>: View {
    let name: String
    @ViewBuilder let content: () -> Content
    @ViewBuilder let footer: () -> Footer
    static var topInset: CGFloat { 6 }
    var body: some View {
        VStack(spacing: 0) {
            GeometryReader { proxy in
                let room = proxy.size.height - Self.topInset
                content()
                    .padding(.horizontal, WizardLayout.margin)
                    .frame(width: proxy.size.width, alignment: .topLeading)
                    .fixedSize(horizontal: false, vertical: true)
                    .background(GeometryReader { measured in
                        Color.clear.preference(key: StepContentHeight.self, value: measured.size.height)
                    })
                    .onPreferenceChange(StepContentHeight.self) { WizardFit.record(name, content: $0, room: room) }
                    .environment(\.stepRoom, room)
                    .padding(.top, Self.topInset)
                    .frame(width: proxy.size.width, height: proxy.size.height, alignment: .top)
            }
            footer()
        }
    }
}

private struct ChoiceCard: View {
    let title: String
    let lines: [(keys: [String], text: String)]
    var symbol: String?
    let selected: Bool
    let namespace: Namespace.ID
    let group: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    if let symbol { Image(systemName: symbol).font(.title3).foregroundStyle(.secondary) }
                    Text(title).font(.headline)
                    Spacer()
                    Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .foregroundStyle(selected ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
                }
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    HStack(spacing: 6) {
                        ForEach(line.keys, id: \.self) { KeyChip(text: $0) }
                        Text(line.text).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            }
            .padding(14)
            // As tall as the others of its row.
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background {
                ZStack {
                    Glass(cornerRadius: 12)
                    if selected {
                        // The selection moves between the cards.
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(Color.accentColor.opacity(0.06))
                            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 1.5))
                            .matchedGeometryEffect(id: "selection-" + group, in: namespace)
                    }
                }
            }
            .contentShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

// MARK: - Steps

private struct WelcomeStep: View {
    @ObservedObject var model: WizardModel
    @Namespace private var namespace
    @State private var appeared = DemoPinning.still

    /// Everything on the page but the preview, for the preview's height.
    private static let fixed: CGFloat = 64 + 14 + 34 + 6 + 24 + 22 + 28 + 18 + 76 + 4

    var body: some View {
        StepPage(name: "welcome") { RoomReader { room in
            VStack(spacing: 0) {
                // The ceremony: the icon, then the title, then the rest.
                Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 64, height: 64)
                    .arrival(at: 0.08, appeared, rise: 0, scale: 0.86, response: 0.6)
                Text("歡迎使用 ZhuyinType").font(.system(size: 28, weight: .semibold))
                    .padding(.top, 14)
                    .arrival(at: 0.42, appeared)
                Text("選好你的打字習慣，再實際試打。")
                    .font(.title3).foregroundStyle(.secondary)
                    .padding(.top, 6)
                    .arrival(at: 0.56, appeared)
                WelcomePreview(preferences: model.sandbox, maxHeight: max(120, min(240, room - Self.fixed)))
                    .padding(14)
                    .background(Glass())
                    .padding(.top, 22)
                    .arrival(at: 0.72, appeared)
                HStack(spacing: 12) {
                    ChoiceCard(title: "快速設定", lines: [([], "\(Lesson.all.filter { $0.level == .basic }.count) 個步驟，約 2 分鐘")],
                               symbol: "bolt", selected: model.mode == .quick, namespace: namespace, group: "mode") {
                        withAnimation(WizardMotion.success) { model.choose(.quick) }
                    }
                    ChoiceCard(title: "完整導覽", lines: [([], "\(Lesson.all.count) 個步驟，每個功能")], symbol: "map",
                               selected: model.mode == .full, namespace: namespace, group: "mode") {
                        withAnimation(WizardMotion.success) { model.choose(.full) }
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 18)
                .arrival(at: 0.84, appeared)
            }
            // Centred in the window.
            .frame(maxWidth: .infinity, minHeight: room, alignment: .center)
        } } footer: {
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "開始") { model.next() }
            }
            .arrival(at: 0.84, appeared, rise: 0)
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

private struct WelcomePreview: View {
    @ObservedObject var preferences: Preferences
    var maxHeight: CGFloat
    @StateObject private var director = PreviewDirector(name: "welcome")
    var body: some View {
        UnifiedPreview(preferences: preferences, director: director, tabs: false, maxHeight: maxHeight, steadyHeight: true)
    }
}

private struct PresetStep: View {
    @ObservedObject var model: WizardModel
    @Namespace private var namespace
    @State private var appeared = DemoPinning.still
    @State private var showsChanges = false

    var body: some View {
        StepPage(name: "preset") {
            VStack(alignment: .leading, spacing: 20) {
                StepTitle(title: "你原本習慣哪一種注音輸入？", detail: "選最接近的一種，之後每一步都能再改。")
                    .entrance(0, appeared)
                let presets = SettingsPreset.all
                Grid(horizontalSpacing: 14, verticalSpacing: 14) {
                    ForEach(Array(stride(from: 0, to: presets.count, by: 2)), id: \.self) { row in
                        GridRow {
                            ForEach(row..<min(row + 2, presets.count), id: \.self) { index in
                                let preset = presets[index]
                                ChoiceCard(title: preset.title, lines: Array(preset.lines.prefix(3)), selected: model.preset == preset.id,
                                           namespace: namespace, group: "preset") {
                                    withAnimation(WizardMotion.success) { model.choosePreset(preset) }
                                }
                                .entrance(1 + index, appeared)
                            }
                        }
                    }
                }
                // What it changes: a line, the list behind it.
                let changes = model.changes
                HStack(spacing: 8) {
                    Text(changes.isEmpty ? "和目前的設定相同" : "會改變 \(changes.count) 項設定")
                        .foregroundStyle(.secondary)
                        .contentTransition(.numericText())
                    if !changes.isEmpty {
                        Button("查看") { showsChanges.toggle() }
                            .buttonStyle(.link)
                            .popover(isPresented: $showsChanges, arrowEdge: .bottom) { PresetChanges(changes: changes) }
                    }
                }
                .animation(WizardMotion.success, value: changes.count)
                .entrance(6, appeared)
            }
        } footer: {
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "下一步") { model.next() }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

/// What the chosen preset changes, 原本 → 改成, in a popover.
private struct PresetChanges: View {
    let changes: [SettingChange]
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(changes) { change in
                HStack(spacing: 8) {
                    Text(change.setting.title).frame(width: 120, alignment: .leading)
                    Text(change.oldLabel).foregroundStyle(.secondary)
                    Image(systemName: "arrow.right").font(.caption).foregroundStyle(.tertiary)
                    Text(change.newLabel).fontWeight(.semibold)
                }
            }
        }
        .padding(16)
    }
}

private struct LessonStep: View {
    @ObservedObject var model: WizardModel
    let lesson: Lesson
    @State private var appeared = DemoPinning.still

    var body: some View {
        let done = model.completed.contains(lesson.id) || lesson.tasks.isEmpty
        // A horizontal list is wide and short: the practice field takes the whole width under the choices.  A vertical
        // one is narrow and tall: the field takes the right column, the whole height.
        let wide = model.sandbox.candidateLayout == .horizontal && !lesson.tasks.isEmpty
        StepPage(name: "lesson-" + lesson.id) {
            Group {
                if wide {
                    VStack(alignment: .leading, spacing: 18) {
                        StepTitle(title: lesson.title, detail: lesson.detail).entrance(0, appeared)
                        HStack(alignment: .top, spacing: 20) {
                            ForEach(lesson.groups) { group in
                                StyleGroupView(group: group, preferences: model.sandbox, across: lesson.groups.count == 1)
                                    .frame(maxWidth: .infinity, alignment: .topLeading)
                            }
                        }
                        .entrance(1, appeared)
                        demo.entrance(2, appeared)
                    }
                } else {
                    HStack(alignment: .top, spacing: 24) {
                        VStack(alignment: .leading, spacing: 20) {
                            StepTitle(title: lesson.title, detail: lesson.detail, size: 24).entrance(0, appeared)
                            VStack(alignment: .leading, spacing: 18) {
                                ForEach(lesson.groups) { group in StyleGroupView(group: group, preferences: model.sandbox, across: false) }
                            }
                            .entrance(1, appeared)
                        }
                        .frame(width: 236, alignment: .topLeading)
                        demo.frame(maxWidth: .infinity, alignment: .topLeading).entrance(2, appeared)
                    }
                }
            }
            .animation(WizardMotion.success, value: wide)
        } footer: {
            StepFooter {
                if lesson.groups.contains(where: { $0.styles.contains { $0.id.hasPrefix("keys.") } }) {
                    Button("自行設定…") { SettingsWindowController.shared.show(SettingsPlace(.keys)) }.buttonStyle(.link)
                }
            } trailing: {
                if !done { SecondaryButton(title: "略過這一步") { model.next() } }
                PrimaryButton(title: "下一步", enabled: done) { model.next() }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }

    /// The one live demo: the practice field (with the real window), or for a lesson without practice its preview.
    @ViewBuilder private var demo: some View {
        if let task = lesson.tasks.first {
            PracticeArea(model: model, lesson: lesson, task: task, done: model.completed.contains(lesson.id)) {
                withAnimation(WizardMotion.success) { _ = model.completed.insert(lesson.id) }
            }
        } else if let demo = lesson.demo {
            LessonDemoView(demo: demo, preferences: model.sandbox)
        }
    }
}

/// The demo of a lesson without practice, following the sandbox's settings: the real preview, or an ⓘ demo.
private struct LessonDemoView: View {
    let demo: LessonDemo
    @ObservedObject var preferences: Preferences
    @StateObject private var director = PreviewDirector()
    @Environment(\.stepRoom) private var room
    var body: some View {
        Group {
            switch demo {
            case .preview(let state):
                UnifiedPreview(preferences: preferences, director: director, tabs: false, maxHeight: max(160, room - 40))
                    .onAppear { director.pin(state) }
            case .space: SpaceInfo(current: preferences.spaceAction)
            case .punctuation: PunctuationTable(current: preferences.punctuationKeys)
            case .typo: TypoInfo(current: preferences.typoTolerance, correctionsKey: CurrentKeys(preferences).text(.correctionsList))
            case .fuzzy: FuzzyInfo()
            case .context: ContextIllustration(pinned: DemoPinning.still ? DemoPinning.context : nil)
            case .correctionLog: CorrectionLogInfo(keys: CurrentKeys(preferences))
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(Glass())
    }
}

/// A group of styles: exclusive ones as a choice whose highlight moves between the options (only the chosen one shows
/// its detail), combinable ones as checkboxes (their detail on hover).  `across`: the options side by side.
private struct StyleGroupView: View {
    let group: HabitGroup
    @ObservedObject var preferences: Preferences
    var across = false
    @Namespace private var namespace

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(group.title).font(.headline)
            if group.exclusive {
                let options = ForEach(group.styles) { style in option(style) }
                if across { HStack(spacing: 8) { options } } else { VStack(spacing: 6) { options } }
                if let detail = group.styles.first(where: { $0.isActive(preferences) })?.detail {
                    Text(detail)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                        .id(detail)
                        .transition(.opacity)
                }
            } else {
                TrailingFlow(spacing: across ? 16 : 6, lineSpacing: 8, leading: true) {
                    ForEach(group.styles) { style in
                        Toggle(style.title, isOn: Binding(get: { style.isActive(preferences) },
                                                          set: { on in withAnimation(WizardMotion.success) { style.set(preferences, on) } }))
                            .toggleStyle(.checkbox)
                            .help(style.detail ?? "")
                            .frame(maxWidth: across ? nil : .infinity, alignment: .leading)
                    }
                }
            }
        }
        .animation(WizardMotion.success, value: group.styles.map { $0.isActive(preferences) })
    }

    private func option(_ style: HabitStyle) -> some View {
        let on = style.isActive(preferences)
        return Button { withAnimation(WizardMotion.success) { style.set(preferences, true) } } label: {
            HStack(spacing: 10) {
                Image(systemName: on ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(on ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
                Text(style.title).fontWeight(on ? .semibold : .regular).lineLimit(1)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background {
                ZStack {
                    Glass(cornerRadius: 9)
                    if on {
                        RoundedRectangle(cornerRadius: 9, style: .continuous)
                            .fill(Color.accentColor.opacity(0.06))
                            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 1.5))
                            .matchedGeometryEffect(id: "selection", in: namespace)
                    }
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(style.detail ?? "")
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// Where the lesson's task is practised: the real practice field (an embedded ControllerCore with the sandbox's
/// settings and temporary user data, the real candidate window under it).
private struct PracticeArea: View {
    @ObservedObject var model: WizardModel
    let lesson: Lesson
    let task: PracticeTask
    let done: Bool
    let complete: () -> Void

    var body: some View {
        PracticeField(settings: SettingsSnapshot(model.sandbox), task: task,
                      onProgress: { progress in
                          let added = progress.userPhrases.filter { phrase in !model.practicePhrases.contains { $0.word == phrase.word } }
                          if !added.isEmpty { model.practicePhrases += added }
                      },
                      onComplete: {
                          if !Headless.isActive { NSHapticFeedbackManager.defaultPerformer.perform(.levelChange, performanceTime: .now) }
                          complete()
                      })
            .id(SettingsSnapshot(model.sandbox).values.hashValue)
            // No box around it: the field is the box, and under it the room the real window opens in, as in a
            // document.  Done, the field's border turns green and 「完成」 springs in beside the task.
    }
}

private struct FinishStep: View {
    @ObservedObject var model: WizardModel
    @State private var editing: String?
    @State private var showsRest = false
    @State private var appeared = DemoPinning.still

    private static let rowHeight: CGFloat = 41

    var body: some View {
        StepPage(name: "finish") { RoomReader { room in content(room: room) } } footer: {
            StepFooter { EmptyView() } trailing: {
                SecondaryButton(title: "全部捨棄") { model.discard() }
                PrimaryButton(title: "套用") {
                    model.apply { word, reading in try? SettingsWindowController.shared.model.engine?.addUserPhrase(word: word, reading: reading) }
                }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }

    private func content(room: CGFloat) -> some View {
        let changes = model.changes
        // As many rows as the window has room for; the rest behind 「其他 N 項」.
        let fixed: CGFloat = 56 + 20 + 30 + (model.practicePhrases.isEmpty ? 0 : 36)
        let fit = max(2, Int((room - fixed - 30) / Self.rowHeight))
        let shown = changes.count <= fit ? changes : Array(changes.prefix(fit - 1))
        let rest = Array(changes.dropFirst(shown.count))
        return VStack(alignment: .leading, spacing: 20) {
            HStack(spacing: 16) {
                DrawnCheck(drawn: appeared).frame(width: 46, height: 46)
                StepTitle(title: "設定好了", detail: changes.isEmpty ? "沒有要改變的設定。" : "按「套用」之前，還可以改回任何一項。")
                    .arrival(at: 0.3, appeared)
            }
            if !changes.isEmpty {
                VStack(spacing: 0) {
                    ForEach(Array(shown.enumerated()), id: \.element.id) { index, change in
                        if index > 0 { Divider().padding(.horizontal, 14) }
                        ChangeRow(change: change, sandbox: model.sandbox, editing: Binding(get: { editing == change.id },
                                                                                           set: { editing = $0 ? change.id : nil }),
                                  revert: { withAnimation(.easeOut(duration: 0.25)) { model.revert(change) } })
                            .entrance(2 + index, appeared)
                    }
                    if !rest.isEmpty {
                        Divider().padding(.horizontal, 14)
                        HStack {
                            Button("其他 \(rest.count) 項…") { showsRest.toggle() }
                                .buttonStyle(.link)
                                .popover(isPresented: $showsRest, arrowEdge: .bottom) { PresetChanges(changes: rest) }
                            Spacer()
                        }
                        .padding(.horizontal, 14)
                        .frame(height: Self.rowHeight)
                    }
                }
                .background(Glass())
                .entrance(1, appeared)
            }
            if !model.practicePhrases.isEmpty {
                Toggle("保留練習時加入的自訂詞（\(model.practicePhrases.map(\.word).joined(separator: "、"))）", isOn: $model.keepPracticePhrases)
                    .toggleStyle(.checkbox)
                    .lineLimit(1)
                    .entrance(3 + shown.count, appeared)
            }
        }
    }
}

/// The finish moment: a green circle drawn round, then its check.
private struct DrawnCheck: View {
    let drawn: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        let green = Color(nsColor: .systemGreen)
        ZStack {
            Circle().fill(green.opacity(drawn ? 0.12 : 0))
            Circle().trim(from: 0, to: drawn ? 1 : 0)
                .stroke(green, style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                .rotationEffect(.degrees(-90))
                .animation(reduceMotion ? .easeOut(duration: 0.2) : .easeInOut(duration: 0.5).delay(0.1), value: drawn)
            CheckShape().trim(from: 0, to: drawn ? 1 : 0)
                .stroke(green, style: StrokeStyle(lineWidth: 3, lineCap: .round, lineJoin: .round))
                .padding(13)
                .animation(reduceMotion ? .easeOut(duration: 0.2) : .easeOut(duration: 0.28).delay(0.5), value: drawn)
        }
        .animation(.easeOut(duration: 0.3).delay(0.3), value: drawn)
        .accessibilityHidden(true)
    }
}

private struct CheckShape: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: rect.minX, y: rect.midY + rect.height * 0.04))
        path.addLine(to: CGPoint(x: rect.minX + rect.width * 0.36, y: rect.maxY - rect.height * 0.12))
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY + rect.height * 0.12))
        return path
    }
}

/// 原本 → 改成, with 改回; the setting's name opens its control in a popover (the page never grows).
private struct ChangeRow: View {
    let change: SettingChange
    @ObservedObject var sandbox: Preferences
    @Binding var editing: Bool
    let revert: () -> Void
    var body: some View {
        HStack(spacing: 10) {
            Button { editing.toggle() } label: {
                HStack(spacing: 4) {
                    Text(change.setting.title).fontWeight(.medium).lineLimit(1)
                    Image(systemName: "chevron.down").font(.caption2.weight(.semibold)).foregroundStyle(.tertiary)
                }
            }
            .buttonStyle(.plain)
            .popover(isPresented: $editing, arrowEdge: .bottom) {
                SettingControl(setting: change.setting, preferences: sandbox).padding(14)
            }
            .frame(width: 170, alignment: .leading)
            Text(change.oldLabel).foregroundStyle(.secondary).strikethrough(true, color: .secondary).lineLimit(1)
            Image(systemName: "arrow.right").font(.caption).foregroundStyle(.tertiary)
            Text(change.newLabel).fontWeight(.semibold).foregroundStyle(Color.accentColor).lineLimit(1)
            Spacer(minLength: 0)
            Button("改回", action: revert).buttonStyle(.link).font(.callout)
        }
        .padding(.horizontal, 14)
        .frame(height: 41)
    }
}

/// 你會用到的按鍵: from the final settings, only what is on.
struct KeySummaryStep: View {
    @ObservedObject var model: WizardModel
    @State private var appeared = DemoPinning.still
    var body: some View {
        StepPage(name: "keys") {
            VStack(alignment: .leading, spacing: 20) {
                StepTitle(title: "你會用到的按鍵", detail: "依你的設定產生；之後可以在「介紹」再看一次。").entrance(0, appeared)
                KeySummary(preferences: model.original, appeared: appeared)
            }
        } footer: {
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "完成") { model.onClose?() }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

/// The keys of the current settings, in cards by what they are for: the first few of each (the rest are on the
/// 快捷鍵 page).
struct KeySummary: View {
    @ObservedObject var preferences: Preferences
    var appeared = true
    static let perCard = 4
    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)],
                  alignment: .leading, spacing: 14) {
            ForEach(Array(Self.groups(preferences).enumerated()), id: \.element.title) { index, group in
                KeyCard(order: index, appeared: appeared) {
                    VStack(alignment: .leading, spacing: 9) {
                        Label(group.title, systemImage: group.symbol).font(.headline)
                        ForEach(Array(group.items.prefix(Self.perCard).enumerated()), id: \.offset) { _, item in
                            HStack(spacing: 10) {
                                HStack(spacing: 4) { ForEach(item.keys, id: \.self) { KeyChip(text: $0) } }
                                    .frame(width: 92, alignment: .leading)
                                Text(item.action).font(.callout).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
            }
        }
    }

    static func groups(_ p: Preferences) -> [(title: String, symbol: String, items: [(keys: [String], action: String)])] {
        let keys = CurrentKeys(p)
        func commands(_ section: KeySection) -> [(keys: [String], action: String)] {
            ShortcutCommand.allCases.filter { $0.section == section && p.isEnabled($0) }.compactMap { command in
                let list = (p.bindings[command] ?? []).prefix(2).map(\.compact)
                return list.isEmpty ? nil : (Array(list), command.label)
            }
        }
        var typing: [(keys: [String], action: String)] = [
            (["空白"], OptionDemos.spaceLabel(p.spaceAction)),
            (["Esc"], p.escapeClearsComposition ? "清除注音，再按一次清除組字" : "清除注音"),
        ]
        if let mode = keys.modeKey { typing.append(([mode], "切換中英")) }
        return [
            ("打字", "keyboard", typing + commands(.composing)),
            ("候選字清單", "list.bullet.rectangle", [(["↓"], "開啟候選字清單"), ([keys.selection], "選字")] + commands(.list)),
            ("改字與選取", "character.cursor.ibeam", commands(.editing)),
        ].filter { !$0.2.isEmpty }
    }
}

private struct KeyCard<Content: View>: View {
    let order: Int
    let appeared: Bool
    @ViewBuilder let content: () -> Content
    var body: some View {
        content()
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .background(Glass())
            .entrance(1 + order, appeared)
    }
}
