// swift-tools-version: 6.0
// ZhuyinType for macOS: the InputMethodKit app, bundled by scripts/build-app.sh.  The engine is Rust (engine/crates):
// scripts/build-app.sh builds its C ABI (zhuyin-ffi) as engine/target/release/libzhuyin.a first; ZhuyinEngine wraps it
// with Swift types and runs the neural models with Core ML.  Sparkle delivers updates (pinned exactly;
// Package.resolved is committed).
import PackageDescription

let rustLibrary = Context.packageDirectory + "/../../engine/target/release/libzhuyin.a"

let package = Package(
    name: "ZhuyinTypeMac",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "ZhuyinType", targets: ["ZhuyinTypeApp"]),
        .executable(name: "EngineBench", targets: ["EngineBench"]),
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.10.0"),
    ],
    targets: [
        // The engine's C ABI (engine/crates/zhuyin-ffi/include/zhuyin.h).
        .systemLibrary(name: "CZhuyin", path: "Sources/CZhuyin"),
        // The engine for the app: value types, Engine, Composer, and the models with Core ML.
        .target(
            name: "ZhuyinEngine",
            dependencies: ["CZhuyin"],
            path: "Sources/ZhuyinEngine",
            linkerSettings: [.linkedFramework("CoreML"), .unsafeFlags([rustLibrary])]
        ),
        .executableTarget(
            name: "ZhuyinTypeApp",
            dependencies: ["ZhuyinEngine", .product(name: "Sparkle", package: "Sparkle")],
            path: "Sources/ZhuyinTypeApp",
            linkerSettings: [
                .linkedFramework("InputMethodKit"), .linkedFramework("Carbon"),
                // Sparkle.framework is embedded at Contents/Frameworks.
                .unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"]),
            ]
        ),
        // The app's engine path without the app, for latency, energy and memory (Sources/EngineBench/main.swift).
        .executableTarget(name: "EngineBench", dependencies: ["ZhuyinEngine"], path: "Sources/EngineBench"),
    ],
    swiftLanguageModes: [.v5]
)
