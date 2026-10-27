// The engine: lexicon, language models and personal data, in Rust (engine/crates, through zhuyin-ffi's C ABI).
import CZhuyin
import Foundation

/// An error from the Rust engine, with its message.
public struct EngineError: LocalizedError {
    public let message: String
    public var errorDescription: String? { message }
}

/// Takes a string returned by the library.
func take(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
    guard let pointer else { return nil }
    defer { zt_string_free(pointer) }
    return String(cString: pointer)
}

/// Runs `body` with an error slot and throws the library's message when it reports one.
func checked<T>(_ body: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> T?) throws -> T {
    var error: UnsafeMutablePointer<CChar>?
    if let value = body(&error) { return value }
    throw EngineError(message: take(error) ?? "unknown error")
}

public final class Engine: @unchecked Sendable {
    let handle: OpaquePointer

    /// The chat-domain model directory, read when an engine is created (Swift's `Engine.chatModelDirectory`).
    nonisolated(unsafe) public static var chatModelDirectory: URL?

    private static func config() -> ZtEngineConfig { zt_engine_config_default() }

    public init(lexiconURL: URL, userDataDirectory: URL?) throws {
        var config = Self.config()
        let chat = Self.chatModelDirectory?.path, user = userDataDirectory?.path
        handle = try chat.withOptionalCString { chatPath in
            config.chat_model_directory = chatPath
            return try user.withOptionalCString { user in
                try checked { error in zt_engine_open(lexiconURL.path, user, &config, error) }
            }
        }
    }

    public init(lexiconText: String, userDataDirectory: URL? = nil, languageModelDirectory: URL? = nil) throws {
        var config = Self.config()
        let chat = Self.chatModelDirectory?.path
        let user = userDataDirectory?.path, model = languageModelDirectory?.path
        handle = try chat.withOptionalCString { chatPath in
            config.chat_model_directory = chatPath
            return try user.withOptionalCString { user in
                try model.withOptionalCString { model in
                    try checked { error in zt_engine_open_text(lexiconText, user, model, &config, error) }
                }
            }
        }
    }

    deinit { zt_engine_free(handle) }

    public var fuzzySounds: FuzzySounds {
        get { FuzzySounds(rawValue: Int(zt_engine_fuzzy_sounds(handle))) }
        set { zt_engine_set_fuzzy_sounds(handle, UInt32(truncatingIfNeeded: newValue.rawValue)) }
    }

    public func flushPersonalData() { zt_engine_flush_personal_data(handle) }
    public func forgetAllLearned() { zt_engine_forget_all_learned(handle) }

    private func phrases(_ list: UnsafeMutablePointer<ZtPhraseList>?) -> [(reading: String, word: String, count: Double)] {
        guard let list else { return [] }
        defer { zt_phrase_list_free(list) }
        return (0..<list.pointee.count).map { i in
            let item = list.pointee.items[i]
            return (String(cString: item.reading), String(cString: item.word), item.count)
        }
    }

    public func userPhrases() -> [(reading: String, word: String)] {
        phrases(zt_engine_user_phrases(handle)).map { ($0.reading, $0.word) }
    }

    public func learnedEntries() -> [(reading: String, word: String, count: Double)] {
        phrases(zt_engine_learned_entries(handle))
    }

    public func excludedEntries() -> [(reading: String, word: String)] {
        phrases(zt_engine_excluded_entries(handle)).map { ($0.reading, $0.word) }
    }

    public func addUserPhrase(word: String, reading: String) throws {
        _ = try checked { error in zt_engine_add_user_phrase(handle, word, reading, error) ? true : nil }
    }

    public func removeUserPhrase(word: String, reading: String) { zt_engine_remove_user_phrase(handle, word, reading) }
    public func forgetLearned(word: String, reading: String) { zt_engine_forget_learned(handle, word, reading) }
    public func excludeFromLearning(word: String, reading: String) { zt_engine_exclude_from_learning(handle, word, reading) }
    public func includeInLearning(word: String, reading: String) { zt_engine_include_in_learning(handle, word, reading) }

    public func readings(of word: String) -> [String] {
        guard let list = zt_engine_readings(handle, word) else { return [] }
        defer { zt_string_list_free(list) }
        return (0..<list.pointee.count).map { String(cString: list.pointee.items[$0]!) }
    }

    public func exportPersonalData() -> Data {
        Data((take(zt_engine_export_personal_data(handle)) ?? "").utf8)
    }

    @discardableResult
    public func importPersonalData(_ data: Data) throws -> Int {
        let count = try data.withUnsafeBytes { bytes in
            try checked { error -> Int64? in
                let read = zt_engine_import_personal_data(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, error)
                return read < 0 ? nil : read
            }
        }
        return Int(count)
    }
}

extension Optional where Wrapped == String {
    /// `body` with a C string for the value, or NULL for nil.
    func withOptionalCString<T>(_ body: (UnsafePointer<CChar>?) throws -> T) rethrows -> T {
        switch self {
        case .some(let string): return try string.withCString(body)
        case .none: return try body(nil)
        }
    }
}
