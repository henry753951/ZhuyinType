// swift-tools-version: 6.0
// The ZhuyinType engine, shared by every client (clients/macos today; Linux and Windows later).
//   ZhuyinCore        engine and composer state machine (Foundation only; no AppKit)
//   ZhuyinML          the neural models: sentence reranker and syllable-to-character model (Core ML on macOS,
//                     ONNX Runtime on Linux through ONNXBridge)
//   ZhuyinCoreChecks  behaviour and latency checks (`swift run ZhuyinCoreChecks`)
//   zhuyin-cli        type keys into the composer from a terminal; every evaluation runs through it
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

let package = Package(
    name: "ZhuyinEngine",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "ZhuyinCore", targets: ["ZhuyinCore"]),
        .library(name: "ZhuyinML", targets: ["ZhuyinML"]),
        .executable(name: "zhuyin-cli", targets: ["zhuyin-cli"]),
        .executable(name: "ZhuyinCoreChecks", targets: ["ZhuyinCoreChecks"]),
    ],
    targets: [
        .testTarget(name: "ZhuyinMLTests", dependencies: ["ZhuyinCore", "ZhuyinML"]),
        .target(name: "ZhuyinCore", path: "Sources/ZhuyinCore"),
        .target(name: "ZhuyinML", dependencies: modelDependencies, path: "Sources/ZhuyinML",
                linkerSettings: [.linkedFramework("CoreML", .when(platforms: [.macOS]))]),
        .executableTarget(name: "ZhuyinCoreChecks", dependencies: ["ZhuyinCore"], path: "Sources/ZhuyinCoreChecks"),
        .executableTarget(name: "zhuyin-cli", dependencies: ["ZhuyinCore", "ZhuyinML"], path: "Sources/zhuyin-cli"),
    ] + modelTargets,
    swiftLanguageModes: [.v5]
)
