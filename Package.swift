// swift-tools-version: 6.0
// ZhuyinType: a native macOS Zhuyin input method.
//   ZhuyinCore        engine and composer state machine (no AppKit; testable from the command line)
//   ZhuyinML          the neural reranker (Core ML) that picks the displayed conversion among the N best
//   ZhuyinCoreChecks  test runner for ZhuyinCore (`swift run ZhuyinCoreChecks`; the Command Line Tools ship no XCTest)
//   zhuyin-cli        type keys into the composer from a terminal
//   ZhuyinType        the InputMethodKit app, bundled by scripts/build-app.sh
import PackageDescription

let package = Package(
    name: "ZhuyinType",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "ZhuyinCore", targets: ["ZhuyinCore"]),
        .executable(name: "ZhuyinType", targets: ["ZhuyinTypeApp"]),
        .executable(name: "zhuyin-cli", targets: ["zhuyin-cli"]),
        .executable(name: "ZhuyinCoreChecks", targets: ["ZhuyinCoreChecks"]),
    ],
    targets: [
        .target(name: "ZhuyinCore", path: "Sources/ZhuyinCore"),
        .target(name: "ZhuyinML", dependencies: ["ZhuyinCore"], path: "Sources/ZhuyinML",
                linkerSettings: [.linkedFramework("CoreML")]),
        .executableTarget(name: "ZhuyinCoreChecks", dependencies: ["ZhuyinCore"], path: "Sources/ZhuyinCoreChecks"),
        .executableTarget(name: "zhuyin-cli", dependencies: ["ZhuyinCore", "ZhuyinML"], path: "Sources/zhuyin-cli"),
        .executableTarget(
            name: "ZhuyinTypeApp",
            dependencies: ["ZhuyinCore", "ZhuyinML"],
            path: "Sources/ZhuyinTypeApp",
            linkerSettings: [.linkedFramework("InputMethodKit"), .linkedFramework("Carbon")]
        ),
    ],
    swiftLanguageModes: [.v5]
)
