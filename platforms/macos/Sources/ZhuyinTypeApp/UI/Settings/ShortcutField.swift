import AppKit
import SwiftUI

// The one view for a command's keys, wherever they are shown or set: its bindings as keycaps, 「A 或 B」; click one
// to record a new combination (Esc cancels), + to add another, and on hover a control that restores the defaults.
// A binding another command also uses is drawn in red with the other command's name.  Recording pulses gently, keys
// appear as they are pressed, an accepted combination settles in and a refused one shakes (cross-fades under Reduce
// Motion).  Two-key sequences are not supported: a binding is one key with modifiers.

/// A state to draw without interaction, for the UI previews.
enum ShortcutFieldPreview: Equatable {
    case hover
    /// Recording binding `index`, with these modifiers held so far.
    case recording(index: Int, held: [String])
    case refused(String)
    case accepted(index: Int)
}

struct ShortcutField: View {
    @ObservedObject var preferences: Preferences
    let command: ShortcutCommand
    /// The command's feature is off: drawn dimmed, not editable.
    var disabled = false
    var preview: ShortcutFieldPreview?

    @State private var recording: Int?
    @State private var held: [String] = []
    @State private var problem: String?
    @State private var hovered = false
    @State private var settled: Int?
    @State private var shake: CGFloat = 0
    @State private var pulse = false
    @State private var monitor: Any?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let maximumBindings = 3

    var body: some View {
        let bindings = preferences.bindings[command] ?? []
        let conflicts = preferences.conflicts(of: command)
        let recordingIndex = recording ?? previewRecording
        // Hover affordances are always laid out, so hover never changes the layout; only their opacity changes.
        let showsControls = (hovered || preview == .hover) && !disabled && recordingIndex == nil
        VStack(alignment: .trailing, spacing: 4) {
            TrailingFlow(spacing: 5, lineSpacing: 5) {
                // ＋ comes first, so its reserved place is on the free side and the keycaps end on the row's right edge.
                if bindings.count < Self.maximumBindings, !disabled {
                    iconButton("plus", help: "加入另一個按鍵") { start(bindings.count) }
                        .opacity(showsControls ? 1 : 0)
                        .allowsHitTesting(showsControls)
                }
                ForEach(Array(bindings.enumerated()), id: \.offset) { index, shortcut in
                    if index > 0 { Text("或").font(.callout).foregroundStyle(.tertiary) }
                    if recordingIndex == index {
                        recorder
                    } else {
                        bindingButton(index: index, shortcut: shortcut, conflict: conflicts[index], count: bindings.count)
                    }
                }
                if let recordingIndex, recordingIndex >= bindings.count {
                    Text("或").font(.callout).foregroundStyle(.tertiary)
                    recorder
                }
            }
            .offset(x: shake)
            ForEach(Array(conflicts.keys.sorted()), id: \.self) { index in
                if let other = conflicts[index], recordingIndex == nil {
                    ConflictLine(other: other) { start(index) }
                }
            }
            if let message = problem ?? previewProblem {
                Text(message)
                    .font(.callout)
                    .foregroundStyle(Color(nsColor: .systemRed))
                    .multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
                    .transition(.opacity)
            }
        }
        .opacity(disabled ? 0.45 : 1)
        .allowsHitTesting(!disabled)
        .onHover { hovered = $0 }
        .onDisappear(perform: stop)
        .animation(.easeInOut(duration: 0.15), value: hovered)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(command.label)：" + bindings.map(\.spoken).joined(separator: " 或 "))
    }

    private var previewRecording: Int? {
        if case .recording(let index, _) = preview { return index }
        return nil
    }
    private var previewProblem: String? {
        if case .refused(let message) = preview { return message }
        return nil
    }

