import SwiftUI
import ZhuyinEngine

// The setup wizard's window content: a header (上一步, progress, 全部跳過), the step, and its buttons.  Steps push in from
// the side they come from; under Reduce Motion they crossfade.

struct WizardView: View {
    @ObservedObject var model: WizardModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().opacity(model.step == .welcome ? 0 : 1)
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
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var transition: AnyTransition {
        if reduceMotion { return .opacity }
        let forward = model.direction > 0
        return .asymmetric(insertion: .modifier(active: WizardSlide(x: forward ? 28 : -28, opacity: 0), identity: WizardSlide(x: 0, opacity: 1)),
                           removal: .modifier(active: WizardSlide(x: forward ? -14 : 14, opacity: 0), identity: WizardSlide(x: 0, opacity: 1)))
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
                            .fill(index <= progress.index ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.quaternary))
                            .frame(width: index == progress.index ? 18 : 7, height: 7)
                    }
                }
                .animation(WizardMotion.step, value: progress.index)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("第 \(progress.index) 步，共 \(progress.count) 步")
            }
            Spacer()
            if model.step != .finish, model.step != .keys {
                Button("全部跳過") { model.skipAll() }.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 44)
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

// MARK: - Pieces

private struct StepTitle: View {
    let title: String
    let detail: String
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
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
        .padding(.horizontal, 24)
        .padding(.vertical, 16)
        .background(alignment: .top) { Divider() }
    }
}

/// The step's main action: prominent, in the accent colour.
private struct PrimaryButton: View {
    let title: String
    var enabled = true
    let action: () -> Void
    var body: some View {
        Button(title, action: action)
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.defaultAction)
            .disabled(!enabled)
            .frame(minWidth: 96)
    }
}

/// A secondary action beside the primary one.
private struct SecondaryButton: View {
    let title: String
    let action: () -> Void
    var body: some View { Button(title, action: action).buttonStyle(.bordered) }
}

private struct Card<Content: View>: View {
    var selected = false
    @ViewBuilder let content: () -> Content
    var body: some View {
        content()
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(selected ? Color.accentColor.opacity(0.08) : Color.primary.opacity(0.035)))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(selected ? Color.accentColor : Color.primary.opacity(0.08), lineWidth: selected ? 2 : 1))
    }
}

private struct ChoiceCard: View {
    let title: String
    let lines: [(keys: [String], text: String)]
    var symbol: String?
    let selected: Bool
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Card(selected: selected) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        if let symbol { Image(systemName: symbol).font(.title2).foregroundStyle(Color.accentColor) }
                        Text(title).font(.headline)
                        Spacer()
                        Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                            .font(.title3)
                            .foregroundStyle(selected ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
                            .scaleEffect(selected ? 1 : 0.9)
                    }
                    ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                        HStack(spacing: 6) {
                            ForEach(line.keys, id: \.self) { KeyChip(text: $0) }
                            Text(line.text).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
                .frame(minHeight: 96, alignment: .topLeading)
            }
            .contentShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .animation(WizardMotion.success, value: selected)
    }
}

// MARK: - Steps

private struct WelcomeStep: View {
    @ObservedObject var model: WizardModel
    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(spacing: 22) {
                    HStack(alignment: .center, spacing: 18) {
                        Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 84, height: 84)
                            .shadow(color: .black.opacity(0.15), radius: 8, y: 3)
                        VStack(alignment: .leading, spacing: 6) {
                            Text("設定 ZhuyinType").font(.system(size: 30, weight: .semibold))
                            Text("選好你習慣的打法，並在這裡試打。所有設定之後都能在「設定」中更改。")
                                .font(.title3).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer(minLength: 0)
                    }
                    HStack(spacing: 14) {
                        ChoiceCard(title: "快速設定", lines: [([], "標點、空白鍵、中英切換、選字與翻頁、改字"), ([], "約 2 分鐘")], symbol: "bolt.fill",
                                   selected: model.mode == .quick) { model.choose(.quick) }
                        ChoiceCard(title: "完整導覽", lines: [([], "依序介紹每個功能，並在每一步試打"), ([], "\(Lesson.all.count) 個步驟")], symbol: "map.fill",
                                   selected: model.mode == .full) { model.choose(.full) }
                    }
                    // The real candidate window, playing its states one after another.
                    WelcomePreview(preferences: model.sandbox)
                        .padding(16)
                        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.primary.opacity(0.035)))
                }
                .padding(.horizontal, 40)
                .padding(.vertical, 28)
            }
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "開始") { model.next() }
            }
        }
    }
}

private struct WelcomePreview: View {
    @ObservedObject var preferences: Preferences
    @StateObject private var director = PreviewDirector()
    var body: some View { UnifiedPreview(preferences: preferences, director: director, maxHeight: 220) }
}

