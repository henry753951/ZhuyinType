import SwiftUI
import ZhuyinEngine

// The setup wizard's window content, quiet as Apple's own setup screens: the window background, faint grouped surfaces,
// the system accent colour only for the main button and the selection, and type and spacing doing the rest.  A header
// (上一步, progress, 全部跳過); the step; its buttons.  Steps move in from the side they come from and their parts arrive
// one after another (10 pt, 40 ms apart, soft springs); a choice's highlight moves between the cards.  Under Reduce
// Motion everything crossfades.

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
        .frame(minWidth: 720, minHeight: 560)
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
            let progress = model.progress
            if model.step != .welcome, progress.count > 1 {
                HStack(spacing: 6) {
                    ForEach(1...progress.count, id: \.self) { index in
                        Capsule()
                            .fill(index <= progress.index ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(Color.primary.opacity(0.15)))
                            .frame(width: index == progress.index ? 22 : 7, height: 7)
                    }
                }
                .animation(WizardMotion.success, value: progress.index)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("第 \(progress.index) 步，共 \(progress.count) 步")
            }
            Spacer()
            if model.step != .finish, model.step != .keys {
                Button("全部跳過") { model.skipAll() }.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        }
        .padding(.leading, 84)
        .padding(.trailing, 24)
        .frame(height: 52)
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

// MARK: - Backdrop, glass and entrances

/// The window's backdrop: the plain window background, as Apple's setup screens and System Settings.
struct WizardBackdrop: View {
    var body: some View {
        Color(nsColor: .windowBackgroundColor).ignoresSafeArea().accessibilityHidden(true)
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

/// A part of a step arriving: it rises a little and fades in, after the parts before it.
private struct Entrance: ViewModifier {
    let order: Int
    let appeared: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    func body(content: Content) -> some View {
        content
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared || reduceMotion ? 0 : 10)
            .animation(reduceMotion ? .easeOut(duration: 0.2).delay(Double(order) * 0.03)
                       : .spring(response: 0.42, dampingFraction: 0.92).delay(0.05 + Double(order) * 0.04), value: appeared)
    }
}

extension View {
    fileprivate func entrance(_ order: Int, _ appeared: Bool) -> some View { modifier(Entrance(order: order, appeared: appeared)) }
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
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.system(size: 26, weight: .semibold))
            Text(detail).font(.title3).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
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
        .padding(.vertical, 16)
    }
}

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

/// A step's page: its content, fitted to the window (scrolling without a scroller only when the window is too short),
/// and its footer.
private struct StepPage<Content: View, Footer: View>: View {
    @ViewBuilder let content: () -> Content
    @ViewBuilder let footer: () -> Footer
    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                content()
                    .padding(.horizontal, 44)
                    .padding(.top, 12)
                    .padding(.bottom, 20)
                    .frame(maxWidth: 940)
                    .frame(maxWidth: .infinity)
            }
            .scrollIndicators(.never)
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
            VStack(alignment: .leading, spacing: 10) {
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
                        Text(line.text).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, minHeight: 100, alignment: .topLeading)
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

    var body: some View {
        StepPage {
            VStack(spacing: 22) {
                VStack(spacing: 12) {
                    Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 80, height: 80)
                    Text("歡迎使用 ZhuyinType").font(.system(size: 28, weight: .semibold))
                    Text("選好你習慣的打法，並在這裡試打。所有設定之後都能在「設定」中更改。")
                        .font(.title3).foregroundStyle(.secondary).multilineTextAlignment(.center).frame(maxWidth: 480)
                }
                .entrance(0, appeared)
                HStack(spacing: 16) {
                    ChoiceCard(title: "快速設定", lines: [([], "標點、空白鍵、中英切換、選字與翻頁、改字"), ([], "約 2 分鐘")], symbol: "bolt.fill",
                               selected: model.mode == .quick, namespace: namespace, group: "mode") {
                        withAnimation(WizardMotion.success) { model.choose(.quick) }
                    }
                    .entrance(1, appeared)
                    ChoiceCard(title: "完整導覽", lines: [([], "依序介紹每個功能，並在每一步試打"), ([], "\(Lesson.all.count) 個步驟")], symbol: "map.fill",
                               selected: model.mode == .full, namespace: namespace, group: "mode") {
                        withAnimation(WizardMotion.success) { model.choose(.full) }
                    }
                    .entrance(2, appeared)
                }
                WelcomePreview(preferences: model.sandbox)
                    .padding(16)
                    .background(Glass())
                    .entrance(3, appeared)
            }
            .padding(.top, 10)
        } footer: {
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "開始") { model.next() }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

