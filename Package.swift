// swift-tools-version: 6.0
// ZhuyinType: a native macOS Zhuyin input method.
//   ZhuyinCore        engine and composer state machine (no AppKit; testable from the command line)
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
        .executableTarget(name: "ZhuyinCoreChecks", dependencies: ["ZhuyinCore"], path: "Sources/ZhuyinCoreChecks"),
        .executableTarget(name: "zhuyin-cli", dependencies: ["ZhuyinCore"], path: "Sources/zhuyin-cli"),
        .executableTarget(
            name: "ZhuyinTypeApp",
            dependencies: ["ZhuyinCore"],
            path: "Sources/ZhuyinTypeApp",
            linkerSettings: [.linkedFramework("InputMethodKit"), .linkedFramework("Carbon")]
        ),
    ],
    swiftLanguageModes: [.v5]
)
