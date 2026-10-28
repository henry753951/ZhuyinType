import AppKit
import UniformTypeIdentifiers
import ZhuyinEngine

/// 自訂詞: user phrases, edited through the engine.
final class PhraseListController: NSObject, NSTextFieldDelegate {
    private weak var model: SettingsModel?
    let list = ListModel(noun: "自訂詞", sorts: [.reading, .word], emptyTitle: "還沒有自訂詞",
                         emptyText: "按 ＋ 加入，或打字時選取字詞再按 Return。自訂詞在選字時優先列出。")
    private let wordField = NSTextField()
    private let readingField = NSComboBox()

    init(model: SettingsModel) {
        self.model = model
        super.init()
        list.canAdd = true
        list.canEdit = true
        list.onAdd = { [weak self] in self?.presentAddSheet() }
        list.onEdit = { [weak self] item in self?.presentAddSheet(word: item.title, reading: item.detail ?? "", replacing: item) }
        list.onRemove = { [weak self] rows in self?.remove(rows) }
    }

    func reload(select ids: Set<String>? = nil) {
        guard let engine = model?.engine else { list.set([]); return }
        let rows = engine.userPhrases().map { ListItem(id: "\($0.reading)\t\($0.word)", title: $0.word, detail: $0.reading) }
        list.set(rows, select: ids)
    }

    private func remove(_ rows: [ListItem]) {
        guard let engine = model?.engine else { return }
        for row in rows { engine.removeUserPhrase(word: row.title, reading: row.detail ?? "") }
        reload()
    }

    private var editing: ListItem?

    private func presentAddSheet(word: String = "", reading: String = "", replacing: ListItem? = nil) {
        guard let window = SettingsWindowController.shared.sheetWindow, model?.engine != nil else { return }
        editing = replacing
        let alert = NSAlert()
        alert.messageText = replacing == nil ? "加入自訂詞" : "編輯自訂詞"
        alert.informativeText = "自訂詞在選字時優先列出。"
        alert.addButton(withTitle: replacing == nil ? "加入" : "儲存")
        alert.addButton(withTitle: "取消")

        wordField.stringValue = word
        wordField.placeholderString = "例如：王小明"
        wordField.delegate = self
        readingField.stringValue = reading
        readingField.placeholderString = "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ"
        readingField.completes = false
        readingField.numberOfVisibleItems = 6
        updateReadings()

        let grid = NSGridView(views: [
            [NSTextField(labelWithString: "詞："), wordField],
            [NSTextField(labelWithString: "讀音："), readingField],
        ])
        grid.rowSpacing = 8
        grid.column(at: 0).xPlacement = .trailing
        grid.column(at: 1).width = 240
        grid.frame = NSRect(x: 0, y: 0, width: 300, height: 56)
        alert.accessoryView = grid
        alert.window.initialFirstResponder = wordField

        alert.beginSheetModal(for: window) { [weak self] response in
            guard let self, response == .alertFirstButtonReturn else { return }
            self.add(word: self.wordField.stringValue, reading: self.readingField.stringValue, window: window)
        }
    }

    private func add(word: String, reading: String, window: NSWindow) {
        guard let engine = model?.engine else { return }
        let word = word.trimmingCharacters(in: .whitespaces)
        let reading = reading.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        do {
            try engine.addUserPhrase(word: word, reading: reading)
            if let old = editing, old.title != word || old.detail != reading {
                engine.removeUserPhrase(word: old.title, reading: old.detail ?? "")
            }
            editing = nil
            reload(select: ["\(reading)\t\(word)"])
        } catch {
            let alert = NSAlert()
            alert.messageText = "無法加入「\(word)」"
            alert.informativeText = "詞需為 1 到 6 個字，讀音需為每個字一個注音，以空白分隔。"
            alert.addButton(withTitle: "修改")
            alert.addButton(withTitle: "取消")
            alert.beginSheetModal(for: window) { [weak self] response in
                if response == .alertFirstButtonReturn {
                    DispatchQueue.main.async { self?.presentAddSheet(word: word, reading: reading, replacing: self?.editing) }
                }
            }
        }
    }

