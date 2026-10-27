import AppKit
import SwiftUI

// The list pages (記住的字詞, 自訂詞, 在候選字視窗顯示組字的 App) in the same design as every other page: a toolbar
// row (search, count, sort, add or more), the rows on one card, a footer.  Rows show the item, its reading or
// identifier in secondary text and quiet details on the right; 刪除 and 編輯 appear over the details on hover (no
// layout change), and through selection with ⌫ and the context menu.  Rows are lazy, so long lists stay smooth.

/// How a list can be ordered.
enum ListSort: String, CaseIterable, Identifiable {
    case count, reading, word, name
    var id: String { rawValue }
    var title: String {
        switch self {
        case .count: return "次數"
        case .reading: return "注音"
        case .word: return "字詞"
        case .name: return "名稱"
        }
    }
}

struct ListItem: Identifiable, Equatable {
    let id: String
    var title: String
    /// The reading, or the app's identifier: secondary text after the title.
    var detail: String?
    /// Quiet text on the right: times used, 不記住.
    var meta: String?
    /// An app's bundle identifier, for its icon.
    var appIcon: String?
    /// Sorting by count: larger first.
    var count: Double = 0
    var marked = false

    func key(_ sort: ListSort) -> String {
        switch sort {
        case .count, .word, .name: return title
        case .reading: return detail ?? ""
        }
    }
    var searchText: String { [title, detail ?? "", meta ?? ""].joined(separator: " ").lowercased() }
}

/// One list's state: its items, the search, the order, the selection, and what its actions do.
final class ListModel: ObservableObject {
    @Published private(set) var items: [ListItem] = []
    @Published var query = ""
    @Published var sort: ListSort
    @Published var selection: Set<String> = []
    let sorts: [ListSort]
    let noun: String
    let emptyTitle: String
    let emptyText: String
    var canEdit = false
    var canAdd = false
    var onRemove: (([ListItem]) -> Void)?
    var onEdit: ((ListItem) -> Void)?
    var onAdd: (() -> Void)?
    /// The ⋯ menu's items, built when it opens.
    var moreItems: (() -> [(title: String, destructive: Bool, enabled: Bool, action: () -> Void)])?
    /// Extra context-menu items for the selection.
    var rowItems: (([ListItem]) -> [(title: String, action: () -> Void)])?

    init(noun: String, sorts: [ListSort], emptyTitle: String, emptyText: String) {
        self.noun = noun
        self.sorts = sorts
        sort = sorts[0]
        self.emptyTitle = emptyTitle
        self.emptyText = emptyText
    }

    /// Replaces the items (animated by the page), keeping the selection where the ids survive.
    func set(_ new: [ListItem], select ids: Set<String>? = nil) {
        items = new
        let ids = ids ?? selection
        selection = ids.intersection(new.map(\.id))
    }

    var visible: [ListItem] {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        let shown = needle.isEmpty ? items : items.filter { $0.searchText.contains(needle) }
        let sort = self.sort
        return shown.sorted { a, b in
            if sort == .count, a.count != b.count { return a.count > b.count }
            let order = a.key(sort).localizedStandardCompare(b.key(sort))
            return order == .orderedSame ? a.id < b.id : order == .orderedAscending
        }
    }

    func remove(_ ids: Set<String>) {
        let rows = items.filter { ids.contains($0.id) }
        guard !rows.isEmpty else { return }
        onRemove?(rows)
    }
}

