import AppKit
import SwiftUI

// The one view for a command's keys, wherever they are shown or set.  Each binding is one chip with its modifiers
// inside (「⌃]」「⇧Tab」「fn↓」); alternatives are just more chips, no 「或」 (VoiceOver and tooltips still say it).  At rest
// nothing else shows.  With the pointer on the row: a ghost ＋ chip to add a key, 還原預設 beside the field when the keys
// differ from the default; on a chip: its ✕, inside its trailing edge.  Click a chip to record a new combination (Esc
// cancels); the chip becomes an accent outline with the modifiers held so far and a caret.  An accepted key settles with
// a small bounce, a refused one shakes and a red line slides in below.  A binding another command also uses is drawn in
// red with the other command's name.  Keyboard and VoiceOver: chips take focus, ⌫ deletes one, the context menu has
// 改按鍵, 新增按鍵, 刪除 and 還原預設.  Two-key sequences are not supported: a binding is one key with modifiers.

/// A state to draw without interaction, for the UI previews.
enum ShortcutFieldPreview: Equatable {
    /// The pointer on the row.
    case hover
    /// The pointer on binding `index`'s chip.
    case chipHover(Int)
    /// The overview's +N opened.
    case expanded
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
    /// The hover id of the row the field is in, so its controls show wherever the pointer is on the row.
    var row: AnyHashable?
    /// At most this many chips, then 「+N」 (the overview); nil shows every binding, wrapping.
    var maxVisible: Int?
    var preview: ShortcutFieldPreview?

    @State private var recording: Int?
    @State private var held: [String] = []
    @State private var problem: String?
    @State private var settled: Int?
    @State private var shake: CGFloat = 0
    @State private var monitor: Any?
    @State private var expanded = false
    @ObservedObject private var hover = HoverState.shared
    @FocusState private var focusedChip: Int?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let maximumBindings = 5

    private var rowID: AnyHashable { row ?? AnyHashable("keys-" + command.rawValue) }
    private func chipID(_ index: Int) -> AnyHashable { AnyHashable("chip-\(command.rawValue)-\(index)") }
    private var rowHovered: Bool {
        switch preview {
        case .hover?, .expanded?, .chipHover?: return true
        default: return hover.hovered == rowID
        }
    }
    private func chipHovered(_ index: Int) -> Bool {
        if case .chipHover(let hovered)? = preview { return hovered == index }
        return hover.detail == chipID(index)
    }

    var body: some View {
        let bindings = preferences.bindings[command] ?? []
        let conflicts = preferences.conflicts(of: command)
        let recordingIndex = recording ?? previewRecording
        let editing = rowHovered && !disabled && recordingIndex == nil
        VStack(alignment: .trailing, spacing: 4) {
            chips(bindings, conflicts, recordingIndex, editing).offset(x: shake)
            notes(conflicts, recordingIndex)
        }
        .overlay(alignment: .leading) { restore(editing) }
        .opacity(disabled ? 0.45 : 1)
        .allowsHitTesting(!disabled)
        .modifier(OwnHover(id: rowID, tracks: row == nil))
        .onChange(of: rowHovered) { _, now in collapse(now) }
        .onDisappear(perform: stop)
        .animation(.easeOut(duration: 0.12), value: editing)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(spokenLabel(bindings))
    }

    private func spokenLabel(_ bindings: [Shortcut]) -> String {
        let keys: String = bindings.isEmpty ? "沒有按鍵" : bindings.map(\.spoken).joined(separator: " 或 ")
        return "\(command.label)：\(keys)"
    }

