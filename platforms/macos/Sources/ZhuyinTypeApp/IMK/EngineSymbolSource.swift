// Compiled with ZT_SYMBOLS, with ZhuyinEngine/Symbols.swift; until then the picker uses SampleSymbolSource.
#if ZT_SYMBOLS
import Foundation
import ZhuyinEngine

/// The input method's symbols: the engine's zhuyin-symbols data, read once when the picker first opens.
final class EngineSymbolSource: SymbolSource {
    private(set) lazy var categories: [SymbolCategory] = Symbols.categories().map { category in
        SymbolCategory(id: category.id, title: category.title,
                       symbols: category.symbols.map { SymbolItem(text: $0.text, name: $0.name) })
    }
    private lazy var names: [String: String] = {
        var names: [String: String] = [:]
        for category in categories { for symbol in category.symbols where names[symbol.text] == nil { names[symbol.text] = symbol.name } }
        return names
    }()

    func search(_ query: String, limit: Int) -> [SymbolItem] {
        Symbols.search(query, limit: limit).map { SymbolItem(text: $0.text, name: $0.name) }
    }
    func displayQuery(_ query: String) -> String { Symbols.displayQuery(query) }
    func recentPush(stored: String, symbol: String) -> String { Symbols.recentPush(stored: stored, symbol: symbol) }
    func name(of text: String) -> String? { names[text] }
}
#endif
