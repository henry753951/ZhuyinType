import Carbon
import Foundation

enum InstallCommand {
    private static let bundleID = "org.zhuyintype.inputmethod.ZhuyinType"
    private static let modeID = bundleID + ".Zhuyin"

    static func handle(arguments: [String]) -> Bool {
        let flags = Set(arguments)
        let supported: Set<String> = ["--install", "--register", "--enable", "--select", "--disable", "--unregister", "--status"]
        guard !flags.isDisjoint(with: supported) else { return false }
        var succeeded = true

        func report(_ action: String, _ status: OSStatus) {
            print("\(action): \(status == noErr ? "OK" : "failed (OSStatus \(status))")")
            if status != noErr { succeeded = false }
        }
        if flags.contains("--install") || flags.contains("--register") {
            report("Register \(Bundle.main.bundleURL.path)", TISRegisterInputSource(Bundle.main.bundleURL as CFURL))
        }

        let sources = inputSources()
        let parents = sources.filter { string($0, kTISPropertyInputModeID) == nil }
        let modes = sources.filter { string($0, kTISPropertyInputSourceID) == modeID }
        let disabling = flags.contains("--disable") || flags.contains("--unregister")
        let enabling = flags.contains("--install") || flags.contains("--enable")

        if disabling {
            // TIS has no unregister API; disable the modes before their parent.
            for source in modes + parents where boolean(source, kTISPropertyInputSourceIsEnabled) {
                report("Disable \(string(source, kTISPropertyInputSourceID) ?? bundleID)", TISDisableInputSource(source))
            }
            if sources.isEmpty { print("ZhuyinType is already absent from registered input sources.") }
        } else {
            if enabling {
                for source in parents + modes where !boolean(source, kTISPropertyInputSourceIsEnabled) {
                    if boolean(source, kTISPropertyInputSourceIsEnableCapable) {
                        report("Enable \(string(source, kTISPropertyInputSourceID) ?? bundleID)", TISEnableInputSource(source))
                    }
                }
                let enabled = inputSources().contains {
                    string($0, kTISPropertyInputSourceID) == modeID && boolean($0, kTISPropertyInputSourceIsEnabled)
                }
                if !enabled {
                    print("Enable failed: add「ZhuyinType 注音」in System Settings → Keyboard → Input Sources → 繁體中文. Log out and back in if it is not listed.")
                    succeeded = false
                } else { print("ZhuyinType 注音 is enabled.") }
            }
            if flags.contains("--select") {
                if let mode = inputSources().first(where: { string($0, kTISPropertyInputSourceID) == modeID }) {
                    report("Select 注音 ZhuyinType", TISSelectInputSource(mode))
                } else {
                    print("Select failed: input mode not found; run --install first.")
                    succeeded = false
                }
            }
        }

        if flags.contains("--status") {
            let current = inputSources()
            print("ZhuyinType: \(current.count) registered source(s)")
            for source in current {
                print("\(string(source, kTISPropertyInputSourceID) ?? "unknown"): enabled=\(boolean(source, kTISPropertyInputSourceIsEnabled)) selected=\(boolean(source, kTISPropertyInputSourceIsSelected))")
            }
        }
        // main.swift exits successfully for true; preserve failures for installers.
        if !succeeded { exit(EXIT_FAILURE) }
        return true
    }

    private static func inputSources() -> [TISInputSource] {
        let filter = [kTISPropertyBundleID as String: bundleID] as CFDictionary
        guard let list = TISCreateInputSourceList(filter, true)?.takeRetainedValue() else { return [] }
        return list as! [TISInputSource]
    }

    private static func string(_ source: TISInputSource, _ key: CFString) -> String? {
        guard let value = TISGetInputSourceProperty(source, key) else { return nil }
        return Unmanaged<CFString>.fromOpaque(value).takeUnretainedValue() as String
    }

    private static func boolean(_ source: TISInputSource, _ key: CFString) -> Bool {
        guard let value = TISGetInputSourceProperty(source, key) else { return false }
        return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(value).takeUnretainedValue())
    }
}
