#!/usr/bin/env swift
// Read-only checks; never registers, enables, selects, or installs an input source.
import AppKit
import Foundation

// The repository root: platforms/macos/scripts/ is three levels down.
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
let app = CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1]) : root.appendingPathComponent("dist/ZhuyinType.app")
let contents = app.appendingPathComponent("Contents")
let resources = contents.appendingPathComponent("Resources")
let plist = try PropertyListSerialization.propertyList(from: Data(contentsOf: contents.appendingPathComponent("Info.plist")), format: nil) as! [String: Any]
let bundleID = "org.zhuyintype.inputmethod.ZhuyinType"
let modeID = bundleID + ".Zhuyin"
precondition(plist["CFBundleIdentifier"] as? String == bundleID)
precondition(Int(plist["CFBundleVersion"] as? String ?? "") ?? 0 >= 1)
precondition((plist["CFBundleShortVersionString"] as? String)?.split(separator: ".").count == 3)
precondition(plist["LSMinimumSystemVersion"] as? String == "14.0")
precondition(plist["LSBackgroundOnly"] as? Bool == false && plist["LSUIElement"] as? Bool == true)
precondition(plist["InputMethodConnectionName"] as? String == bundleID + "_Connection")
precondition(plist["InputMethodServerControllerClass"] as? String == "ZhuyinInputController")
precondition(plist["InputMethodServerDelegateClass"] as? String == "ZhuyinInputController")
precondition(plist["TISIconIsTemplate"] as? Bool == true)
let modes = plist["ComponentInputModeDict"] as! [String: Any]
precondition(modes["tsVisibleInputModeOrderedArrayKey"] as? [String] == [modeID])
let mode = (modes["tsInputModeListKey"] as! [String: [String: Any]])[modeID]!
precondition(mode["TISInputSourceID"] as? String == modeID)
precondition(mode["TISIntendedLanguage"] as? String == "zh-Hant")
precondition(mode["tsInputModeScriptKey"] as? String == "smTradChinese")
for key in ["TISIconIsTemplate", "tsInputModeDefaultStateKey", "tsInputModePrimaryInScriptKey", "tsInputModeIsVisibleKey"] {
    precondition(mode[key] as? Bool == true, key)
}
for key in ["tsInputModeMenuIconFileKey", "tsInputModeAlternateMenuIconFileKey", "tsInputModePaletteIconFileKey"] {
    precondition(mode[key] as? String == "InputSourceTemplate.pdf", key)
}
for locale in ["en", "zh_TW", "zh-Hant"] {
    let strings = try PropertyListSerialization.propertyList(from: Data(contentsOf: resources.appendingPathComponent("\(locale).lproj/InfoPlist.strings")), format: nil) as! [String: String]
    precondition(strings[modeID] == "注音 ZhuyinType")
    precondition(strings["CFBundleName"] == (locale == "en" ? "ZhuyinType" : "ZhuyinType 注音"))
}
// The input-source badge: a 22×16 pt monochrome vector drawn black on transparent, which macOS tints as a template.
let badge = NSImage(contentsOf: resources.appendingPathComponent("InputSourceTemplate.pdf"))!
precondition(abs(badge.size.width - 22) < 0.01 && abs(badge.size.height - 16) < 0.01, "badge must be 22×16 points")
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 44, pixelsHigh: 32, bitsPerSample: 8, samplesPerPixel: 4,
                           hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
badge.draw(in: NSRect(x: 0, y: 0, width: 44, height: 32))
NSGraphicsContext.restoreGraphicsState()
var visible = 0
var transparent = 0
for y in 0..<rep.pixelsHigh {
    for x in 0..<rep.pixelsWide {
        let color = rep.colorAt(x: x, y: y)!
        if color.alphaComponent > 0.5 {
            visible += 1
            precondition(color.redComponent < 0.05 && color.greenComponent < 0.05 && color.blueComponent < 0.05)
        } else { transparent += 1 }
    }
}
precondition(visible > 40 && transparent > 40, "Monochrome artwork must have transparent background")
for name in ["reranker/vocab.json", "reranker/config.json"] {
    precondition(FileManager.default.fileExists(atPath: resources.appendingPathComponent(name).path), name)
}
precondition(["reranker/RerankerTree.mlmodelc/model.mil", "reranker/Reranker.mlmodelc/model.mil"].contains {
    FileManager.default.fileExists(atPath: resources.appendingPathComponent($0).path)
}, "reranker model")
let stray = FileManager.default.enumerator(atPath: app.path)!.compactMap { $0 as? String }
    .filter { ($0 as NSString).lastPathComponent.hasPrefix("._") }
