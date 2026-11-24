import Foundation

/// What each version changed, carried in the app (`release-notes.json`, made from docs/releases/*.md by
/// tools/release_notes.py) so an update can say it without the network (docs/product/DESIGN.md §3.6).  The same rules
/// as Linux and Windows (`zt_core::release_notes`).
struct ReleaseNotes: Decodable, Equatable {
    struct Item: Decodable, Equatable {
        /// The name of a new feature; other changes have none.
        let title: String?
        let text: String
        /// The systems it is about; empty: every one.
        let platforms: [String]

        /// In one line: the feature's name, or the first sentence.
        var headline: String {
            let line = (title ?? text.components(separatedBy: "。")[0]).trimmingCharacters(in: .whitespaces)
            return line.count <= ReleaseNotes.headlineLimit ? line : line.prefix(ReleaseNotes.headlineLimit - 1) + "…"
        }
    }

    /// 新功能, 改進 or 修正.
    struct Section: Decodable, Equatable {
        let title: String
        var items: [Item]
    }

    let version: String
    let date: String
    var sections: [Section]

    static let headlineLimit = 44
    /// How many changes the notice names.
    static let highlightLimit = 3
    static let page = URL(string: "https://github.com/henry753951/ZhuyinType/releases")!

    // MARK: Reading

    private struct File: Decodable { let releases: [ReleaseNotes] }

    /// Every release in `data`, newest first, with only what concerns macOS.
    static func parse(_ data: Data) -> [ReleaseNotes] {
        guard let file = try? JSONDecoder().decode(File.self, from: data) else { return [] }
        return file.releases.map { release in
            var release = release
            release.sections = release.sections.compactMap { section in
                var section = section
                section.items.removeAll { !$0.platforms.isEmpty && !$0.platforms.contains("macos") }
                return section.items.isEmpty ? nil : section
            }
            return release
        }
    }

    static let all: [ReleaseNotes] = Bundle.main.url(forResource: "release-notes", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }.map(parse) ?? []

    static var currentVersion: String? { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String }

    static func newer(_ candidate: String, than current: String) -> Bool {
        candidate.compare(current, options: .numeric) == .orderedDescending
    }

    /// The releases after `previous` up to `current`, newest first; only `current` when `previous` is not known (or
    /// not earlier).  An earlier one that changed nothing here is left out.
    static func between(_ releases: [ReleaseNotes], previous: String?, current: String) -> [ReleaseNotes] {
        let previous = previous.flatMap { newer(current, than: $0) ? $0 : nil }
        return releases.filter { release in
            guard let previous else { return release.version == current }
            return newer(release.version, than: previous) && !newer(release.version, than: current)
                && (release.version == current || !release.sections.isEmpty)
        }
    }

    // MARK: The notice

    struct Announcement: Equatable {
        let version: String
        let lines: [String]
        var title: String { "ZhuyinType 已更新到 \(version)" }
    }

    static func announcement(_ releases: [ReleaseNotes], current: String) -> Announcement {
        // New features first, then improvements, then fixes; the newest release's first within each.
        var lines: [String] = []
        var total = 0
        for kind in ["新功能", "改進", "修正"] {
            for item in releases.flatMap(\.sections).filter({ $0.title == kind }).flatMap(\.items) {
                total += 1
                // A fix's first sentence says what went wrong.
                if lines.count < highlightLimit { lines.append((kind == "修正" ? "修正：" : "") + item.headline) }
            }
        }
        if total == 0 { return Announcement(version: current, lines: ["這一版沒有 macOS 的變更。"]) }
        if total > lines.count {
            let rest = total - lines.count
            let versions = releases.filter { !$0.sections.isEmpty }.count
            lines.append(versions > 1 ? "\(versions) 個版本，另有 \(rest) 項" : "另有 \(rest) 項")
        }
        return Announcement(version: current, lines: lines)
    }
}

/// Which version's notes the user has been told about.
struct ReleaseNotesProgress {
    static let seenKey = "updateNotesSeenVersion"
    static let previousKey = "updateNotesPreviousVersion"

    let defaults: SettingsStorage

    /// What to do when the app starts.
    enum Start: Equatable {
        /// Announced already.
        case nothing
        /// A first install, an older version put back, or the notice turned off: remember the version, say nothing.
        case remember
        /// Announce; the version updated from, if known.
        case announce(previous: String?)
    }

    /// `usedBefore`: there are signs of earlier use, so without a version seen (0.7.6 and earlier kept none) this is
    /// an update and not a new install.
    func start(current: String, usedBefore: Bool, wanted: Bool) -> Start {
        let seen = defaults.string(forKey: Self.seenKey)
        if seen == current { return .nothing }
        if let seen, !ReleaseNotes.newer(current, than: seen) { return .remember }
        if seen == nil, !usedBefore { return .remember }
        return wanted ? .announce(previous: seen) : .remember
    }

    /// `current` was announced (or needs no announcement).
    func remember(_ current: String) {
        let seen = defaults.string(forKey: Self.seenKey)
        guard seen != current else { return }
        if let seen, ReleaseNotes.newer(current, than: seen) {
            defaults.set(seen, forKey: Self.previousKey)
        } else {
            defaults.removeObject(forKey: Self.previousKey)
        }
        defaults.set(current, forKey: Self.seenKey)
    }

    /// What 「更新內容」 lists: everything since the version before the one last announced.
    func shown(_ releases: [ReleaseNotes], current: String) -> [ReleaseNotes] {
        ReleaseNotes.between(releases, previous: defaults.string(forKey: Self.previousKey), current: current)
    }
}
