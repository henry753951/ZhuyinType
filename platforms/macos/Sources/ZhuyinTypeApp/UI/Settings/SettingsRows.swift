import AppKit
import SwiftUI

// The one settings row.  Every row of every page uses it, so rows cannot drift apart:
//
// - leading: the title, an optional description that wraps over as many lines as it needs, an optional link;
// - trailing, in this order: a value, the control, then the accessory column (ⓘ or ›), which is always reserved so
//   every control of a page ends on the same right edge;
// - the trailing block is centred vertically on the whole leading block;
// - one minimum height and one vertical padding; sub-rows (the 快捷鍵 row under a feature) indent the leading block by
//   a fixed amount and keep the same right edge;
// - hover never changes the layout: hover affordances are always laid out and only change opacity.
//
// Controls come from the same few views (`RowSwitch`, `RowPicker`, `KeyCombo`, `ShortcutField`), so a switch, a
// picker, a keycap group, a chevron and an ⓘ are the same size wherever they appear.

/// Spacing and insets of the settings pages, in one place.  Every page, card and row uses these; nothing pads itself.
enum RowMetrics {
    // The page.
    /// Left and right of the cards, and of the sub-page header.
    static let pageInset: CGFloat = 20
    static let pageTop: CGFloat = 16
    static let pageBottom: CGFloat = 24
    /// The cards' widest; wider windows centre the column.
    static let maxWidth: CGFloat = 700
    /// Between one section's card (or footer) and the next section's header.
    static let sectionSpacing: CGFloat = 22
    // A section.
    /// Header to its card, and card to its footer.
    static let headerGap: CGFloat = 7
    static let footerGap: CGFloat = 7
    // A card.
    /// Left and right inside a card, for every row, divider, header and footer: one value, so both sides match.
    static let cardInset: CGFloat = 16
    /// Above and below every row inside a card.
    static let rowPadding: CGFloat = 10
    static let cardRadius: CGFloat = 10
    /// Between tiles (分析), which are small cards: their padding is the card inset all round.
    static let tileSpacing: CGFloat = 10
    static let tilePadding: CGFloat = 14
    // A row.
    static let minHeight: CGFloat = 24
    /// A sub-row's (快捷鍵 under its feature) left indent; its right edge is the parent's.
    static let indent: CGFloat = 18
    /// Between the left block and the right block.
    static let gap: CGFloat = 12
    /// Between the value, the control and the ⓘ or ›.
    static let trailingSpacing: CGFloat = 8
    /// Above and below the sub-page bar's breadcrumb.
    static let barPadding: CGFloat = 10
    /// The arrival tint, inset from the card's edges.
    static let tintInset: CGFloat = 4
}

/// What ends a row on the right, after its control.
enum RowAccessory {
    case none
    case chevron
    /// A › that goes somewhere, when the row itself does something else.
    case link(help: String, action: () -> Void)
    case info(title: String, content: AnyView)
}

/// The section a row is in, for the arrival cue.
private struct SettingsSectionKey: EnvironmentKey { static let defaultValue: String? = nil }
extension EnvironmentValues {
    var settingsSection: String? {
        get { self[SettingsSectionKey.self] }
        set { self[SettingsSectionKey.self] = newValue }
    }
}

/// Which section each anchored row is in, filled in as rows are drawn.
enum RowSections {
    static var map: [String: String] = [:]
    static func headerID(_ section: String) -> String { "section-" + section }
}

/// A sub-page's bar: the page it belongs to, its title, and going back.
struct SubpageHeaderInfo {
    let parent: String
    let title: String
    let back: () -> Void
}
private struct SubpageHeaderKey: EnvironmentKey { static let defaultValue: SubpageHeaderInfo? = nil }
extension EnvironmentValues {
    var subpageHeader: SubpageHeaderInfo? {
        get { self[SubpageHeaderKey.self] }
        set { self[SubpageHeaderKey.self] = newValue }
    }
}