/// A list page's list: the toolbar row and the card.
struct ListSection: View {
    @ObservedObject var list: ListModel
    @ObservedObject private var hover = HoverState.shared
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let rows = list.visible
        VStack(alignment: .leading, spacing: RowMetrics.headerGap + 3) {
            toolbar(count: rows.count)
            listCard(rows)
        }
        .animation(SettingsWindowController.isPreview ? nil : reduceMotion ? .easeInOut(duration: 0.15) : .easeOut(duration: 0.22),
                   value: rows.map(\.id))
    }

    private func toolbar(count: Int) -> some View {
        HStack(spacing: 10) {
            HStack(spacing: 5) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary).font(.system(size: 12))
                TextField("搜尋\(list.noun)", text: $list.query)
                    .textFieldStyle(.plain)
                if !list.query.isEmpty {
                    Button { list.query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary) }
                        .buttonStyle(.plain)
                        .help("清除搜尋")
                }
            }
            .padding(.horizontal, 8)
            .frame(height: 26)
            .frame(maxWidth: 260)
            .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(Color.primary.opacity(0.05)))
            Spacer(minLength: 0)
            Text(list.query.isEmpty ? "共 \(list.items.count) 個" : "\(count) / \(list.items.count) 個")
                .font(.callout)
                .foregroundStyle(.secondary)
                .monospacedDigit()
            if list.sorts.count > 1 {
                Picker("排序", selection: $list.sort) {
                    ForEach(list.sorts) { Text("依\($0.title)").tag($0) }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .controlSize(.small)
                .fixedSize()
            }
            if list.canAdd {
                Button { list.onAdd?() } label: { Image(systemName: "plus") }
                    .controlSize(.small)
                    .help("加入\(list.noun)")
            }
            if let more = list.moreItems {
                Menu {
                    ForEach(Array(more().enumerated()), id: \.offset) { _, item in
                        Button(item.title, role: item.destructive ? .destructive : nil, action: item.action)
                            .disabled(!item.enabled)
                    }
                } label: { Image(systemName: "ellipsis.circle") }
                    .menuStyle(.borderlessButton)
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .help("更多")
            }
        }
        .padding(.horizontal, RowMetrics.cardInset)
    }

    @ViewBuilder private func listCard(_ rows: [ListItem]) -> some View {
        if rows.isEmpty {
            VStack(spacing: 6) {
                Image(systemName: list.query.isEmpty ? "tray" : "magnifyingglass")
                    .font(.system(size: 22, weight: .light))
                    .foregroundStyle(.tertiary)
                Text(list.query.isEmpty ? list.emptyTitle : "沒有符合「\(list.query)」的\(list.noun)")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Text(list.query.isEmpty ? list.emptyText : "試試較短的字，或用注音搜尋。")
                    .font(.callout)
                    .foregroundStyle(.tertiary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 36)
            .padding(.horizontal, RowMetrics.cardInset)
            .background(RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous).fill(Color.primary.opacity(0.03)))
            .transition(.opacity)
        } else {
            LazyVStack(spacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.element.id) { index, item in
                    let row = ListItemRow(list: list, item: item, first: index == 0, hovered: hover.hovered == AnyHashable("list-" + item.id))
                    if let removal = DemoPinning.removal, removal.id == item.id {
                        row.modifier(CollapseModifier(fraction: 1 - removal.progress))
                    } else {
                        row.transition(.collapse)
                    }
                }
            }
            .background(RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous).fill(Color.primary.opacity(0.03)))
            .clipShape(RoundedRectangle(cornerRadius: RowMetrics.cardRadius, style: .continuous))
            .focusable()
            .focusEffectDisabled()
            .focused($focused)
            .onKeyPress(keys: [.delete, .deleteForward]) { _ in
                guard !list.selection.isEmpty else { return .ignored }
                list.remove(list.selection)
                return .handled
            }
            .onKeyPress(.escape) {
                guard !list.selection.isEmpty else { return .ignored }
                list.selection = []
                return .handled
            }
        }
    }
}

/// One row: the item, its reading, details on the right; 編輯 and 刪除 fade in over the details on hover.
private struct ListItemRow: View {
    @ObservedObject var list: ListModel
    let item: ListItem
    let first: Bool
    let hovered: Bool

