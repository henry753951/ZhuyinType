// swift-tools-version: 6.0
// ZhuyinType for macOS: the InputMethodKit app, bundled by scripts/build-app.sh.  The engine is the shared package in
// nlp/engine; Sparkle delivers updates (pinned exactly; Package.resolved is committed).
import PackageDescription

let package = Package(
    name: "ZhuyinTypeMac",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "ZhuyinType", targets: ["ZhuyinTypeApp"])],
    dependencies: [
        .package(path: "../../nlp/engine"),
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.10.0"),
    ],
    targets: [
        .executableTarget(
            name: "ZhuyinTypeApp",
            dependencies: [
                .product(name: "ZhuyinCore", package: "engine"),
                .product(name: "ZhuyinML", package: "engine"),
                .product(name: "Sparkle", package: "Sparkle"),
            ],
            path: "Sources/ZhuyinTypeApp",
            linkerSettings: [
                .linkedFramework("InputMethodKit"), .linkedFramework("Carbon"),
                // Sparkle.framework is embedded at Contents/Frameworks.
                .unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"]),
            ]
        ),
    ],
    swiftLanguageModes: [.v5]
)