    private func collapse(_ hovered: Bool) {
        guard !hovered, expanded else { return }
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.2)) { expanded = false }
    }

    private func visibleCount(_ bindings: [Shortcut], _ recordingIndex: Int?) -> Int {
        let pinnedOpen: Bool = preview == .expanded
        let showsAll: Bool = maxVisible == nil || expanded || pinnedOpen || recordingIndex != nil
        return showsAll ? bindings.count : min(bindings.count, maxVisible ?? bindings.count)
    }

    @ViewBuilder
    private func chips(_ bindings: [Shortcut], _ conflicts: [Int: ShortcutCommand], _ recordingIndex: Int?,
                       _ editing: Bool) -> some View {
        let visible = visibleCount(bindings, recordingIndex)
        TrailingFlow(spacing: 6, lineSpacing: 6) {
            // The ghost ＋ keeps its place on the free side, so showing it moves nothing.
            if !bindings.isEmpty, bindings.count < Self.maximumBindings, !disabled {
                ghostAdd(editing, count: bindings.count)
            }
            if bindings.isEmpty, recordingIndex == nil { emptyState(editing) }
            ForEach(0..<visible, id: \.self) { index in
                chipOrRecorder(index, bindings[index], conflicts[index] != nil, recordingIndex, editing)
            }
            if visible < bindings.count { moreChip(bindings.count - visible) }
            if let recordingIndex, recordingIndex >= bindings.count { recorder }
        }
    }

    private func ghostAdd(_ editing: Bool, count: Int) -> some View {
        GhostChip(text: "＋") { start(count) }
            .help("新增按鍵")
            .opacity(editing ? 1 : 0)
            .allowsHitTesting(editing)
    }

    private func emptyState(_ editing: Bool) -> some View {
        ZStack(alignment: .trailing) {
            Text("—").foregroundStyle(.tertiary).opacity(editing ? 0 : 1)
                .frame(height: Keycap.height(12))
                .accessibilityLabel("沒有按鍵")
            GhostChip(text: "＋ 新增按鍵") { start(0) }
                .opacity(editing ? 1 : 0)
                .allowsHitTesting(editing)
        }
    }

    @ViewBuilder
    private func chipOrRecorder(_ index: Int, _ shortcut: Shortcut, _ conflict: Bool, _ recordingIndex: Int?,
                                _ editing: Bool) -> some View {
        if recordingIndex == index { recorder } else { chip(index, shortcut, conflict: conflict, editing: editing) }
    }

    private func moreChip(_ count: Int) -> some View {
        MoreChip(count: count) {
            withAnimation(reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.85)) { expanded = true }
        }
    }

    @ViewBuilder
    private func notes(_ conflicts: [Int: ShortcutCommand], _ recordingIndex: Int?) -> some View {
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
                .transition(.move(edge: .top).combined(with: .opacity))
        }
    }

    /// 還原預設 on hover, beside the field on its free side: an overlay, so it takes no room.
    @ViewBuilder
    private func restore(_ editing: Bool) -> some View {
        if preferences.isCustomized(command), !disabled {
            Button("還原預設") { problem = preferences.resetBindings(command) }
                .buttonStyle(.link)
                .font(.callout)
                .fixedSize()
                .opacity(editing ? 1 : 0)
                .allowsHitTesting(editing)
                .help("還原成 " + command.defaultBindings.map(\.spoken).joined(separator: " 或 "))
                // Left of the field, outside its layout: a zero-width frame it overflows towards the leading side.
                .frame(width: 0, alignment: .trailing)
                .offset(x: -10)
        }
    }

    private var previewRecording: Int? {
        if case .recording(let index, _) = preview { return index }
        return nil
    }
    private var previewProblem: String? {
        if case .refused(let message) = preview { return message }
        return nil
    }

    private func chip(_ index: Int, _ shortcut: Shortcut, conflict: Bool, editing: Bool) -> some View {
        let removing = editing && chipHovered(index)
        let accepted: Bool = settled == index || preview == .accepted(index: index)
        let bounce: Bool = accepted && !reduceMotion && preview == nil
        let face = Button { start(index) } label: {
            KeyChip(text: shortcut.compact, tint: conflict ? Color(nsColor: .systemRed) : nil, removing: removing,
                    focused: focusedChip == index)
        }
        .buttonStyle(.plain)
        .overlay(alignment: .trailing) { removeButton(index, shortcut, removing) }
        .scaleEffect(bounce ? 1.08 : 1)
        return keyboard(face, index)
            .hoverTrackedDetail(chipID(index))
            .help(chipHelp(shortcut, conflict))
            .contextMenu { chipMenu(index) }
            .accessibilityLabel(shortcut.spoken)
    }

    private func chipHelp(_ shortcut: Shortcut, _ conflict: Bool) -> String {
        let tail: String = conflict ? "：也用於另一個功能；點一下改按鍵" : "：點一下改按鍵"
        return shortcut.spoken + tail
    }

    private func keyboard(_ view: some View, _ index: Int) -> some View {
        view
            .focusable()
            .focusEffectDisabled()
            .focused($focusedChip, equals: index)
            .onKeyPress(keys: [.delete, .deleteForward]) { _ in remove(index); return .handled }
            .onKeyPress(.return) { start(index); return .handled }
    }

    private func removeButton(_ index: Int, _ shortcut: Shortcut, _ removing: Bool) -> some View {
        Button { remove(index) } label: {
            Image(systemName: "xmark")
                .font(.system(size: 8, weight: .bold))
                .foregroundStyle(.secondary)
                .frame(width: 14, height: Keycap.height(12))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.trailing, 2)
        .opacity(removing ? 1 : 0)
        .allowsHitTesting(removing)
        .help("刪除 " + shortcut.spoken)
        .accessibilityLabel("刪除 " + shortcut.spoken)
    }

    @ViewBuilder
    private func chipMenu(_ index: Int) -> some View {
        let count = preferences.bindings[command]?.count ?? 0
        Button("改按鍵…") { start(index) }
        Button("新增按鍵") { start(count) }.disabled(count >= Self.maximumBindings)
        Button("刪除") { remove(index) }
        if preferences.isCustomized(command) { Button("還原預設") { problem = preferences.resetBindings(command) } }
    }

    /// The binding being recorded: an accent outline with the modifiers held so far and a caret.
    private var recorder: some View {
        let keys: [String] = {
            if case .recording(_, let held) = preview { return held }
            return held
        }()
        return HStack(spacing: 1) {
            Text(keys.joined())
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color.accentColor)
            RecordingCaret(still: reduceMotion || preview != nil)
        }
        .padding(.horizontal, 7)
        .frame(minWidth: 30)
        .frame(height: Keycap.height(12))
        .background(RoundedRectangle(cornerRadius: 12 * 0.42, style: .continuous).fill(Color.accentColor.opacity(0.08)))
        .overlay(RoundedRectangle(cornerRadius: 12 * 0.42, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 1.5))
        .help("按下新的組合；Esc 取消")
        .accessibilityLabel("按下新的組合")
    }

    private func remove(_ index: Int) {
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.18)) {
            if let message = preferences.unbind(command, at: index) { refuse(message) } else { problem = nil }
        }
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
        withAnimation(.easeOut(duration: 0.18)) { problem = message }
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
        case .nextPage, .previousPage, .singleCharacterList, .correctionsList, .previousCharacter, .nextCharacter: return .list
        case .typeZhuyin, .selectLeft, .selectRight: return .editing
        }
    }
}

