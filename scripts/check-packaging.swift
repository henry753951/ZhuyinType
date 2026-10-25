#!/usr/bin/env swift
// Read-only checks; never registers, enables, selects, or installs an input source.
import AppKit
import Foundation

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let app = CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1]) : root.appendingPathComponent("dist/ZhuyinType.app")
let contents = app.appendingPathComponent("Contents")
let resources = contents.appendingPathComponent("Resources")
let plist = try PropertyListSerialization.propertyList(from: Data(contentsOf: contents.appendingPathComponent("Info.plist")), format: nil) as! [String: Any]
let bundleID = "org.zhuyintype.inputmethod.ZhuyinType"
let modeID = bundleID + ".Zhuyin"
precondition(plist["CFBundleIdentifier"] as? String == bundleID)
precondition(plist["CFBundleVersion"] as? String == "1")
precondition(plist["CFBundleShortVersionString"] as? String == "0.1.0")
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
    precondition(mode[key] as? String == "InputSourceTemplate.tiff", key)
}
for locale in ["en", "zh_TW", "zh-Hant"] {
    let strings = try PropertyListSerialization.propertyList(from: Data(contentsOf: resources.appendingPathComponent("\(locale).lproj/InfoPlist.strings")), format: nil) as! [String: String]
    precondition(strings[modeID] == "注音 ZhuyinType")
    precondition(strings["CFBundleName"] == (locale == "en" ? "ZhuyinType" : "ZhuyinType 注音"))
}
let tiff = try Data(contentsOf: resources.appendingPathComponent("InputSourceTemplate.tiff"))
let reps = NSBitmapImageRep.imageReps(with: tiff).compactMap { $0 as? NSBitmapImageRep }
precondition(reps.count == 2 && Set(reps.map(\.pixelsWide)) == [16, 32])
for rep in reps {
    precondition(rep.pixelsWide == rep.pixelsHigh && rep.hasAlpha)
    precondition(abs(rep.size.width - 16) < 0.01, "Both TIFF representations must be 16 points")
    var visible = 0
    var transparent = 0
    for y in 0..<rep.pixelsHigh {
        for x in 0..<rep.pixelsWide {
            let color = rep.colorAt(x: x, y: y)!.usingColorSpace(.deviceRGB)!
            if color.alphaComponent > 0.1 {
                visible += 1
                precondition(color.redComponent < 0.01 && color.greenComponent < 0.01 && color.blueComponent < 0.01)
            } else { transparent += 1 }
        }
    }
    precondition(visible > 40 && transparent > 40, "Monochrome artwork must have transparent background")
}
precondition(NSImage(contentsOf: resources.appendingPathComponent("AppIcon.icns")) != nil)
for name in ["lexicon.tsv", "Licenses/AGPL-3.0.txt", "Licenses/LEXICON.txt", "Licenses/CC-BY-SA-4.0.txt", "Licenses/Unicode-3.0.txt", "Licenses/wordfreq-NOTICE.txt", "Licenses/SOURCES.md"] {
    let data = try Data(contentsOf: resources.appendingPathComponent(name))
    precondition(!data.isEmpty, name)
}
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
print("Packaging checks passed: metadata, localizations, 1×/2× template TIFF, licenses, --status.")