/// A settings page: its sections in a column, scrolled, with the page insets.  On a sub-page the bar is pinned above:
/// the content starts below it, and scrolls behind it.
struct SettingsForm<Content: View>: View {
    @ViewBuilder let content: () -> Content
    @Environment(\.subpageHeader) private var header
    @State private var scrolled = false
    @State private var restTop: CGFloat?
    init(@ViewBuilder content: @escaping () -> Content) { self.content = content }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: RowMetrics.sectionSpacing) { content() }
                .frame(maxWidth: RowMetrics.maxWidth)
                .padding(.horizontal, RowMetrics.pageInset)
                .padding(.top, header == nil ? RowMetrics.pageTop : RowMetrics.pageTop / 2)
                .padding(.bottom, RowMetrics.pageBottom)
                .frame(maxWidth: .infinity)
                .background(alignment: .top) {
                    // Where the content's top is, against the scroll view: below its rest position means scrolled.
                    GeometryReader { proxy in
                        Color.clear.preference(key: ScrollTop.self, value: proxy.frame(in: .named("settingsPage")).minY)
                    }
                    .frame(height: 0)
                }
        }
        .coordinateSpace(name: "settingsPage")
        .onPreferenceChange(ScrollTop.self) { top in
            // The top at rest is where the content starts below the bar; anything above it has scrolled under.
            if restTop == nil || top > restTop! { restTop = top }
            let now = top < (restTop ?? top) - 0.5
            if now != scrolled { scrolled = now }
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            if let header { SubpageBar(info: header, scrolled: scrolled) }
        }
        .background(Color(nsColor: .textBackgroundColor))
    }
}

private struct ScrollTop: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

/// Pushing a sub-page in from the trailing side, or the page it leaves moving back a little; both fade.
struct NavigationMotion: ViewModifier, Animatable {
    let offset: CGFloat
    var visible: Double
    var animatableData: Double {
        get { visible }
        set { visible = newValue }
    }
    func body(content: Content) -> some View {
        content
            .offset(x: offset * CGFloat(1 - visible))
            .opacity(visible)
    }
}

/// A section of a settings page: its header (which lights up with the arrival cue), its card of rows and a footer.
struct SettingsSection<Content: View, Footer: View>: View {
    let title: String?
    /// False: the content brings its own cards (tiles) and sits on the page.
    var card = true
    @ViewBuilder let content: () -> Content
    @ViewBuilder let footer: () -> Footer

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let title {
                SettingsSectionHeader(title: title)
                    .padding(.horizontal, RowMetrics.cardInset)
                    .padding(.bottom, RowMetrics.headerGap)
            }
            if card {
                SettingsCard(section: title) { content().environment(\.settingsSection, title) }
            } else {
                VStack(alignment: .leading, spacing: RowMetrics.tileSpacing) { content() }
                    .environment(\.settingsSection, title)
            }
            if Footer.self != EmptyView.self {
                footer()
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, RowMetrics.cardInset)
                    .padding(.top, RowMetrics.footerGap)
            }
        }
    }
}

extension SettingsSection where Footer == EmptyView {
    init(_ title: String? = nil, card: Bool = true, @ViewBuilder content: @escaping () -> Content) {
        self.init(title: title, card: card, content: content, footer: { EmptyView() })
    }
}

extension SettingsSection {
    init(_ title: String? = nil, @ViewBuilder content: @escaping () -> Content, @ViewBuilder footer: @escaping () -> Footer) {
        self.init(title: title, card: true, content: content, footer: footer)
    }
}

/// A section's header: in the accent colour while the arrival cue is on the section or one of its rows.
struct SettingsSectionHeader: View {
    let title: String
    @ObservedObject var model: SettingsModel = SettingsWindowController.shared.model
    var body: some View {
        Text(title)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(model.arrivedSection == title ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.secondary))
            .id(RowSections.headerID(title))
    }
}

/// A view that spans the card edge to edge (a list), without the card's insets.
private struct FullBleed: _ViewTraitKey { static let defaultValue = false }
extension View {
    func cardFullBleed() -> some View { _trait(FullBleed.self, true) }
}

/// A section's rows on one card: the same inset on both sides of every row and divider, the same padding above and
/// below every row.  When a link leads to the whole section, the card tints once.
struct SettingsCard<Content: View>: View {
    let section: String?
    @ViewBuilder let content: () -> Content
    @ObservedObject var model: SettingsModel = SettingsWindowController.shared.model
    @State private var probe = UUID()

    var body: some View {
        let lit = section != nil && model.arrivedRow == nil && model.arrivedSection == section
        _VariadicView.Tree(CardRows(card: probe)) { content() }
            .background {
                ZStack {
                    RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous)
                        .fill(Color.primary.opacity(0.03))
                    RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous)
                        .fill(Color.accentColor.opacity(lit ? (DemoPinning.arrivalOpacity ?? ArrivalHighlight.peakOpacity) : 0))
                }
            }
            .background(CardProbe(card: probe))
    }
}