private struct WelcomePreview: View {
    @ObservedObject var preferences: Preferences
    @StateObject private var director = PreviewDirector()
    var body: some View { UnifiedPreview(preferences: preferences, director: director, maxHeight: 200) }
}

private struct PresetStep: View {
    @ObservedObject var model: WizardModel
    @Namespace private var namespace
    @State private var appeared = DemoPinning.still

    var body: some View {
        StepPage {
            VStack(alignment: .leading, spacing: 20) {
                StepTitle(title: "你原本習慣哪一種注音輸入？", detail: "選一個最接近的習慣組合，之後每一步都可以再改。")
                    .entrance(0, appeared)
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 16), GridItem(.flexible(), spacing: 16)], spacing: 16) {
                    ForEach(Array(SettingsPreset.all.enumerated()), id: \.element.id) { index, preset in
                        ChoiceCard(title: preset.title, lines: preset.lines, selected: model.preset == preset.id,
                                   namespace: namespace, group: "preset") {
                            withAnimation(WizardMotion.success) { model.choosePreset(preset) }
                        }
                        .entrance(1 + index, appeared)
                    }
                }
                PresetChanges(changes: model.changes).entrance(6, appeared)
            }
        } footer: {
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "下一步") { model.next() }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

/// What the chosen preset changes, as chips.
private struct PresetChanges: View {
    let changes: [SettingChange]
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(changes.isEmpty ? "和目前的設定相同，不改變設定" : "會改變 \(changes.count) 項設定")
                .font(.headline)
                .contentTransition(.numericText())
            TrailingFlow(spacing: 8, lineSpacing: 8, leading: true) {
                ForEach(changes) { change in
                    HStack(spacing: 6) {
                        Text(change.setting.title).foregroundStyle(.secondary)
                        Text(change.newLabel).fontWeight(.semibold)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(Capsule().fill(Color.primary.opacity(0.06)))
                    .transition(.scale(scale: 0.85).combined(with: .opacity))
                }
            }
        }
        .animation(WizardMotion.success, value: changes.map(\.id))
    }
}

private struct LessonStep: View {
    @ObservedObject var model: WizardModel
    let lesson: Lesson
    @State private var appeared = DemoPinning.still

    var body: some View {
        let done = model.completed.contains(lesson.id) || lesson.tasks.isEmpty
        StepPage {
            VStack(alignment: .leading, spacing: 20) {
                StepTitle(title: lesson.title, detail: lesson.detail).entrance(0, appeared)
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 22) {
                        choices.frame(minWidth: 280, maxWidth: .infinity, alignment: .topLeading).entrance(1, appeared)
                        demo.frame(minWidth: 320, maxWidth: .infinity, alignment: .topLeading).entrance(2, appeared)
                    }
                    VStack(alignment: .leading, spacing: 20) {
                        choices.entrance(1, appeared)
                        demo.entrance(2, appeared)
                    }
                }
                if let task = lesson.tasks.first {
                    PracticeArea(model: model, lesson: lesson, task: task, done: model.completed.contains(lesson.id)) {
                        withAnimation(WizardMotion.success) { _ = model.completed.insert(lesson.id) }
                    }
                    .entrance(3, appeared)
                }
            }
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

