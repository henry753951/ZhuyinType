import AppKit
import SwiftUI

// Building blocks shared by the settings panes: keycaps, info popovers and the small looping demos that show what an
// option does instead of describing it in a paragraph.

/// A two-way binding to a `Preferences` property.
func binding<Value>(_ preferences: Preferences, _ keyPath: ReferenceWritableKeyPath<Preferences, Value>) -> Binding<Value> {
    Binding(get: { preferences[keyPath: keyPath] }, set: { preferences[keyPath: keyPath] = $0 })
}

/// One short line under a section, for what the rows themselves cannot say.
struct FooterText: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// A row's title with an optional one-line subtitle, as System Settings draws them.
struct RowLabel: View {
    let title: String
    let subtitle: String?
    init(_ title: String, _ subtitle: String? = nil) {
        self.title = title
        self.subtitle = subtitle
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if let subtitle {
                Text(subtitle)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Keys drawn as keycaps.  `tint` marks them (system red for a conflict).
struct KeyCombo: View {
    let keys: [String]
    var size: CGFloat = 12
    var highlighted = false
    var tint: Color?
    var body: some View {
        HStack(spacing: 3) {
            ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
                Keycap(key: key, size: size, highlighted: highlighted, tint: tint)
            }
        }
    }
}

struct Keycap: View {
    let key: String
    var size: CGFloat = 12
    var highlighted = false
    var tint: Color?

    private var ink: AnyShapeStyle {
        if highlighted { return AnyShapeStyle(Color.white) }
        if let tint { return AnyShapeStyle(tint) }
        return AnyShapeStyle(.secondary)
    }
    private var fill: AnyShapeStyle {
        if highlighted { return AnyShapeStyle(Color.accentColor) }
        if let tint { return AnyShapeStyle(tint.opacity(0.1)) }
        return AnyShapeStyle(.quaternary.opacity(0.6))
    }
    private var edge: AnyShapeStyle {
        if highlighted { return AnyShapeStyle(Color.clear) }
        if let tint { return AnyShapeStyle(tint.opacity(0.8)) }
        return AnyShapeStyle(.quaternary)
    }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: size * 0.42)
        Text(key)
            .font(.system(size: size, weight: .medium))
            .foregroundStyle(ink)
            .lineLimit(1)
            .padding(.horizontal, size * 0.5)
            .frame(minWidth: size * 1.85, minHeight: size * 1.7)
            .background(shape.fill(fill))
            .overlay(shape.strokeBorder(edge, lineWidth: tint == nil ? 0.5 : 1))
            .fixedSize()
    }
}

/// The ⓘ button of a row: a popover that shows how the option behaves.
struct InfoButton<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content
    @State private var shown = false

    var body: some View {
        Button { shown.toggle() } label: {
            Image(systemName: "info.circle")
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.borderless)
        .help("\(title)的說明")
        .accessibilityLabel("\(title)的說明")
        .popover(isPresented: $shown, arrowEdge: .trailing) {
            InfoCard(title: title, content: content)
        }
    }
}

/// What an info popover shows; also rendered on its own for the UI previews.
struct InfoCard<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            content()
        }
        .padding(16)
        .frame(width: 340, alignment: .leading)
    }
}

// MARK: - Looping demos

/// Steps through frames, holding each for its own time, and loops.  Paused, it stays on `restingFrame`; with Reduce
/// Motion it starts paused.  `pinned` shows one frame and never moves (for the UI previews).
struct StoryClock: ViewModifier {
    @Binding var index: Int
    let holds: [Double]
    let playing: Bool

    func body(content: Content) -> some View {
        content.task(id: TaskKey(playing: playing, count: holds.count)) {
            guard playing, !holds.isEmpty else { return }
            while !Task.isCancelled {
                let hold = holds[min(index, holds.count - 1)]
                try? await Task.sleep(nanoseconds: UInt64(hold * 1_000_000_000))
                if Task.isCancelled { break }
                index = (index + 1) % holds.count
            }
        }
    }
    private struct TaskKey: Equatable { var playing: Bool; var count: Int }
}