    func controlTextDidChange(_ notification: Notification) { updateReadings() }

    private func updateReadings() {
        let word = wordField.stringValue.trimmingCharacters(in: .whitespaces)
        let readings = word.isEmpty ? [] : (model?.engine?.readings(of: word) ?? [])
        readingField.removeAllItems()
        readingField.addItems(withObjectValues: readings)
        if let first = readings.first, !readings.contains(readingField.stringValue) {
            readingField.stringValue = first
        }
    }
}

/// 記住的字詞: what the engine learned from picks, and the words it is told never to remember.
final class LearnedListController: NSObject {
    private weak var model: SettingsModel?
    let list = ListModel(noun: "字詞", sorts: [.count, .reading, .word], emptyTitle: "還沒有記住的字詞",
                         emptyText: "從候選字清單選字後，ZhuyinType 會記住你的選擇，下次排在前面。")
    private let number: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.maximumFractionDigits = 1
        return formatter
    }()
    private static let excludedPrefix = "excluded\t"

    init(model: SettingsModel) {
        self.model = model
        super.init()
        list.onRemove = { [weak self] rows in self?.remove(rows) }
        list.moreItems = { [weak self] in
            guard let self else { return [] }
            let chosen = self.list.items.filter { self.list.selection.contains($0.id) && !$0.marked }
            return [("不再記住所選字詞", false, !chosen.isEmpty, { self.exclude(chosen) }),
                    ("匯出…", false, true, { self.exportData() }), ("匯入…", false, true, { self.importData() }),
                    ("全部清除…", true, true, { self.confirmClearAll() })]
        }
        list.rowItems = { [weak self] rows in
            let learned = rows.filter { !$0.marked }
            guard let self, !learned.isEmpty else { return [] }
            return [("不再記住", { self.exclude(learned) })]
        }
    }

    func reload() {
        guard let engine = model?.engine else { list.set([]); return }
        let entries = engine.learnedEntries()
        let top = log1p(entries.map(\.count).max() ?? 1)
        let learned = entries.map { entry in
            ListItem(id: "\(entry.reading)\t\(entry.word)", title: entry.word, detail: entry.reading, count: entry.count,
                     level: max(1, Int((log1p(entry.count) / max(top, 0.0001) * 5).rounded(.up))))
        }
        let excluded = engine.excludedEntries().map { entry in
            ListItem(id: Self.excludedPrefix + "\(entry.reading)\t\(entry.word)", title: entry.word, detail: entry.reading,
                     meta: "不記住", count: -1, marked: true)
        }
        list.set(learned + excluded)
    }

    /// Removing a learned word forgets it; removing an excluded one lets it be learned again.
    private func remove(_ rows: [ListItem]) {
        guard let engine = model?.engine else { return }
        for row in rows {
            if row.marked { engine.includeInLearning(word: row.title, reading: row.detail ?? "") }
            else { engine.forgetLearned(word: row.title, reading: row.detail ?? "") }
        }
        reload()
    }

    private func exclude(_ rows: [ListItem]) {
        guard let engine = model?.engine else { return }
        for row in rows where !row.marked { engine.excludeFromLearning(word: row.title, reading: row.detail ?? "") }
        reload()
    }

    private func exportData() {
        guard let window = SettingsWindowController.shared.sheetWindow, let engine = model?.engine else { return }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "ZhuyinType 個人資料.txt"
        panel.allowedContentTypes = [.plainText]
        panel.message = "自訂詞、記住的字詞與不記住的字詞會存成一個文字檔。"
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            do { try engine.exportPersonalData().write(to: url, options: .atomic) }
            catch { Self.show("無法匯出", detail: error.localizedDescription, in: window) }
        }
    }

    private func importData() {
        guard let window = SettingsWindowController.shared.sheetWindow, let engine = model?.engine else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.plainText]
        panel.message = "匯入的資料會加到這台 Mac 現有的資料中。"
        panel.prompt = "匯入"
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            do {
                try engine.importPersonalData(Data(contentsOf: url))
                self?.reload()
                self?.model?.phrases.reload()
            } catch {
                Self.show("無法匯入「\(url.lastPathComponent)」", detail: "這個檔案不是 ZhuyinType 匯出的個人資料。", in: window)
            }
        }
    }

    private static func show(_ message: String, detail: String, in window: NSWindow) {
        let alert = NSAlert()
        alert.messageText = message
        alert.informativeText = detail
        alert.beginSheetModal(for: window)
    }

    private func confirmClearAll() {
        guard let window = SettingsWindowController.shared.sheetWindow else { return }
        let alert = NSAlert()
        alert.messageText = "要清除所有記住的字詞嗎？"
        alert.informativeText = "清除後無法復原。不記住的字詞會保留。"
        alert.addButton(withTitle: "清除").hasDestructiveAction = true
        alert.addButton(withTitle: "取消")
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertFirstButtonReturn else { return }
            self?.model?.engine?.forgetAllLearned()
            self?.reload()
        }
    }
}