    private var choices: some View {
        VStack(alignment: .leading, spacing: 18) {
            ForEach(lesson.groups) { group in StyleGroupView(group: group, preferences: model.sandbox) }
        }
    }

    @ViewBuilder private var demo: some View {
        if let demo = lesson.demo { LessonDemoView(demo: demo, preferences: model.sandbox) }
    }
}

/// The demo beside a lesson's choices, following the sandbox's settings: the real preview, or an ⓘ demo.
private struct LessonDemoView: View {
    let demo: LessonDemo
    @ObservedObject var preferences: Preferences
    @StateObject private var director = PreviewDirector()
    var body: some View {
        Group {
            switch demo {
            case .preview(let state):
                UnifiedPreview(preferences: preferences, director: director, tabs: false, maxHeight: 240)
                    .onAppear { director.show(state, animated: false) }
            case .space: SpaceInfo(current: preferences.spaceAction)
            case .punctuation: PunctuationTable(current: preferences.punctuationKeys)
            case .typo: TypoInfo(current: preferences.typoTolerance, correctionsKey: CurrentKeys(preferences).text(.correctionsList))
            case .fuzzy: FuzzyInfo()
            case .context: ContextIllustration(pinned: DemoPinning.still ? DemoPinning.context : nil)
            case .correctionLog: CorrectionLogInfo(keys: CurrentKeys(preferences))
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(Glass())
    }
}

/// A group of styles: exclusive ones as a choice whose highlight moves between the options, combinable ones as
/// checkboxes.
private struct StyleGroupView: View {
    let group: HabitGroup
    @ObservedObject var preferences: Preferences
    @Namespace private var namespace

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(group.title).font(.headline)
            if group.exclusive {
                VStack(spacing: 8) {
                    ForEach(group.styles) { style in
                        let on = style.isActive(preferences)
                        Button { withAnimation(WizardMotion.success) { style.set(preferences, true) } } label: {
                            HStack(spacing: 12) {
                                Image(systemName: on ? "largecircle.fill.circle" : "circle")
                                    .font(.title3)
                                    .foregroundStyle(on ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(style.title).font(.body.weight(on ? .semibold : .regular))
                                    if let detail = style.detail {
                                        Text(detail).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 14).padding(.vertical, 11)
                            .background {
                                ZStack {
                                    Glass(cornerRadius: 10)
                                    if on {
                                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                                            .fill(Color.accentColor.opacity(0.06))
                                            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 1.5))
                                            .matchedGeometryEffect(id: "selection", in: namespace)
                                    }
                                }
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(on ? .isSelected : [])
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 9) {
                    ForEach(group.styles) { style in
                        Toggle(isOn: Binding(get: { style.isActive(preferences) },
                                             set: { on in withAnimation(WizardMotion.success) { style.set(preferences, on) } })) {
                            HStack(spacing: 8) {
                                Text(style.title)
                                if let detail = style.detail { Text(detail).font(.callout).foregroundStyle(.secondary) }
                            }
                        }
                        .toggleStyle(.checkbox)
                    }
                }
                .padding(.horizontal, 16).padding(.vertical, 14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Glass(cornerRadius: 10))
            }
        }
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
                      onComplete: complete)
            .id(SettingsSnapshot(model.sandbox).values.hashValue)
            .padding(16)
            .background(Glass())
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(done ? Color(nsColor: .systemGreen) : Color.clear, lineWidth: 2))
            .animation(WizardMotion.success, value: done)
    }
}

private struct FinishStep: View {
    @ObservedObject var model: WizardModel
    @State private var editing: String?
    @State private var appeared = DemoPinning.still