    private func bindingButton(index: Int, shortcut: Shortcut, conflict: ShortcutCommand?, count: Int) -> some View {
        let accepted = settled == index || preview == .accepted(index: index)
        return Button { start(index) } label: {
            KeyCombo(keys: shortcut.keycaps, tint: conflict == nil ? nil : Color(nsColor: .systemRed))
                .scaleEffect(accepted && !reduceMotion && preview == nil ? 1.06 : 1)
                .overlay(alignment: .topTrailing) {
                    if accepted {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(.white, Color(nsColor: .systemGreen))
                            .offset(x: 5, y: -5)
                            .transition(.opacity)
                    }
                }
        }
        .buttonStyle(.plain)
        .help(shortcut.spoken + "：" + (conflict.map { "也用於「\($0.label)」；點一下改按鍵" } ?? "點一下，再按下新的組合"))
        .contextMenu {
            Button("改按鍵…") { start(index) }
            if count > 1 { Button("移除這個按鍵") { preferences.unbind(command, at: index) } }
            Button("全部還原成預設值") { problem = preferences.resetBindings(command) }
        }
        .accessibilityLabel(shortcut.spoken + (conflict.map { "，也用於「\($0.label)」" } ?? ""))
    }

    /// The binding being recorded: a gently pulsing outline, the modifiers held so far, or the prompt.
    private var recorder: some View {
        let keys: [String] = {
            if case .recording(_, let held) = preview { return held }
            return held
        }()
        return HStack(spacing: 3) {
            if keys.isEmpty {
                Text("按下新的組合…")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(Color.accentColor)
                    .padding(.horizontal, 4)
            } else {
                KeyCombo(keys: keys + ["…"])
            }
        }
        .padding(.horizontal, 4)
        .frame(height: Keycap.height(12))
        .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Color.accentColor.opacity(0.08)))
        .overlay(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(Color.accentColor.opacity(pulse || reduceMotion || preview != nil ? 0.9 : 0.35), lineWidth: 1.5)
        )
        .onAppear {
            guard !reduceMotion, preview == nil else { return }
            withAnimation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true)) { pulse = true }
        }
        .onDisappear { pulse = false }
        .accessibilityLabel("按下新的組合")
    }

    private func iconButton(_ symbol: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(.secondary)
                .frame(width: 20, height: 20)
                .background(Circle().fill(.quaternary.opacity(0.6)))
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help)
        .transition(.opacity)
    }

    // MARK: Recording

    private func start(_ index: Int) {
        stop()
        problem = nil
        held = []
        withAnimation(.easeOut(duration: 0.15)) { recording = index }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { event in
            switch Self.record(event, command: command, at: index, in: preferences) {
            case .held(let keys): held = keys; return event
            case .cancel: stop()
            case .refuse(let message): refuse(message)
            case .accept: stop(); accept(index)
            }
            return nil
        }
    }

    enum Recorded: Equatable {
        /// Modifiers held so far.
        case held([String])
        case cancel
        case refuse(String)
        case accept
    }

    /// What a key event does while recording binding `index` of `command`: the recorder's logic, apart from the view
    /// so the self-tests can feed it events.  fn is shown while held; fn with an arrow or ⌫ is Page Up / Down, Home /
    /// End or ⌦ (the event's .function and .numericPad flags are not modifiers of the shortcut).
    static func record(_ event: NSEvent, command: ShortcutCommand, at index: Int, in preferences: Preferences) -> Recorded {
        if event.type == .flagsChanged { return .held(modifierCaps(event.modifierFlags)) }
        let flags = event.modifierFlags.intersection([.control, .option, .shift, .command])
        if event.keyCode == 53 && flags.isEmpty { return .cancel }
        guard let shortcut = Shortcut(keyCode: event.keyCode, modifiers: event.modifierFlags) else { return .refuse("這個鍵不能用") }
        if let message = preferences.bind(command, at: index, to: shortcut) { return .refuse(message) }
        return .accept
    }

    private func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        withAnimation(.easeOut(duration: 0.15)) { recording = nil }
        held = []
    }

    private func accept(_ index: Int) {
        withAnimation(reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.3, dampingFraction: 0.55)) { settled = index }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            withAnimation(.easeOut(duration: 0.3)) { if settled == index { settled = nil } }
        }
    }

    /// A refused combination: the message, and a short shake (a cross-fade under Reduce Motion).  Recording goes on.
    private func refuse(_ message: String) {
        held = []
        withAnimation(.easeInOut(duration: 0.15)) { problem = message }
        guard !reduceMotion else { return }
        let steps: [CGFloat] = [-6, 5, -3, 2, 0]
        for (offset, value) in steps.enumerated() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05 * Double(offset)) {
                withAnimation(.linear(duration: 0.05)) { shake = value }
            }
        }
    }

    static func modifierCaps(_ flags: NSEvent.ModifierFlags) -> [String] {
        [(NSEvent.ModifierFlags.control, "⌃"), (.option, "⌥"), (.shift, "⇧"), (.command, "⌘"), (.function, "fn")]
            .filter { flags.contains($0.0) }.map(\.1)
    }
}