/// One binding: a single keycap with its modifiers inside.  While it can be removed (the pointer on it), the end of its
/// text fades under the ✕ drawn over it, so the chip keeps its size.
struct KeyChip: View {
    let text: String
    var tint: Color?
    var removing = false
    var focused = false
    var body: some View {
        Keycap(key: text, size: 12, tint: tint)
            .mask {
                HStack(spacing: 0) {
                    Rectangle()
                    LinearGradient(colors: [Color.black, Color.black.opacity(removing ? 0.08 : 1.0)], startPoint: .leading,
                                   endPoint: .trailing)
                        .frame(width: 16)
                }
            }
            .overlay(RoundedRectangle(cornerRadius: 12 * 0.42, style: .continuous)
                .strokeBorder(Color.accentColor.opacity(focused ? 0.9 : 0), lineWidth: 1.5))
    }
}

/// Add a key: a faint dashed chip.
private struct GhostChip: View {
    let text: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Text(text)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 6)
                .frame(minWidth: Keycap.height(12))
                .frame(height: Keycap.height(12))
                .overlay(RoundedRectangle(cornerRadius: 12 * 0.42, style: .continuous)
                    .strokeBorder(Color.secondary.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: [3, 2])))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(text.replacingOccurrences(of: "＋", with: "新增按鍵"))
    }
}

/// 「+N」: more keys than the overview shows; pointing at it or clicking opens them.
private struct MoreChip: View {
    let count: Int
    let open: () -> Void
    var body: some View {
        Button(action: open) {
            Text("+\(count)")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 6)
                .frame(height: Keycap.height(12))
                .background(RoundedRectangle(cornerRadius: 12 * 0.42, style: .continuous).fill(.quaternary.opacity(0.35)))
        }
        .buttonStyle(.plain)
        .onHover { if $0 { open() } }
        .help("還有 \(count) 個按鍵")
        .accessibilityLabel("還有 \(count) 個按鍵")
    }
}

/// The recorder's caret: a thin bar that breathes (still under Reduce Motion and in previews).
private struct RecordingCaret: View {
    let still: Bool
    @State private var dim = false
    var body: some View {
        RoundedRectangle(cornerRadius: 0.75)
            .fill(Color.accentColor)
            .frame(width: 1.5, height: 12)
            .opacity(dim ? 0.25 : 1)
            .onAppear {
                guard !still else { return }
                withAnimation(.easeInOut(duration: 0.55).repeatForever(autoreverses: true)) { dim = true }
            }
    }
}

/// Tracks the pointer for a field that is not in a row tracking it.
private struct OwnHover: ViewModifier {
    let id: AnyHashable
    let tracks: Bool
    func body(content: Content) -> some View {
        if tracks { content.hoverTracked(id) } else { content }
    }
}