/// Play and pause for a looping demo.
struct PlayPauseButton: View {
    @Binding var playing: Bool
    var body: some View {
        Button { playing.toggle() } label: {
            Image(systemName: playing ? "pause.fill" : "play.fill")
                .font(.system(size: 10, weight: .semibold))
                .frame(width: 22, height: 22)
                .background(Circle().fill(.quaternary.opacity(0.7)))
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.plain)
        .help(playing ? "暫停" : "播放")
        .accessibilityLabel(playing ? "暫停示範" : "播放示範")
    }
}

/// A run of composed text: a word, or Zhuyin still being typed.
struct DemoSegment: Equatable {
    enum Style: Equatable {
        /// Committed text in the document.
        case document
        /// Composed text: a thin underline per word.
        case marked
        /// The part the candidate window changes: a thick accent underline on an accent wash.
        case focus
        /// The character ↓ would change: a thick accent underline.
        case target
        /// What a demo points at, such as the text read before the cursor.
        case highlight
        /// Text the demo shows as no longer used.
        case faded
    }
    var text: String
    var style: Style
    init(_ text: String, _ style: Style = .marked) {
        self.text = text
        self.style = style
    }
}

/// One line of a text document: committed text, the composition with its underlines, and the caret after the
/// segment at `caret` (nil: after everything).
struct DemoLine: View {
    let segments: [DemoSegment]
    var caret: Int?
    var fontSize: CGFloat = 15
    var showsCaret = true
    var caretBlinks = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            ForEach(Array(segments.enumerated()), id: \.offset) { index, segment in
                segmentView(segment)
                    .padding(.trailing, gap(after: index))
                if showsCaret, caret == index { DemoCaret(height: fontSize * 1.25, blinks: caretBlinks && !reduceMotion) }
            }
            if showsCaret, caret == nil { DemoCaret(height: fontSize * 1.25, blinks: caretBlinks && !reduceMotion) }
        }
        .font(.system(size: fontSize))
        .lineLimit(1)
    }

    /// Words of a composition are underlined separately, with a small gap between them.
    private func gap(after index: Int) -> CGFloat {
        guard index + 1 < segments.count else { return 0 }
        let composed: Set<DemoSegment.Style> = [.marked, .focus, .target]
        return composed.contains(segments[index].style) && composed.contains(segments[index + 1].style) ? 1.5 : 0
    }

    @ViewBuilder private func segmentView(_ segment: DemoSegment) -> some View {
        let text = Text(segment.text.replacingOccurrences(of: " ", with: "\u{2002}"))
        switch segment.style {
        case .document:
            text.foregroundStyle(.primary)
        case .faded:
            text.foregroundStyle(.tertiary)
        case .highlight:
            text.foregroundStyle(.primary)
                .background(RoundedRectangle(cornerRadius: 3).fill(Color.accentColor.opacity(0.22)).padding(.vertical, -1))
        case .marked, .focus, .target:
            text.foregroundStyle(.primary)
                .background(segment.style == .focus ? Color.accentColor.opacity(0.18) : .clear)
                .overlay(alignment: .bottom) {
                    Rectangle()
                        .fill(segment.style == .marked ? AnyShapeStyle(.primary.opacity(0.75)) : AnyShapeStyle(Color.accentColor))
                        .frame(height: segment.style == .marked ? 1 : 2.5)
                        .offset(y: 2)
                }
        }
    }
}

struct DemoCaret: View {
    let height: CGFloat
    let blinks: Bool
    var body: some View {
        if blinks {
            TimelineView(.periodic(from: .now, by: 0.53)) { context in
                let on = Int(context.date.timeIntervalSinceReferenceDate / 0.53) % 2 == 0
                bar.opacity(on ? 1 : 0)
            }
        } else {
            bar
        }
    }
    private var bar: some View {
        Rectangle().fill(.primary).frame(width: 1.5, height: height).padding(.horizontal, 0.5)
            .alignmentGuide(.firstTextBaseline) { $0[.bottom] - height * 0.22 }
    }
}

/// A frame of a small demo: the key just pressed, the line after it, and a few words under it.
struct DemoFrame {
    var keys: [String] = []
    var line: [DemoSegment]
    var caret: Int?
    /// A small candidate list under the line.
    var list: [String]?
    var caption: String
    var hold: Double = 0.9
}

