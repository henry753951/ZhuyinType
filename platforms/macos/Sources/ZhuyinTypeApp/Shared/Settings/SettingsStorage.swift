import Foundation

/// Where settings are stored: the methods of `UserDefaults` that `Preferences` uses.  The app uses `UserDefaults`; the
/// setup wizard's sandbox, the UI previews and the self-tests use `MemorySettingsStorage`, so trying settings out writes
/// nothing until they are applied.
protocol SettingsStorage: AnyObject {
    func register(defaults: [String: Any])
    func object(forKey key: String) -> Any?
    func set(_ value: Any?, forKey key: String)
    func removeObject(forKey key: String)
    func bool(forKey key: String) -> Bool
    func integer(forKey key: String) -> Int
    func double(forKey key: String) -> Double
    func string(forKey key: String) -> String?
    func data(forKey key: String) -> Data?
    func stringArray(forKey key: String) -> [String]?
}

extension UserDefaults: SettingsStorage {}

/// Settings kept in memory only.
final class MemorySettingsStorage: SettingsStorage {
    private var values: [String: Any] = [:]
    private var registered: [String: Any] = [:]

    init(copying source: [String: Any] = [:]) { values = source }

    func register(defaults: [String: Any]) { registered.merge(defaults) { _, new in new } }
    func object(forKey key: String) -> Any? { values[key] ?? registered[key] }
    func set(_ value: Any?, forKey key: String) {
        if let value { values[key] = value } else { values.removeValue(forKey: key) }
    }
    func removeObject(forKey key: String) { values.removeValue(forKey: key) }
    func bool(forKey key: String) -> Bool { (object(forKey: key) as? NSNumber)?.boolValue ?? (object(forKey: key) as? Bool) ?? false }
    func integer(forKey key: String) -> Int { (object(forKey: key) as? NSNumber)?.intValue ?? (object(forKey: key) as? Int) ?? 0 }
    func double(forKey key: String) -> Double { (object(forKey: key) as? NSNumber)?.doubleValue ?? (object(forKey: key) as? Double) ?? 0 }
    func string(forKey key: String) -> String? { object(forKey: key) as? String }
    func data(forKey key: String) -> Data? { object(forKey: key) as? Data }
    func stringArray(forKey key: String) -> [String]? { object(forKey: key) as? [String] }

    /// What has been set here, by key (not the registered defaults).
    var stored: [String: Any] { values }
}