private struct CardRows: _VariadicView_MultiViewRoot {
    let card: UUID
    func body(children: _VariadicView.Children) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(children) { child in
                if child.id != children.first?.id {
                    Rectangle()
                        .fill(Color.primary.opacity(0.08))
                        .frame(height: 1)
                        .padding(.horizontal, RowMetrics.cardInset)
                }
                if child[FullBleed.self] {
                    child.frame(maxWidth: .infinity)
                } else {
                    child
                        .background(ChildProbe(card: card))
                        .padding(.horizontal, RowMetrics.cardInset)
                        .padding(.vertical, RowMetrics.rowPadding)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }
}

struct SettingRow<Control: View>: View {
    let title: String
    var subtitle: String?
    var value: String?
    var indent = false
    var dimmed = false
    var link: (title: String, place: SettingsPlace)?
    /// A small accent dot before the title: the setting differs from its default (「已自訂」 as its tooltip).
    var marked = false
    var accessory: RowAccessory = .none
    /// The row's id for links: links scroll to it and it shows the arrival tint.
    var anchor: String?
    @ViewBuilder var control: () -> Control
    @Environment(\.settingsSection) private var section
    /// This row's identity for the geometry check.
    @State private var probe = UUID()

    var body: some View {
        if let anchor, let section { let _ = RowSections.map[anchor] = section }
        HStack(alignment: .center, spacing: RowMetrics.gap) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .foregroundStyle(indent || dimmed ? .secondary : .primary)
                    .help(title)
                    .overlay(alignment: .leading) {
                        if marked {
                            // In the margin, so the title keeps its place.
                            Circle().fill(Color.accentColor).frame(width: 6, height: 6)
                                .offset(x: -11)
                                .help("已自訂")
                                .accessibilityLabel("已自訂")
                        }
                    }
                if let subtitle {
                    Text(subtitle)
                        .font(.callout)
                        .foregroundStyle(dimmed ? .tertiary : .secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let link { PlaceLink(title: link.title, place: link.place) }
            }
            .padding(.leading, indent ? RowMetrics.indent : 0)
            .frame(maxWidth: .infinity, alignment: .leading)
            .anchorPreference(key: RowBlocks.self, value: .bounds) { RowBlocks.Value(leading: $0) }
            HStack(spacing: RowMetrics.trailingSpacing) {
                if let value {
                    Text(value)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .help(value)
                }
                control()
                accessoryView
            }
            .layoutPriority(1)
            .anchorPreference(key: RowBlocks.self, value: .bounds) { RowBlocks.Value(trailing: $0) }
        }
        .overlayPreferenceValue(RowBlocks.self) { blocks in
            if RowGeometry.recording, let leading = blocks.leading, let trailing = blocks.trailing {
                GeometryReader { proxy in
                    let _ = RowGeometry.record(probe, title: title, leading: proxy[leading], trailing: proxy[trailing],
                                               origin: proxy.frame(in: .global).origin)
                    Color.clear
                }
            }
        }
        .frame(minHeight: RowMetrics.minHeight)
        .background { ArrivalHighlight(id: anchor) }
        .id(anchor ?? title)
    }

    @ViewBuilder private var accessoryView: some View {
        switch accessory {
        case .none: EmptyView()
        case .chevron:
            Image(systemName: "chevron.right")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .frame(width: 16)
        case .link(let help, let action):
            Button(action: action) {
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.tertiary)
                    .frame(width: 16, height: 20)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(help)
            .accessibilityLabel(help)
        case .info(let title, let content): InfoButton(title: title) { content }
        }
    }
}

extension SettingRow where Control == EmptyView {
    init(title: String, subtitle: String? = nil, value: String? = nil, indent: Bool = false, dimmed: Bool = false,
         link: (title: String, place: SettingsPlace)? = nil, accessory: RowAccessory = .none, anchor: String? = nil) {
        self.init(title: title, subtitle: subtitle, value: value, indent: indent, dimmed: dimmed, link: link,
                  accessory: accessory, anchor: anchor) { EmptyView() }
    }
}

/// The arrival cue: when a link leads to a row, the row's cell tints with the accent colour once, softly, and fades
/// (`SettingsPaneView.reveal` times it); a section target tints its card (`SettingsCard`).  Drawn behind the row, so
/// it never changes the layout.
struct ArrivalHighlight: View {
    let id: String?
    @ObservedObject var model: SettingsModel = SettingsWindowController.shared.model
    static let peakOpacity = 0.17
    var body: some View {
        let lit = id != nil && model.arrivedRow == id
        RoundedRectangle(cornerRadius: RowMetrics.cardRadius - RowMetrics.tintInset, style: .continuous)
            .fill(Color.accentColor.opacity(lit ? (DemoPinning.arrivalOpacity ?? Self.peakOpacity) : 0))
            .padding(.horizontal, RowMetrics.tintInset - RowMetrics.cardInset)
            .padding(.vertical, RowMetrics.tintInset - RowMetrics.rowPadding)
            .allowsHitTesting(false)
    }
}

/// A whole row that is a button: opens a sub-page or another place.
struct SettingLinkRow: View {
    let title: String
    var subtitle: String?
    var value: String?
    var dimmed = false
    var anchor: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            SettingRow(title: title, subtitle: subtitle, value: value, dimmed: dimmed, accessory: .chevron, anchor: anchor)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityHint("開啟「\(title)」")
    }
}