/// A text field that plays `frames` in a loop, with the key pressed and a caption for each step.
struct MiniDemo: View {
    let frames: [DemoFrame]
    /// The frame shown while paused (and under Reduce Motion before playing).
    var restingFrame: Int?
    var pinned: Int?
    @State private var index = 0
    @State private var playing = true
    @State private var started = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var current: DemoFrame { frames[min(pinned ?? index, frames.count - 1)] }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: 7)
                    .fill(Color(nsColor: .textBackgroundColor))
                    .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(.quaternary, lineWidth: 1))
                VStack(alignment: .leading, spacing: 4) {
                    DemoLine(segments: current.line, caret: current.caret, caretBlinks: pinned == nil)
                    if let list = current.list { MiniList(items: list) }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
            }
            .frame(height: frames.contains { $0.list != nil } ? 112 : 38)
            HStack(spacing: 8) {
                if pinned == nil { PlayPauseButton(playing: $playing) }
                ZStack(alignment: .leading) {
                    // Reserve the widest keys so the caption does not move.
                    KeyCombo(keys: current.keys.isEmpty ? [" "] : current.keys, size: 11, highlighted: true)
                        .opacity(current.keys.isEmpty ? 0 : 1)
                }
                Text(current.caption)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
        }
        .modifier(StoryClock(index: $index, holds: frames.map(\.hold), playing: playing && pinned == nil))
        .onAppear {
            guard !started else { return }
            started = true
            if reduceMotion {
                playing = false
                index = restingFrame ?? frames.count - 1
            }
        }
        .onChange(of: playing) { _, now in
            if !now, let restingFrame { index = restingFrame }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(frames.map(\.caption).joined(separator: "，"))
    }
}

/// A few candidates under a demo line, in the candidate window's style.  "今 ㄐㄧㄣ" is a candidate with its
/// corrected reading.
struct MiniList: View {
    let items: [String]
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                let parts = item.split(separator: " ", maxSplits: 1).map(String.init)
                let selected = index == 0
                HStack(spacing: 6) {
                    Text("\(index + 1)").font(.system(size: 10, weight: .medium)).monospacedDigit()
                        .foregroundStyle(selected ? AnyShapeStyle(Color.white.opacity(0.85)) : AnyShapeStyle(.secondary))
                    Text(parts[0]).font(.system(size: 13))
                        .foregroundStyle(selected ? AnyShapeStyle(Color.white) : AnyShapeStyle(.primary))
                    if parts.count > 1 {
                        Text(parts[1]).font(.system(size: 10))
                            .foregroundStyle(selected ? AnyShapeStyle(Color.white.opacity(0.8)) : AnyShapeStyle(.tertiary))
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 6)
                .frame(height: 20)
                .background(RoundedRectangle(cornerRadius: 5).fill(selected ? Color.accentColor : .clear))
            }
        }
        .padding(4)
        .frame(minWidth: 92)
        .fixedSize()
        .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial)
            .shadow(color: .black.opacity(0.18), radius: 4, y: 1))
        .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(.quaternary, lineWidth: 0.5))
    }
}

/// One choice of an option with its demo, marked when it is the current setting.
struct ChoiceDemo: View {
    let title: String
    let selected: Bool
    let frames: [DemoFrame]
    var pinned: Int?
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Text(title).font(.subheadline.weight(.semibold))
                if selected {
                    Text("目前的設定")
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(Capsule().fill(Color.accentColor.opacity(0.18)))
                        .foregroundStyle(Color.accentColor)
                }
            }
            MiniDemo(frames: frames, pinned: pinned)
        }
    }
}

/// A short fact row inside an info card: a symbol, then one sentence.
struct FactRow: View {
    let symbol: String
    let text: String
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: symbol)
                .foregroundStyle(Color.accentColor)
                .frame(width: 18)
            Text(text)
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(.callout)
    }
}

// MARK: - The keys in use

/// Every key the settings pages name, read from the current settings: the rebindable shortcuts, the features turned
/// off, the selection keys, the Shift toggle.  Nothing on the pages spells a configurable key itself, and a key whose
/// feature is off is not shown.
struct CurrentKeys {
    let shortcuts: [ShortcutCommand: Shortcut]
    let disabled: Set<KeyFeature>
    let pageKeys: [String]
    let shiftToggle: ShiftToggle
    let selectionKeySet: SelectionKeySet

