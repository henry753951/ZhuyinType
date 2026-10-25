#!/usr/bin/env swift
// Builds the app icon and the input-source menu icon from the ㄓ glyph of PingFang TC.
//   swift scripts/make_icons.swift          (needs Xcode for actool)
// Writes:
//   Resources/AppIcon.icon/          Icon Composer source: a blue background and ㄓ on a Liquid Glass layer
//   Resources/Assets.car             compiled icon, rendered by macOS 26 and later with the system glass material
//   Resources/AppIcon.icns           the same icon for earlier systems
//   Resources/InputSourceTemplate.tiff  menu bar icon (template), 16 pt at 1× and 2×
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
func glyphPath(height: CGFloat, center: CGPoint) -> CGPath {
    let font = CTFontCreateWithName("PingFangTC-Semibold" as CFString, 1000, nil)
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

/// A filled rounded square with ㄓ cut out, black on clear, which macOS tints as a template image.
func menuIcon(points: CGFloat, scale: CGFloat) -> Data {
    let pixels = Int(points * scale)
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .calibratedRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: points, height: points)
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    let box = NSRect(x: 1, y: 1, width: points - 2, height: points - 2)
    NSColor.black.setFill()
    NSBezierPath(roundedRect: box, xRadius: 3.5, yRadius: 3.5).fill()
    let cg = context.cgContext
    cg.setBlendMode(.destinationOut)
    cg.addPath(glyphPath(height: points * 0.58, center: CGPoint(x: box.midX, y: box.midY)))
    cg.fillPath()
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let menuFiles = [(1, "InputSource.png"), (2, "InputSource@2x.png")].map { scale, name -> String in
    let url = compiled.appendingPathComponent(name)
    try! menuIcon(points: 16, scale: CGFloat(scale)).write(to: url)
    return url.path
}
try run("/usr/bin/tiffutil", ["-cathidpicheck"] + menuFiles + ["-out", resources.appendingPathComponent("InputSourceTemplate.tiff").path])
try? fileManager.removeItem(at: compiled)
print("Generated AppIcon.icon, Assets.car, AppIcon.icns and InputSourceTemplate.tiff")