    var body: some View {
        let selected = list.selection.contains(item.id)
        VStack(spacing: 0) {
            if !first {
                Rectangle().fill(Color.primary.opacity(0.08)).frame(height: 1).padding(.horizontal, RowMetrics.cardInset)
            }
            HStack(spacing: 10) {
                if let bundle = item.appIcon {
                    Image(nsImage: AppIcons.icon(bundle)).resizable().frame(width: 20, height: 20)
                }
                Text(item.title)
                    .foregroundStyle(item.marked ? .secondary : .primary)
                    .lineLimit(1)
                if let detail = item.detail {
                    Text(detail)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
                Spacer(minLength: 8)
                // The details and the hover actions share one place, so hovering never moves anything.
                ZStack(alignment: .trailing) {
                    Text(item.meta ?? "")
                        .font(.callout)
                        .monospacedDigit()
                        .foregroundStyle(.tertiary)
                        .opacity(hovered ? 0 : 1)
                    HStack(spacing: 4) {
                        if list.canEdit {
                            Button("編輯") { list.onEdit?(item) }
                        }
                        Button("刪除") { list.remove([item.id]) }
                    }
                    .buttonStyle(.borderless)
                    .controlSize(.small)
                    .opacity(hovered ? 1 : 0)
                    .allowsHitTesting(hovered)
                }
                .animation(.easeOut(duration: 0.12), value: hovered)
            }
            .frame(minHeight: RowMetrics.minHeight)
            .padding(.horizontal, RowMetrics.cardInset)
            .padding(.vertical, RowMetrics.rowPadding - 2)
            .background {
                RoundedRectangle(cornerRadius: RowMetrics.cardRadius - RowMetrics.tintInset, style: .continuous)
                    .fill(Color.accentColor.opacity(selected ? 0.16 : 0))
                    .padding(.horizontal, RowMetrics.tintInset)
                    .padding(.vertical, 2)
            }
            .contentShape(Rectangle())
        }
        .hoverTracked("list-" + item.id)
        .background {
            if RowGeometry.recording {
                GeometryReader { proxy in
                    let _ = RowGeometry.recordListRow(item.id, proxy.frame(in: .global))
                    Color.clear
                }
            }
        }
        .onTapGesture {
            if NSEvent.modifierFlags.contains(.command) {
                if selected { list.selection.remove(item.id) } else { list.selection.insert(item.id) }
            } else {
                list.selection = selected && list.selection.count == 1 ? [] : [item.id]
            }
        }
        .contextMenu {
            let targets = selected ? list.items.filter { list.selection.contains($0.id) } : [item]
            if list.canEdit, targets.count == 1 { Button("編輯…") { list.onEdit?(item) } }
            ForEach(Array((list.rowItems?(targets) ?? []).enumerated()), id: \.offset) { _, entry in
                Button(entry.title, action: entry.action)
            }
            Divider()
            Button(targets.count > 1 ? "刪除 \(targets.count) 個" : "刪除", role: .destructive) { list.remove(Set(targets.map(\.id))) }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// App icons, loaded once.
enum AppIcons {
    private static var cache: [String: NSImage] = [:]
    static func icon(_ bundle: String) -> NSImage {
        if let icon = cache[bundle] { return icon }
        let icon = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundle).map { NSWorkspace.shared.icon(forFile: $0.path) }
            ?? NSWorkspace.shared.icon(for: .application)
        cache[bundle] = icon
        return icon
    }
}

// MARK: - Collapse

/// A removed row folds its height away (the rows below move up with it) while fading; an added row unfolds.
struct CollapseModifier: ViewModifier, Animatable {
    var fraction: CGFloat
    var animatableData: CGFloat {
        get { fraction }
        set { fraction = newValue }
    }
    func body(content: Content) -> some View {
        let layout = HeightFraction(fraction: fraction)
        layout { content }
            .opacity(Double(max(0, fraction * 1.4 - 0.4)))
            .clipped()
    }
}

extension AnyTransition {
    static var collapse: AnyTransition {
        .modifier(active: CollapseModifier(fraction: 0), identity: CollapseModifier(fraction: 1))
    }
}

/// Lays out its content at full size and reports `fraction` of its height.
struct HeightFraction: Layout {
    var fraction: CGFloat
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let size = subviews.first?.sizeThatFits(proposal) ?? .zero
        return CGSize(width: size.width, height: size.height * fraction)
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        subviews.first?.place(at: bounds.origin, anchor: .topLeading, proposal: ProposedViewSize(width: bounds.width, height: nil))
    }
}