    init(_ preferences: Preferences) {
        shortcuts = preferences.shortcuts
        disabled = preferences.disabledKeyFeatures
        pageKeys = preferences.pageSelectionKeys.map { String($0).uppercased() }
        shiftToggle = preferences.shiftToggle
        selectionKeySet = preferences.selectionKeys
    }

    func isOn(_ feature: KeyFeature) -> Bool { !disabled.contains(feature) }
    func isOn(_ command: ShortcutCommand) -> Bool { command.feature.map(isOn) ?? true }

    /// The command's keys as separate keycaps (["⇧", "↑"]), whether or not it is on.
    func assigned(_ command: ShortcutCommand) -> [String] { (shortcuts[command] ?? command.defaultShortcut).keycaps }
    /// The command's keycaps, or nil when it is turned off.
    func caps(_ command: ShortcutCommand) -> [String]? { isOn(command) ? assigned(command) : nil }
    /// The command's keys in one piece ("⇧↑"), or nil when it is turned off.
    func text(_ command: ShortcutCommand) -> String? { caps(command)?.joined() }

    /// The keys that pick on a page: "1–9", or "A S D F G" for the home row.
    var selection: String {
        selectionKeySet == .digits ? "\(pageKeys.first ?? "1")–\(pageKeys.last ?? "9")" : pageKeys.joined(separator: " ")
    }

    /// The Shift that switches 中/英, or nil when Shift does not switch.
    var shiftTap: String? {
        switch shiftToggle {
        case .both: return "Shift"
        case .left: return "左 Shift"
        case .right: return "右 Shift"
        case .none: return nil
        }
    }

    /// The key that shows the 中/英 badge: the Shift tap, or Caps Lock; nil when neither is on.
    var modeKey: String? { shiftTap ?? (isOn(.capsLockEnglish) ? "Caps Lock" : nil) }
}

/// A keycap beside the text, as if the key had just been pressed: it settles in with a short press, and presses again
/// whenever its keys change.  Under Reduce Motion it only appears.
struct PressedKeycap: View {
    let keys: [String]
    /// Small text after the keys, such as "×3".
    var note: String?
    /// False for the UI previews, which draw it at rest (or at `frame`, a moment of the press).
    var animated = true
    var frame: PressFrame = .rest
    /// Changes when the same keys are pressed again (a scene of a story), to press once more.
    var trigger = 0
    @State private var pressed = false
    @State private var shown = false

    /// Moments of the press, for the UI previews.
    enum PressFrame { case appearing, down, rest }
    private var isPressed: Bool { animated ? pressed : frame == .down }
    private var isShown: Bool { animated ? shown : frame != .appearing }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 3) {
            ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
                Text(key)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.primary.opacity(0.85))
                    .padding(.horizontal, 5)
                    .frame(minWidth: 21, minHeight: 21)
                    .background(
                        RoundedRectangle(cornerRadius: 5, style: .continuous)
                            .fill(Color(nsColor: .controlBackgroundColor))
                            .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous).fill(.primary.opacity(0.07)))
                            .shadow(color: .black.opacity(isPressed ? 0.10 : 0.28), radius: isPressed ? 0.3 : 0.6,
                                    y: isPressed ? 0.3 : 1.6)
                    )
                    .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .strokeBorder(.primary.opacity(0.18), lineWidth: 0.5))
                    .offset(y: isPressed ? 1.2 : 0)
            }
            if let note {
                Text(note).font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary)
            }
        }
        .fixedSize()
        .opacity(isShown ? 1 : (animated ? 0 : 0.45))
        .scaleEffect(isShown || reduceMotion ? 1 : 0.9)
        .onAppear(perform: press)
        .onChange(of: keys) { _, _ in press() }
        .onChange(of: trigger) { _, _ in press() }
        .accessibilityLabel(keys.joined(separator: " "))
    }

    private func press() {
        guard animated else { return }
        guard !reduceMotion else {
            withAnimation(.easeOut(duration: 0.2)) { shown = true }
            return
        }
        withAnimation(.easeOut(duration: 0.16)) { shown = true }
        withAnimation(.easeIn(duration: 0.07).delay(0.12)) { pressed = true }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.62).delay(0.21)) { pressed = false }
    }
}
