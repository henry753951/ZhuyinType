// swift-tools-version: 6.0
// ZhuyinType: a native macOS Zhuyin input method.
//   ZhuyinCore        engine and composer state machine (no AppKit; testable from the command line)
//   ZhuyinML          the neural reranker (Core ML) that picks the displayed conversion among the N best
//   ZhuyinCoreChecks  test runner for ZhuyinCore (`swift run ZhuyinCoreChecks`; the Command Line Tools ship no XCTest)
//   zhuyin-cli        type keys into the composer from a terminal
//   ZhuyinType        the InputMethodKit app, bundled by scripts/build-app.sh
import PackageDescription
import Foundation

#if os(Linux)
let ortRoot = ProcessInfo.processInfo.environment["ONNXRUNTIME_ROOT"] ?? NSHomeDirectory() + "/opt/onnxruntime"
let modelDependencies: [Target.Dependency] = ["ZhuyinCore", "ONNXBridge"]
let modelTargets: [Target] = [.target(name: "ONNXBridge", path: "Sources/ONNXBridge",
    cSettings: [.unsafeFlags(["-I" + ortRoot + "/include"])],
    linkerSettings: [.unsafeFlags(["-L" + ortRoot + "/lib", "-Xlinker", "-rpath", "-Xlinker", ortRoot + "/lib"]),
                     .linkedLibrary("onnxruntime")])]
#else
let modelDependencies: [Target.Dependency] = ["ZhuyinCore"]
let modelTargets: [Target] = []
#endif

#if os(macOS)
let appProducts: [Product] = [.executable(name: "ZhuyinType", targets: ["ZhuyinTypeApp"])]
let appTargets: [Target] = [.executableTarget(
    name: "ZhuyinTypeApp", dependencies: ["ZhuyinCore", "ZhuyinML"], path: "Sources/ZhuyinTypeApp",
    linkerSettings: [.linkedFramework("InputMethodKit"), .linkedFramework("Carbon")])]
#else
let appProducts: [Product] = []
let appTargets: [Target] = []
#endif

let package = Package(
    name: "ZhuyinType",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "ZhuyinCore", targets: ["ZhuyinCore"]),
        .executable(name: "zhuyin-cli", targets: ["zhuyin-cli"]),
        .executable(name: "ZhuyinCoreChecks", targets: ["ZhuyinCoreChecks"]),
    ] + appProducts,
    targets: [
        .testTarget(name: "ZhuyinMLTests", dependencies: ["ZhuyinCore", "ZhuyinML"]),
        .target(name: "ZhuyinCore", path: "Sources/ZhuyinCore"),
        .target(name: "ZhuyinML", dependencies: modelDependencies, path: "Sources/ZhuyinML",
                linkerSettings: [.linkedFramework("CoreML", .when(platforms: [.macOS]))]),
        .executableTarget(name: "ZhuyinCoreChecks", dependencies: ["ZhuyinCore"], path: "Sources/ZhuyinCoreChecks"),
        .executableTarget(name: "zhuyin-cli", dependencies: ["ZhuyinCore", "ZhuyinML"], path: "Sources/zhuyin-cli"),
    ] + appTargets + modelTargets,
    swiftLanguageModes: [.v5]
)