    var body: some View {
        let changes = model.changes
        StepPage {
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 18) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 40))
                        .foregroundStyle(Color(nsColor: .systemGreen))
                        .scaleEffect(appeared ? 1 : 0.85)
                        .opacity(appeared ? 1 : 0)
                        .animation(.spring(response: 0.4, dampingFraction: 0.8), value: appeared)
                    StepTitle(title: "設定好了", detail: changes.isEmpty ? "沒有要改變的設定。"
                              : "按「套用」之前，可以在這裡改回或再調整每一項。")
                        .entrance(0, appeared)
                }
                if !changes.isEmpty {
                    VStack(spacing: 0) {
                        ForEach(Array(changes.enumerated()), id: \.element.id) { index, change in
                            if index > 0 { Divider().padding(.horizontal, 16) }
                            ChangeRow(change: change, sandbox: model.sandbox, editing: editing == change.id,
                                      edit: { withAnimation(.easeOut(duration: 0.2)) { editing = editing == change.id ? nil : change.id } },
                                      revert: { withAnimation(.easeOut(duration: 0.25)) { model.revert(change) } })
                                .entrance(2 + index, appeared)
                        }
                    }
                    .background(Glass())
                    .entrance(1, appeared)
                }
                if !model.practicePhrases.isEmpty {
                    Toggle("保留練習中加入的自訂詞（\(model.practicePhrases.map(\.word).joined(separator: "、"))）", isOn: $model.keepPracticePhrases)
                        .toggleStyle(.checkbox)
                        .entrance(3 + changes.count, appeared)
                }
            }
        } footer: {
            StepFooter { EmptyView() } trailing: {
                SecondaryButton(title: "全部捨棄") { model.discard() }
                PrimaryButton(title: "套用") {
                    model.apply { word, reading in try? SettingsWindowController.shared.model.engine?.addUserPhrase(word: word, reading: reading) }
                }
            }
        }
        .modifier(Arrival(appeared: $appeared))
    }
}

/// 原本 → 改成, with 改回; the setting's name opens its control in place.
private struct ChangeRow: View {
    let change: SettingChange
    @ObservedObject var sandbox: Preferences
    let editing: Bool
    let edit: () -> Void
    let revert: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Button(action: edit) {
                    HStack(spacing: 4) {
                        Text(change.setting.title).fontWeight(.medium)
                        Image(systemName: "chevron.right").font(.caption2.weight(.semibold)).foregroundStyle(.tertiary)
                            .rotationEffect(.degrees(editing ? 90 : 0))
                    }
                }
                .buttonStyle(.plain)
                .frame(width: 180, alignment: .leading)
                Text(change.oldLabel).foregroundStyle(.secondary).strikethrough(true, color: .secondary).lineLimit(1)
                Image(systemName: "arrow.right").font(.caption).foregroundStyle(.tertiary)
                Text(change.newLabel).fontWeight(.semibold).foregroundStyle(Color.accentColor).lineLimit(1)
                Spacer()
                Button("改回", action: revert).buttonStyle(.link).font(.callout)
            }
            if editing {
                HStack {
                    Spacer()
                    SettingControl(setting: change.setting, preferences: sandbox)
                }
                .transition(.opacity)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }
}

/// 你會用到的按鍵: from the final settings, only what is on.
struct KeySummaryStep: View {
    @ObservedObject var model: WizardModel
    @State private var appeared = DemoPinning.still
    var body: some View {
        StepPage {
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

/// The keys of the current settings, in cards by what they are for.
struct KeySummary: View {
    @ObservedObject var preferences: Preferences
    var appeared = true
    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)],
                  alignment: .leading, spacing: 14) {
            ForEach(Array(Self.groups(preferences).enumerated()), id: \.element.title) { index, group in
                KeyCard(order: index, appeared: appeared) {
                    VStack(alignment: .leading, spacing: 10) {
                        Label(group.title, systemImage: group.symbol).font(.headline)
                        ForEach(Array(group.items.enumerated()), id: \.offset) { _, item in
                            HStack(spacing: 10) {
                                HStack(spacing: 4) { ForEach(item.keys, id: \.self) { KeyChip(text: $0) } }
                                    .frame(width: 92, alignment: .leading)
                                Text(item.action).font(.callout).fixedSize(horizontal: false, vertical: true)
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
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .background(Glass())
            .entrance(1 + order, appeared)
    }
}