/// A binding another command also uses: one red line with the way out.
private struct ConflictLine: View {
    let other: ShortcutCommand
    let replace: () -> Void
    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "exclamationmark.triangle.fill").font(.caption)
            Text("也用於「\(other.label)」").lineLimit(1).truncationMode(.tail)
            HStack(spacing: 10) {
                Button("改按鍵", action: replace).buttonStyle(.link)
                Button("前往") { SettingsWindowController.shared.open(other.place) }.buttonStyle(.link)
            }
            .padding(.leading, 4)
        }
        .font(.callout)
        .foregroundStyle(Color(nsColor: .systemRed))
        .help("「\(other.label)」也用這個按鍵；兩個功能都按下時只有一個會作用")
    }
}

/// Lays its children out right to left on lines, wrapping onto a new line when the width runs out, each line aligned
/// to the trailing edge, so many bindings never overflow a row.
struct TrailingFlow: Layout {
    var spacing: CGFloat = 6
    var lineSpacing: CGFloat = 6
    /// Lines start at the leading edge instead.
    var leading = false

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let lines = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let width = lines.map(\.width).max() ?? 0
        let height = lines.map(\.height).reduce(0, +) + CGFloat(max(0, lines.count - 1)) * lineSpacing
        return CGSize(width: min(width, proposal.width ?? width), height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for line in arrange(width: bounds.width, subviews: subviews) {
            var x = leading ? bounds.minX : bounds.maxX - line.width
            for index in line.items {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y + (line.height - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += line.height + lineSpacing
        }
    }

    private struct Line { var items: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Line] {
        var lines: [Line] = [Line()]
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let needed = lines[lines.count - 1].items.isEmpty ? size.width : lines[lines.count - 1].width + spacing + size.width
            if needed > width, !lines[lines.count - 1].items.isEmpty {
                lines.append(Line())
            }
            var line = lines[lines.count - 1]
            line.width = line.items.isEmpty ? size.width : line.width + spacing + size.width
            line.height = max(line.height, size.height)
            line.items.append(index)
            lines[lines.count - 1] = line
        }
        return lines.filter { !$0.items.isEmpty }
    }
}

extension Preferences {
    /// Bindings of `command` that another command also uses, by index; only between commands that are on.
    func conflicts(of command: ShortcutCommand) -> [Int: ShortcutCommand] {
        guard isEnabled(command) else { return [:] }
        let all = bindings
        var result: [Int: ShortcutCommand] = [:]
        for (index, shortcut) in (all[command] ?? []).enumerated() {
            if let other = all.first(where: { $0.key != command && isEnabled($0.key) && $0.value.contains(shortcut) })?.key {
                result[index] = other
            }
        }
        return result
    }

    /// Every conflicting pair, once each, for the overview.
    var allConflicts: [(ShortcutCommand, ShortcutCommand, Shortcut)] {
        var seen = Set<String>()
        var result: [(ShortcutCommand, ShortcutCommand, Shortcut)] = []
        for command in ShortcutCommand.allCases {
            for (index, other) in conflicts(of: command).sorted(by: { $0.key < $1.key }) {
                let key = [command.rawValue, other.rawValue].sorted().joined()
                guard seen.insert(key).inserted, let shortcut = bindings[command]?[index] else { continue }
                result.append((command, other, shortcut))
            }
        }
        return result
    }
}

extension ShortcutCommand {
    /// The section of the 快捷鍵 page the command is listed in.
    var section: KeySection {
        switch self {
        case .commitWithNewline, .deleteWord: return .composing
        case .nextPage, .previousPage, .singleCharacterList, .correctionsList: return .list
        case .typeZhuyin, .selectLeft, .selectRight: return .editing
        }
    }
}