precondition(stray.isEmpty, "AppleDouble files in the bundle: \(stray.prefix(3))")
precondition(NSImage(contentsOf: resources.appendingPathComponent("AppIcon.icns")) != nil)
precondition(FileManager.default.fileExists(atPath: resources.appendingPathComponent("lexicon.bin").path)
             || FileManager.default.fileExists(atPath: resources.appendingPathComponent("lexicon.tsv").path), "lexicon")
for name in ["Licenses/AGPL-3.0.txt", "Licenses/NOTICE.txt", "Licenses/LEXICON.txt", "Licenses/CC-BY-SA-4.0.txt", "Licenses/Unicode-3.0.txt", "Licenses/wordfreq-NOTICE.txt", "Licenses/SOURCES.md"] {
    let data = try Data(contentsOf: resources.appendingPathComponent(name))
    precondition(!data.isEmpty, name)
}
struct Sources: Decodable {
    struct Group: Decodable { let id: String }
    struct Entry: Decodable {
        let id: String, group: String, distribution: String
        let licenseFiles: [String]
        let notices: [String]?
        let platforms: [String]?
    }
    let version: Int
    let groups: [Group]
    let entries: [Entry]
}
let sources = try JSONDecoder().decode(Sources.self, from: Data(contentsOf: resources.appendingPathComponent("Licenses/sources.json")))
precondition(sources.version == 1, "sources version")
let groupIDs = Set(sources.groups.map(\.id))
precondition(groupIDs.count == sources.groups.count, "duplicate group id")
precondition(Set(sources.entries.map(\.id)).count == sources.entries.count, "duplicate source id")
func checkSourceFile(_ path: String, under directory: URL) throws {
    let url = directory.appendingPathComponent(path).standardizedFileURL
    precondition(!path.isEmpty && url.path.hasPrefix(directory.standardizedFileURL.path + "/"), "invalid source path: \(path)")
    let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
    precondition(attributes[.type] as? FileAttributeType == .typeRegular && (attributes[.size] as? NSNumber)?.int64Value ?? 0 > 0, "empty source file: \(path)")
}
for entry in sources.entries {
    precondition(groupIDs.contains(entry.group), "unknown group: \(entry.group)")
    precondition(["bundled", "training", "evaluation"].contains(entry.distribution), "unknown distribution: \(entry.distribution)")
    // Entries only the Linux and Windows client ships (Qt) have no files in this bundle.
    if entry.distribution == "bundled", entry.platforms?.contains("macos") ?? true {
        for file in entry.licenseFiles { try checkSourceFile(file, under: resources.appendingPathComponent("Licenses")) }
    }
    if entry.platforms?.contains("macos") ?? true {
        for file in entry.notices ?? [] { try checkSourceFile(file, under: resources) }
    }
}
if FileManager.default.fileExists(atPath: resources.appendingPathComponent("lm").path) {
    try checkSourceFile("lm/articles.tsv.gz", under: resources)
}
if FileManager.default.fileExists(atPath: resources.appendingPathComponent("v3").path) {
    try checkSourceFile("v3/articles.tsv.gz", under: resources)
}
let bundledLicense = try Data(contentsOf: resources.appendingPathComponent("Licenses/AGPL-3.0.txt"))
let programLicense = try Data(contentsOf: root.appendingPathComponent("LICENSE"))
precondition(bundledLicense == programLicense, "bundled program license")
let bundledNotice = try Data(contentsOf: resources.appendingPathComponent("Licenses/NOTICE.txt"))
let programNotice = try Data(contentsOf: root.appendingPathComponent("NOTICE"))
precondition(bundledNotice == programNotice, "bundled attribution")
precondition((plist["NSHumanReadableCopyright"] as? String)?.contains("Henry753951") == true, "author attribution")
let pkgInfo = try Data(contentsOf: contents.appendingPathComponent("PkgInfo"))
precondition(pkgInfo == Data("APPL????".utf8))
let status = Process()
status.executableURL = contents.appendingPathComponent("MacOS/ZhuyinType")
status.arguments = ["--status"]
let pipe = Pipe()
status.standardOutput = pipe
try status.run()
let output = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
status.waitUntilExit()
precondition(status.terminationStatus == 0 && output.contains("registered source(s)"), output)
print(output, terminator: "")
print("Packaging checks passed: metadata, localizations, template badge, reranker, licenses, --status.")
