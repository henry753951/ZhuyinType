// swift-tools-version: 6.0
// Measurement tool for the Rust port (engine/): compiles the Swift engine's ZhuyinCore sources (symlinked, not
// copied or changed) into one executable with this directory's main.swift, which has internal access to them.
//   swift run -c release SwiftParity golden DATA TEXT OUT   reference values for the golden tests
//   swift run -c release SwiftParity bench DATA TEXT        timings matching the Rust criterion benchmarks
import PackageDescription

let package = Package(
    name: "SwiftParity",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(name: "SwiftParity", path: "Sources/SwiftParity"),
    ],
    swiftLanguageModes: [.v5]
)
