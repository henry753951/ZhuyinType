// swift-tools-version: 6.0
// ZhuyinType for macOS: the InputMethodKit app, bundled by scripts/build-app.sh.  The engine is the shared package in
// engine/swift; Sparkle delivers updates (pinned exactly; Package.resolved is committed).
//
// ZHUYIN_ENGINE=rust builds the app on the Rust engine instead (engine/crates/zhuyin-ffi, as the static library
// engine/target/release/libzhuyin.a, which scripts/build-app.sh builds first): the ZhuyinRust module wraps its C ABI
// with the API the app uses, and Core ML stays here.  Without it the app uses the Swift engine.
import Foundation
import PackageDescription

let rustEngine = ProcessInfo.processInfo.environment["ZHUYIN_ENGINE"] == "rust"
let rustLibrary = Context.packageDirectory + "/../../engine/target/release/libzhuyin.a"
let engineTargets: [Target] = rustEngine ? [
    .systemLibrary(name: "CZhuyin", path: "Sources/CZhuyin"),
    .target(name: "ZhuyinRust", dependencies: ["CZhuyin", .product(name: "ZhuyinCore", package: "swift")],
            path: "Sources/ZhuyinRust", linkerSettings: [.linkedFramework("CoreML")]),
] : []

let package = Package(
    name: "ZhuyinTypeMac",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "ZhuyinType", targets: ["ZhuyinTypeApp"]),
        .executable(name: "EngineBench", targets: ["EngineBench"]),
    ],
    dependencies: [
        .package(path: "../../engine/swift"),
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.10.0"),
    ],
    targets: [
        .executableTarget(
            name: "ZhuyinTypeApp",
            dependencies: [
                .product(name: "ZhuyinCore", package: "swift"),
                .product(name: "ZhuyinML", package: "swift"),
                .product(name: "Sparkle", package: "Sparkle"),
            ] + (rustEngine ? ["ZhuyinRust"] : []),
            path: "Sources/ZhuyinTypeApp",
            swiftSettings: rustEngine ? [.define("ZHUYIN_RUST_ENGINE")] : [],
            linkerSettings: [
                .linkedFramework("InputMethodKit"), .linkedFramework("Carbon"),
                // Sparkle.framework is embedded at Contents/Frameworks.
                .unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"]
                             + (rustEngine ? [rustLibrary] : [])),
            ]
        ),
        // The app's engine path without the app, for latency, energy and memory (Sources/EngineBench/main.swift).
        .executableTarget(
            name: "EngineBench",
            dependencies: [
                .product(name: "ZhuyinCore", package: "swift"),
                .product(name: "ZhuyinML", package: "swift"),
            ] + (rustEngine ? ["ZhuyinRust"] : []),
            path: "Sources/EngineBench",
            swiftSettings: rustEngine ? [.define("ZHUYIN_RUST_ENGINE")] : [],
            linkerSettings: rustEngine ? [.unsafeFlags([rustLibrary])] : []
        ),
    ] + engineTargets,
    swiftLanguageModes: [.v5]
)
