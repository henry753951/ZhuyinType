// Compiled with ZT_SYMBOLS (the zhuyin-symbols C functions in zhuyin.h); remove the condition once the engine has them.
#if ZT_SYMBOLS
import CZhuyin
import Foundation

/// 符號與 Emoji: the symbols, their names and the search of the engine's zhuyin-symbols crate (C functions
/// `zt_symbols_*` in zhuyin.h).  Every category except 最近使用, which the app keeps.
public enum Symbols {
    public struct Symbol: Decodable, Equatable, Sendable {
        public let text: String
        /// The zh_Hant CLDR name, or a short Chinese name.
        public let name: String
    }

    public struct Category: Decodable, Equatable, Sendable {
        public let id: String
        public let title: String
        public let symbols: [Symbol]
    }

    /// Every category, in the picker's order.
    public static func categories() -> [Category] { decode(take(zt_symbols_categories_json())) ?? [] }

    /// Ranked matches for the raw keys typed (Zhuyin on the standard layout, tones optional, or English).
    public static func search(_ query: String, limit: Int) -> [Symbol] {
        decode(take(zt_symbols_search_json(query, limit))) ?? []
    }

    /// The query as shown: "vul" → "ㄒㄧㄠ", "heart" → "heart".
    public static func displayQuery(_ query: String) -> String { take(zt_symbols_display_query(query)) ?? query }

    /// The recent list's stored text after `symbol` was inserted: newest first, each once, at most `recentCapacity`.
    public static func recentPush(stored: String, symbol: String) -> String {
        take(zt_symbols_recent_push(stored, symbol)) ?? stored
    }

    public static var recentCapacity: Int { Int(zt_symbols_recent_capacity()) }

    private static func decode<T: Decodable>(_ json: String?) -> T? {
        guard let data = json?.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }
}
#endif
