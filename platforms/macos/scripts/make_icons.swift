#!/usr/bin/env swift
// Builds the app icon and the input-source menu icon from the ㄓ glyph of PingFang TC.
//   swift scripts/make_icons.swift          (needs Xcode for actool)
// Writes:
//   Resources/AppIcon.icon/          Icon Composer source: a blue background and ㄓ on a Liquid Glass layer
//   Resources/Assets.car             compiled icon, rendered by macOS 26 and later with the system glass material
//   Resources/AppIcon.icns           the same icon for earlier systems
//   Resources/InputSourceTemplate.pdf   input-source icon (template), 22 × 16 pt vector
import AppKit
import CoreText

let root = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent().deletingLastPathComponent()
let resources = root.appendingPathComponent("Resources")
let fileManager = FileManager.default

func run(_ executable: String, _ arguments: [String]) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = arguments
    try process.run()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw CocoaError(.executableLoad) }
}

/// The outline of ㄓ, scaled so its bounding box is `height` tall and centred on `center`.
func glyphPath(height: CGFloat, center: CGPoint, font name: String = "PingFangTC-Semibold") -> CGPath {
    let font = CTFontCreateWithName(name as CFString, 1000, nil)
    var character: UniChar = 0x3113
    var glyph: CGGlyph = 0
    CTFontGetGlyphsForCharacters(font, &character, &glyph, 1)
    let outline = CTFontCreatePathForGlyph(font, glyph, nil)!
    let box = outline.boundingBoxOfPath
    let scale = height / box.height
    var transform = CGAffineTransform(translationX: center.x, y: center.y)
        .scaledBy(x: scale, y: scale)
        .translatedBy(x: -box.midX, y: -box.midY)
    return outline.copy(using: &transform)!
}

// MARK: - App icon (Icon Composer)

/// SVG path data for `path` in a canvas `size` high (SVG's y axis points down).
func svgPathData(_ path: CGPath, canvas size: CGFloat) -> String {
    var d = ""
    func p(_ point: CGPoint) -> String { String(format: "%.2f %.2f", point.x, size - point.y) }
    path.applyWithBlock { element in
        let e = element.pointee
        switch e.type {
        case .moveToPoint: d += "M\(p(e.points[0]))"
        case .addLineToPoint: d += "L\(p(e.points[0]))"
        case .addQuadCurveToPoint: d += "Q\(p(e.points[0])) \(p(e.points[1]))"
        case .addCurveToPoint: d += "C\(p(e.points[0])) \(p(e.points[1])) \(p(e.points[2]))"
        case .closeSubpath: d += "Z"
        @unknown default: break
        }
    }
    return d
}

let iconSource = resources.appendingPathComponent("AppIcon.icon")
try? fileManager.removeItem(at: iconSource)
try fileManager.createDirectory(at: iconSource.appendingPathComponent("Assets"), withIntermediateDirectories: true)
let glyph = glyphPath(height: 470, center: CGPoint(x: 512, y: 512))
let svg = """
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
<path fill="#FFFFFF" d="\(svgPathData(glyph, canvas: 1024))"/>
</svg>
"""
try svg.write(to: iconSource.appendingPathComponent("Assets/zhuyin.svg"), atomically: true, encoding: .utf8)
let iconJSON = """
{
  "fill" : {
    "automatic-gradient" : "extended-srgb:0.22353,0.41176,0.98039,1.00000"
  },
  "groups" : [
    {
      "layers" : [
        {
          "glass" : true,
          "hidden" : false,
          "image-name" : "zhuyin.svg",
          "name" : "zhuyin",
          "position" : {
            "scale" : 1,
            "translation-in-points" : [ 0, 0 ]
          }
        }
      ],
      "shadow" : {
        "kind" : "neutral",
        "opacity" : 0.5
      },
      "translucency" : {
        "enabled" : true,
        "value" : 0.4
      }
    }
  ],
  "supported-platforms" : {
    "squares" : [
      "macOS"
    ]
  }
}
"""
try iconJSON.write(to: iconSource.appendingPathComponent("icon.json"), atomically: true, encoding: .utf8)

let compiled = fileManager.temporaryDirectory.appendingPathComponent("ZhuyinTypeIcon-\(UUID().uuidString)")
try fileManager.createDirectory(at: compiled, withIntermediateDirectories: true)
try run("/usr/bin/xcrun", ["actool", iconSource.path, "--compile", compiled.path, "--platform", "macosx",
                           "--minimum-deployment-target", "14.0", "--app-icon", "AppIcon",
                           "--output-partial-info-plist", compiled.appendingPathComponent("partial.plist").path,
                           "--output-format", "human-readable-text", "--notices", "--warnings", "--errors"])
for name in ["Assets.car", "AppIcon.icns"] {
    let target = resources.appendingPathComponent(name)
    try? fileManager.removeItem(at: target)
    try fileManager.copyItem(at: compiled.appendingPathComponent(name), to: target)
}
try? fileManager.removeItem(at: resources.appendingPathComponent("AppIcon.iconset"))

// MARK: - Menu bar icon

/// The input-source badge, drawn like the system's own ABC badge: a 22 × 16 pt rectangle with 4.5 pt corners and the
/// glyph cut out in 1.5 pt strokes, 10 pt tall.  The font's ㄓ comes out thin and blurred at this size, so the strokes
/// are drawn as rectangles on the half-point grid, which keeps every edge on a pixel at 2×; the glyph sits a quarter
/// point left of centre so the stem can.  Black on clear; Info.plist marks it as a template, so macOS tints it for
/// the menu bar, the input menu and the indicator by the insertion point.
let badgeSize = CGSize(width: 22, height: 16)
let badgeURL = resources.appendingPathComponent("InputSourceTemplate.pdf")
var mediaBox = CGRect(origin: .zero, size: badgeSize)
let pdf = CGContext(badgeURL as CFURL, mediaBox: &mediaBox, nil)!
pdf.beginPDFPage(nil)
let badge = CGMutablePath()
badge.addRoundedRect(in: mediaBox, cornerWidth: 4.5, cornerHeight: 4.5)
// ㄓ, measured from the top-left corner.
let strokes = [
    CGRect(x: 10, y: 3, width: 1.5, height: 10),       // stem
    CGRect(x: 5.5, y: 4.5, width: 1.5, height: 5),     // left post
    CGRect(x: 14.5, y: 4.5, width: 1.5, height: 5),    // right post
    CGRect(x: 5.5, y: 8, width: 10.5, height: 1.5),    // cup
    CGRect(x: 4.5, y: 11.5, width: 12.5, height: 1.5), // base
]
let glyphStrokes = CGMutablePath()
for stroke in strokes {
    glyphStrokes.addRect(CGRect(x: stroke.minX, y: badgeSize.height - stroke.maxY, width: stroke.width, height: stroke.height))
}
// Union the strokes first, so the even-odd fill cuts them out cleanly where they cross.
badge.addPath(glyphStrokes.normalized(using: .winding))
pdf.addPath(badge)
pdf.setFillColor(CGColor(gray: 0, alpha: 1))
pdf.fillPath(using: .evenOdd)
pdf.endPDFPage()
pdf.closePDF()
try? fileManager.removeItem(at: resources.appendingPathComponent("InputSourceTemplate.tiff"))
try? fileManager.removeItem(at: compiled)
print("Generated AppIcon.icon, Assets.car, AppIcon.icns and InputSourceTemplate.pdf")