/// The settings switch: one size everywhere.
struct RowSwitch: View {
    let title: String
    @Binding var isOn: Bool
    var body: some View {
        Toggle(title, isOn: $isOn)
            .labelsHidden()
            .toggleStyle(.switch)
            .controlSize(.small)
    }
}

/// The settings picker: a pop-up menu, or segments for two or three short choices.
struct RowPicker<Value: Hashable>: View {
    let title: String
    @Binding var selection: Value
    let options: [(String, Value)]
    var segmented = false
    var body: some View {
        let picker = Picker(title, selection: $selection) {
            ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                Text(option.0).tag(option.1)
            }
        }
        .labelsHidden()
        .fixedSize()
        .controlSize(.small)
        if segmented { picker.pickerStyle(.segmented) } else { picker.pickerStyle(.menu) }
    }
}

// MARK: - Geometry check

/// While checking (the UI preview renderer), each row reports where its leading and trailing blocks are, so the
/// renderer can verify that the trailing block is centred on the leading one and draw a grid over the pages.
enum RowGeometry {
    struct Row { let id: String; let leading: CGRect; let trailing: CGRect }
    struct Card { var frame: CGRect = .zero; var children: [CGRect] = [] }
    static var recording = false
    private static var rows: [UUID: Row] = [:]
    private static var cardFrames: [UUID: CGRect] = [:]
    private static var childFrames: [UUID: [CGRect]] = [:]

    static func reset() { rows = [:]; cardFrames = [:]; childFrames = [:]; bar = nil; listRows = [:] }

    /// Rows of the list pages (not `SettingRow`s), for the bar check.
    private static var listRows: [String: CGRect] = [:]
    static func recordListRow(_ id: String, _ frame: CGRect) { if recording, frame.height > 0 { listRows[id] = frame } }

    /// The sub-page bar, which no row may be under while the page is at its top.
    private(set) static var bar: CGRect?
    static func recordBar(_ frame: CGRect) { if recording, frame.height > 0 { bar = frame } }

    /// Rows under the sub-page bar.
    static func underBar(_ rows: [Row]) -> [String] {
        guard let bar else { return [] }
        let all = rows.map { ($0.id, $0.leading) } + listRows.map { ($0.key, $0.value) }
        return all.filter { $0.1.minY < bar.maxY - 0.5 && $0.1.maxY > bar.minY }
            .map { "「\($0.0)」 at y \($0.1.minY) is under the sub-page bar (bottom \(bar.maxY))" }
    }

    /// Records a row's two blocks, measured in the same layout pass; a later pass replaces them.
    static func record(_ probe: UUID, title: String, leading: CGRect, trailing: CGRect, origin: CGPoint) {
        guard recording, leading.width > 0, trailing.width > 0 else { return }
        rows[probe] = Row(id: title, leading: leading.offsetBy(dx: origin.x, dy: origin.y),
                          trailing: trailing.offsetBy(dx: origin.x, dy: origin.y))
    }

    static func recordCard(_ card: UUID, _ frame: CGRect) {
        guard recording, frame.width > 0 else { return }
        if let old = cardFrames[card], old != frame { childFrames[card] = [] }
        cardFrames[card] = frame
    }

    static func recordChild(_ card: UUID, _ frame: CGRect) {
        guard recording, frame.height > 0 else { return }
        var list = childFrames[card] ?? []
        list.removeAll { abs($0.minY - frame.minY) < 0.5 || $0.intersects(frame.insetBy(dx: 0, dy: 1)) }
        list.append(frame)
        childFrames[card] = list
    }

    static var all: [Row] { Array(rows.values) }
    static var cards: [Card] {
        cardFrames.map { id, frame in Card(frame: frame, children: (childFrames[id] ?? []).filter { frame.insetBy(dx: -1, dy: -1).contains($0) }) }
    }

