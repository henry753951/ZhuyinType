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

enum RowMetrics {
    static let minHeight: CGFloat = 24
    static let verticalPadding: CGFloat = 3
    static let indent: CGFloat = 18
    static let accessoryWidth: CGFloat = 20
    static let gap: CGFloat = 12
    static let trailingSpacing: CGFloat = 8
}

/// What sits in the accessory column at the right edge.
enum RowAccessory {
    case none
    case chevron
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

/// A section of a settings page: its header (which lights up with the arrival cue) and its rows.
struct SettingsSection<Content: View, Footer: View>: View {
    let title: String?
    @ViewBuilder let content: () -> Content
    @ViewBuilder let footer: () -> Footer

    var body: some View {
        Section {
            content().environment(\.settingsSection, title)
        } header: {
            if let title { SettingsSectionHeader(title: title) }
        } footer: {
            footer()
        }
    }
}

extension SettingsSection where Footer == EmptyView {
    init(_ title: String? = nil, @ViewBuilder content: @escaping () -> Content) {
        self.init(title: title, content: content, footer: { EmptyView() })
    }
}

extension SettingsSection {
    init(_ title: String? = nil, @ViewBuilder content: @escaping () -> Content, @ViewBuilder footer: @escaping () -> Footer) {
        self.init(title: title, content: content, footer: footer)
    }
}

/// A section's header: in the accent colour while the arrival cue is on a row of the section.
struct SettingsSectionHeader: View {
    let title: String
    @ObservedObject var model: SettingsModel = SettingsWindowController.shared.model
    var body: some View {
        Text(title)
            .foregroundStyle(model.arrivedSection == title ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.secondary))
            .id(RowSections.headerID(title))
    }
}

struct SettingRow<Control: View>: View {
    let title: String
    var subtitle: String?
    var value: String?
    var indent = false
    var dimmed = false
    var link: (title: String, place: SettingsPlace)?
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
                    .frame(width: RowMetrics.accessoryWidth)
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
        .padding(.vertical, RowMetrics.verticalPadding)
        .background { ArrivalHighlight(id: anchor, section: section) }
        .id(anchor ?? title)
    }

    @ViewBuilder private var accessoryView: some View {
        switch accessory {
        case .none: Color.clear.frame(height: 1)
        case .chevron:
            Image(systemName: "chevron.right")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
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

/// The arrival cue: when a link leads to a row, its background tints with the accent colour once, softly, and
/// fades (`SettingsPaneView.reveal` times it).  Drawn behind the row, so it never changes the layout.
struct ArrivalHighlight: View {
    let id: String?
    var section: String?
    @ObservedObject var model: SettingsModel = SettingsWindowController.shared.model
    static let peakOpacity = 0.17
    var body: some View {
        // The row a link led to, or every row of a section a link led to.
        let lit = (id != nil && model.arrivedRow == id) || (section != nil && model.arrivedRow == nil
                                                            && model.arrivedSection == section)
        let opacity = lit ? (DemoPinning.arrivalOpacity ?? Self.peakOpacity) : 0
        RoundedRectangle(cornerRadius: 7, style: .continuous)
            .fill(Color.accentColor.opacity(opacity))
            .padding(.horizontal, -8)
            .padding(.vertical, -1)
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
        if segmented { picker.pickerStyle(.segmented) } else { picker.pickerStyle(.menu) }
    }
}

// MARK: - Geometry check

/// While checking (the UI preview renderer), each row reports where its leading and trailing blocks are, so the
/// renderer can verify that the trailing block is centred on the leading one and draw a grid over the pages.
enum RowGeometry {
    struct Row { let id: String; let leading: CGRect; let trailing: CGRect }
    static var recording = false
    private static var rows: [UUID: Row] = [:]

    static func reset() { rows = [:] }

    /// Records a row's two blocks, measured in the same layout pass; a later pass replaces them.
    static func record(_ probe: UUID, title: String, leading: CGRect, trailing: CGRect, origin: CGPoint) {
        guard recording, leading.width > 0, trailing.width > 0 else { return }
        rows[probe] = Row(id: title, leading: leading.offsetBy(dx: origin.x, dy: origin.y),
                          trailing: trailing.offsetBy(dx: origin.x, dy: origin.y))
    }

    static var all: [Row] { Array(rows.values) }

    /// Rows whose trailing block is not centred on the leading block (more than half a point off).
    static func misaligned(_ rows: [Row]) -> [Row] {
        rows.filter { abs($0.leading.midY - $0.trailing.midY) > 0.5 }
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