private struct PresetStep: View {
    @ObservedObject var model: WizardModel
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    StepTitle(title: "你原本習慣哪一種注音輸入？", detail: "選一個最接近的習慣組合，之後每一步都可以再改。")
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 14) {
                        ForEach(SettingsPreset.all) { preset in
                            ChoiceCard(title: preset.title, lines: preset.lines, selected: model.preset == preset.id) {
                                withAnimation(WizardMotion.success) { model.choosePreset(preset) }
                            }
                        }
                    }
                    PresetChanges(changes: model.changes)
                }
                .padding(.horizontal, 40)
                .padding(.vertical, 28)
            }
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "下一步") { model.next() }
            }
        }
    }
}

/// What the chosen preset changes, as 原本 → 改成 chips.
private struct PresetChanges: View {
    let changes: [SettingChange]
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(changes.isEmpty ? "不改變設定" : "會改變 \(changes.count) 項設定")
                .font(.headline)
                .contentTransition(.numericText())
            TrailingFlow(spacing: 8, lineSpacing: 8, leading: true) {
                ForEach(changes) { change in
                    HStack(spacing: 6) {
                        Text(change.setting.title).foregroundStyle(.secondary)
                        Text(change.newLabel).fontWeight(.medium)
                    }
                    .font(.callout)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(Capsule().fill(Color.accentColor.opacity(0.1)))
                    .transition(.scale(scale: 0.9).combined(with: .opacity))
                }
            }
        }
        .animation(WizardMotion.success, value: changes.map(\.id))
    }
}

private struct LessonStep: View {
    @ObservedObject var model: WizardModel
    let lesson: Lesson

    var body: some View {
        let done = model.completed.contains(lesson.id) || lesson.tasks.isEmpty
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    StepTitle(title: lesson.title, detail: lesson.detail)
                    ViewThatFits(in: .horizontal) {
                        HStack(alignment: .top, spacing: 24) {
                            choices.frame(minWidth: 280, maxWidth: .infinity, alignment: .topLeading)
                            demo.frame(minWidth: 320, maxWidth: .infinity, alignment: .topLeading)
                        }
                        VStack(alignment: .leading, spacing: 20) { choices; demo }
                    }
                    if let task = lesson.tasks.first {
                        PracticeArea(lesson: lesson, task: task, preferences: model.sandbox, done: model.completed.contains(lesson.id)) {
                            withAnimation(WizardMotion.success) { _ = model.completed.insert(lesson.id) }
                        }
                    }
                }
                .padding(.horizontal, 40)
                .padding(.vertical, 28)
            }
            StepFooter {
                if lesson.groups.contains(where: { $0.styles.contains { $0.id.hasPrefix("keys.") } }) {
                    Button("自行設定…") { SettingsWindowController.shared.show(SettingsPlace(.keys)) }.buttonStyle(.link)
                }
            } trailing: {
                if !done { SecondaryButton(title: "略過這一步") { model.next() } }
                PrimaryButton(title: "下一步", enabled: done) { model.next() }
            }
        }
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
                UnifiedPreview(preferences: preferences, director: director, tabs: false)
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
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.primary.opacity(0.035)))
    }
}

/// A group of styles: exclusive ones as a choice, combinable ones as checkboxes.
private struct StyleGroupView: View {
    let group: HabitGroup
    @ObservedObject var preferences: Preferences
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(group.title).font(.headline)
            if group.exclusive {
                VStack(spacing: 8) {
                    ForEach(group.styles) { style in
                        let on = style.isActive(preferences)
                        Button { withAnimation(WizardMotion.success) { style.set(preferences, true) } } label: {
                            HStack(spacing: 10) {
                                Image(systemName: on ? "largecircle.fill.circle" : "circle")
                                    .foregroundStyle(on ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.tertiary))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(style.title).fontWeight(on ? .semibold : .regular)
                                    if let detail = style.detail {
                                        Text(detail).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 12).padding(.vertical, 9)
                            .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(on ? Color.accentColor.opacity(0.1) : Color.primary.opacity(0.035)))
                            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(on ? Color.accentColor : Color.primary.opacity(0.08), lineWidth: on ? 1.5 : 1))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(on ? .isSelected : [])
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 7) {
                    ForEach(group.styles) { style in
                        Toggle(isOn: Binding(get: { style.isActive(preferences) }, set: { style.set(preferences, $0) })) {
                            HStack(spacing: 8) {
                                Text(style.title)
                                if let detail = style.detail { Text(detail).font(.caption).foregroundStyle(.secondary) }
                            }
                        }
                        .toggleStyle(.checkbox)
                    }
                }
                .padding(.horizontal, 12).padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(Color.primary.opacity(0.035)))
            }
        }
    }
}

