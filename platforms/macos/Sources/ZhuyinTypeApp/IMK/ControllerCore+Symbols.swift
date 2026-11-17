import AppKit
import ZhuyinEngine

// 符號與 Emoji in the controller: the shortcut opens the picker (taking what was being composed as its search), every
// key goes to the picker while it is open, and the chosen symbol is sent as a picked candidate would be.
extension ControllerCore {
    var isSymbolPickerOpen: Bool { symbolPicker != nil }

    /// What the picker makes of a key: nil when the picker is closed and the key is not its shortcut.
    func symbolKey(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool? {
        let shortcut = Shortcut(keyCode: keyCode, modifiers: modifiers.subtracting(.capsLock))
        let isShortcut = shortcut.map { shortcuts[$0] == .symbolPicker } ?? false
        guard let picker = symbolPicker else {
            guard isShortcut, symbolSource != nil else { return nil }
            openSymbolPicker()
            return true
        }
        if isShortcut {
            closeSymbolPicker()
            return true
        }
        switch picker.handle(keyCode: keyCode, modifiers: modifiers) {
        case .handled:
            showSymbolPicker?(picker, symbolCaret)
            return true
        case .insert(let symbol):
            insertSymbol(symbol)
            return true
        case .close:
            closeSymbolPicker()
            return true
        case .passThrough:
            closeSymbolPicker()
            return false
        }
    }

    /// Opens the picker.  What was being composed becomes its search (the text as shown) and leaves the app's field:
    /// cancelled, not sent.
    func openSymbolPicker() {
        guard let symbolSource, symbolPicker == nil else { return }
        let prefill = composer.presentation.text
        symbolCaret = client.caretRect(cursorIndex: 0)
        if composer.isComposing {
            composer.reset()
            render()
        }
        hidePanel?()
        let picker = SymbolPicker(source: symbolSource, recent: recentSymbols?.items ?? [], prefill: prefill)
        symbolPicker = picker
        showSymbolPicker?(picker, symbolCaret)
    }

    func closeSymbolPicker() {
        guard symbolPicker != nil else { return }
        symbolPicker = nil
        hideSymbolPicker?()
    }

    /// A click on an item of the picker.
    func pickSymbol(at index: Int) {
        guard let symbol = symbolPicker?.item(at: index) else { return }
        insertSymbol(symbol)
    }

    /// A click on a category tab.
    func selectSymbolCategory(_ index: Int) {
        guard let picker = symbolPicker else { return }
        picker.selectTab(index)
        showSymbolPicker?(picker, symbolCaret)
    }

    /// The mouse wheel over the grid.
    func scrollSymbols(rows: Int) {
        guard let picker = symbolPicker else { return }
        picker.scroll(rows: rows)
        showSymbolPicker?(picker, symbolCaret)
    }

    /// Sends `symbol` into the field as a picked candidate is sent, and starts a new sentence after it.  Recent
    /// symbols are not kept from a password field.
    private func insertSymbol(_ symbol: String) {
        closeSymbolPicker()
        render(commit: symbol)
        breakContext()
        if !secureInput(), let symbolSource { recentSymbols?.push(symbol, source: symbolSource) }
    }
}