/// 進階: apps whose composition is shown in the candidate window.
final class AppListController: NSObject {
    let list = ListModel(noun: "App", sorts: [.name], emptyTitle: "沒有 App",
                         emptyText: "每個 App 都直接在文字中顯示組字。少數 App 需要把組字改放在候選字視窗上方；按 ＋ 加入。")
    private let preferences: Preferences

    init(preferences: Preferences = .shared) {
        self.preferences = preferences
        super.init()
        list.canAdd = true
        list.onAdd = { [weak self] in self?.showAddMenu() }
        list.onRemove = { [weak self] rows in
            guard let self else { return }
            let removed = Set(rows.map(\.id))
            self.preferences.popupCompositionApps.removeAll { removed.contains($0) }
            self.reload()
        }
    }

    func reload(select ids: Set<String>? = nil) {
        let rows = preferences.popupCompositionApps.map { id -> ListItem in
            ListItem(id: id, title: Self.appInfo(bundleID: id).name, detail: id, appIcon: id)
        }
        list.set(rows, select: ids)
    }

    private static func appInfo(bundleID: String) -> (name: String, icon: NSImage) {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            let name = FileManager.default.displayName(atPath: url.path).replacingOccurrences(of: ".app", with: "")
            return (name, NSWorkspace.shared.icon(forFile: url.path))
        }
        return (bundleID, NSWorkspace.shared.icon(for: .application))
    }

    private func showAddMenu() {
        let menu = NSMenu()
        let existing = Set(preferences.popupCompositionApps)
        let apps = NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular && $0.bundleIdentifier != nil
                && $0.bundleIdentifier != Bundle.main.bundleIdentifier && !existing.contains($0.bundleIdentifier!) }
            .sorted { ($0.localizedName ?? "").localizedStandardCompare($1.localizedName ?? "") == .orderedAscending }
        for app in apps {
            let item = NSMenuItem(title: app.localizedName ?? app.bundleIdentifier!, action: #selector(addRunning(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = app.bundleIdentifier
            if let icon = app.icon?.copy() as? NSImage {
                icon.size = NSSize(width: 16, height: 16)
                item.image = icon
            }
            menu.addItem(item)
        }
        if !apps.isEmpty { menu.addItem(.separator()) }
        let other = NSMenuItem(title: "選擇其他 App…", action: #selector(chooseApp), keyEquivalent: "")
        other.target = self
        menu.addItem(other)
        menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
    }

    @objc private func addRunning(_ item: NSMenuItem) {
        guard let id = item.representedObject as? String else { return }
        add(id)
    }

    @objc private func chooseApp() {
        guard let window = SettingsWindowController.shared.sheetWindow else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.application]
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        panel.prompt = "加入"
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url, let id = Bundle(url: url)?.bundleIdentifier else { return }
            self?.add(id)
        }
    }

    private func add(_ id: String) {
        if !preferences.popupCompositionApps.contains(id) { preferences.popupCompositionApps.append(id) }
        reload(select: [id])
    }
}