/// Where the lesson's task is practised.  The practice field (an embedded ControllerCore with the sandbox's settings,
/// the real candidate window under it) plugs in here; until it lands, the task can be marked done.
private struct PracticeArea: View {
    let lesson: Lesson
    let task: PracticeTask
    @ObservedObject var preferences: Preferences
    let done: Bool
    let complete: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 14) {
            Image(systemName: done ? "checkmark.circle.fill" : "keyboard")
                .font(.system(size: 26))
                .foregroundStyle(done ? AnyShapeStyle(Color(nsColor: .systemGreen)) : AnyShapeStyle(Color.accentColor))
                .scaleEffect(done ? 1.1 : 1)
                .contentTransition(.symbolEffect(.replace))
            VStack(alignment: .leading, spacing: 3) {
                Text("試試看").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Text(task.prompt).font(.headline)
                if let hint = task.hint { Text(hint).font(.callout).foregroundStyle(.secondary) }
            }
            Spacer()
            if done {
                Text("完成").font(.headline).foregroundStyle(Color(nsColor: .systemGreen)).transition(.scale(scale: 0.6).combined(with: .opacity))
            } else {
                Button("我打好了", action: complete).buttonStyle(.bordered)
            }
        }
        .padding(16)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(done ? Color(nsColor: .systemGreen).opacity(0.08) : Color.primary.opacity(0.035)))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
            .strokeBorder(done ? Color(nsColor: .systemGreen) : Color.accentColor.opacity(0.4), lineWidth: done ? 2 : 1))
        .animation(WizardMotion.success, value: done)
    }
}

private struct FinishStep: View {
    @ObservedObject var model: WizardModel
    @State private var editing: String?
    @State private var arrived = DemoPinning.still
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let changes = model.changes
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HStack(spacing: 16) {
                        Image(systemName: "checkmark.seal.fill")
                            .font(.system(size: 44))
                            .foregroundStyle(Color(nsColor: .systemGreen))
                            .scaleEffect(arrived ? 1 : 0.3)
                            .opacity(arrived ? 1 : 0)
                        StepTitle(title: "設定好了", detail: changes.isEmpty ? "沒有要改變的設定。"
                                  : "按「套用」之前，可以在這裡改回或再調整每一項。")
                    }
                    if !changes.isEmpty {
                        VStack(spacing: 0) {
                            ForEach(Array(changes.enumerated()), id: \.element.id) { index, change in
                                if index > 0 { Divider().padding(.horizontal, 16) }
                                ChangeRow(change: change, sandbox: model.sandbox, editing: editing == change.id,
                                          edit: { withAnimation(.easeOut(duration: 0.2)) { editing = editing == change.id ? nil : change.id } },
                                          revert: { withAnimation(.easeOut(duration: 0.25)) { model.revert(change) } })
                                    .opacity(arrived ? 1 : 0)
                                    .offset(y: arrived || reduceMotion ? 0 : 10)
                                    .animation(reduceMotion ? .easeOut(duration: 0.2) : .spring(response: 0.4, dampingFraction: 0.8)
                                        .delay(0.25 + Double(index) * 0.07), value: arrived)
                            }
                        }
                        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.primary.opacity(0.035)))
                    }
                    if !model.practicePhrases.isEmpty {
                        Toggle("保留練習中加入的自訂詞（\(model.practicePhrases.map(\.word).joined(separator: "、"))）", isOn: $model.keepPracticePhrases)
                            .toggleStyle(.checkbox)
                    }
                }
                .padding(.horizontal, 40)
                .padding(.vertical, 28)
            }
            StepFooter { EmptyView() } trailing: {
                SecondaryButton(title: "全部捨棄") { model.discard() }
                PrimaryButton(title: "套用") {
                    model.apply { word, reading in try? SettingsWindowController.shared.model.engine?.addUserPhrase(word: word, reading: reading) }
                }
            }
        }
        .onAppear { withAnimation(WizardMotion.success) { arrived = true } }
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
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    StepTitle(title: "你會用到的按鍵", detail: "依你的設定產生；之後可以在「介紹」再看一次。")
                    KeySummary(preferences: model.original)
                }
                .padding(.horizontal, 40)
                .padding(.vertical, 28)
            }
            StepFooter { EmptyView() } trailing: {
                PrimaryButton(title: "完成") { model.onClose?() }
            }
        }
    }
}

/// The keys of the current settings, in cards by what they are for.
struct KeySummary: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)],
                  alignment: .leading, spacing: 14) {
            ForEach(Self.groups(preferences), id: \.title) { group in
                Card {
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