    /// Rows whose trailing block is not centred on the leading block (more than half a point off).
    static func misaligned(_ rows: [Row]) -> [Row] {
        rows.filter { abs($0.leading.midY - $0.trailing.midY) > 0.5 }
    }

    /// What is unbalanced on a card: a row whose left inset differs from its right one, or a first row whose space
    /// above differs from the last row's space below.
    static func imbalances(cards: [Card], rows: [Row]) -> [String] {
        var problems: [String] = []
        for card in cards {
            for row in rows where card.frame.insetBy(dx: -1, dy: -1).contains(row.leading) {
                let left = row.leading.minX - card.frame.minX, right = card.frame.maxX - row.trailing.maxX
                if abs(left - right) > 0.5 { problems.append("「\(row.id)」 inset left \(left) pt, right \(right) pt") }
            }
            guard let top = card.children.map(\.minY).min(), let bottom = card.children.map(\.maxY).max() else { continue }
            let above = top - card.frame.minY, below = card.frame.maxY - bottom
            if abs(above - below) > 0.5 {
                problems.append("card at y \(Int(card.frame.minY)): \(above) pt above the first row, \(below) pt below the last")
            }
        }
        return problems
    }
}

private struct CardProbe: View {
    let card: UUID
    var body: some View {
        if RowGeometry.recording {
            GeometryReader { proxy in
                let _ = RowGeometry.recordCard(card, proxy.frame(in: .global))
                Color.clear
            }
        }
    }
}

private struct ChildProbe: View {
    let card: UUID
    var body: some View {
        if RowGeometry.recording {
            GeometryReader { proxy in
                let _ = RowGeometry.recordChild(card, proxy.frame(in: .global))
                Color.clear
            }
        }
    }
}

/// Where a row's leading and trailing blocks are, for the geometry check.
private struct RowBlocks: PreferenceKey {
    struct Value { var leading: Anchor<CGRect>?; var trailing: Anchor<CGRect>? }
    static let defaultValue = Value()
    static func reduce(value: inout Value, nextValue: () -> Value) {
        let next = nextValue()
        value.leading = next.leading ?? value.leading
        value.trailing = next.trailing ?? value.trailing
    }
}

// MARK: - Hover

/// What the pointer is over in the settings window: one thing at most.  Each hover-revealed control registers
/// through `hoverTracked(_:)`.  Content scrolling under a still pointer sends no exit, so any scroll clears it, as do
/// the window losing key and a page change; the next pointer move sets it again.
final class HoverState: ObservableObject {
    static let shared = HoverState()
    @Published private(set) var hovered: AnyHashable?
    /// A part of the hovered thing under the pointer (a key chip in a row), tracked on its own.
    @Published private(set) var detail: AnyHashable?
    private var monitor: Any?

    func enter(_ id: AnyHashable) {
        if hovered != id { hovered = id }
        if monitor == nil, NSApp != nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel]) { [weak self] event in
                self?.scrolled()
                return event
            }
        }
    }
    func leave(_ id: AnyHashable) { if hovered == id { hovered = nil } }
    func enterDetail(_ id: AnyHashable) { if detail != id { detail = id } }
    func leaveDetail(_ id: AnyHashable) { if detail == id { detail = nil } }
    func scrolled() { clear() }
    func clear() {
        if hovered != nil { hovered = nil }
        if detail != nil { detail = nil }
    }
}

private struct HoverTracked: ViewModifier {
    let id: AnyHashable
    @ObservedObject var state = HoverState.shared
    func body(content: Content) -> some View {
        // The UI previews draw without a pointer.
        if SettingsWindowController.isPreview { content } else { tracked(content) }
    }
    private func tracked(_ content: Content) -> some View {
        content.onContinuousHover { phase in
            switch phase {
            case .active: state.enter(id)
            case .ended: state.leave(id)
            }
        }
    }
}

private struct HoverTrackedDetail: ViewModifier {
    let id: AnyHashable
    @ObservedObject var state = HoverState.shared
    func body(content: Content) -> some View {
        if SettingsWindowController.isPreview {
            content
        } else {
            content.onContinuousHover { phase in
                switch phase {
                case .active: state.enterDetail(id)
                case .ended: state.leaveDetail(id)
                }
            }
        }
    }
}

extension View {
    /// Reports the pointer over this view to `HoverState` under `id`.
    func hoverTracked(_ id: AnyHashable) -> some View { modifier(HoverTracked(id: id)) }
    /// Reports the pointer over a part (a chip) of the hovered thing.
    func hoverTrackedDetail(_ id: AnyHashable) -> some View { modifier(HoverTrackedDetail(id: id)) }
}
