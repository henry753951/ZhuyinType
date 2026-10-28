import Foundation

struct Metadata: Decodable {
    struct Package: Decodable {
        let id: String, name: String, version: String, manifest_path: String
        let license: String?
    }
    struct Node: Decodable {
        struct Dependency: Decodable {
            struct Kind: Decodable { let kind: String? }
            let pkg: String
            let dep_kinds: [Kind]
        }
        let id: String
        let deps: [Dependency]
    }
    struct Resolve: Decodable { let nodes: [Node] }
    let packages: [Package]
    let workspace_members: [String]
    let resolve: Resolve
}

do {
    guard CommandLine.arguments.count == 2 else {
        throw NSError(domain: "用法：swift platforms/macos/scripts/rust-licenses.swift OUTPUT", code: 1)
    }
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    var selected: [String: Metadata.Package] = [:]
    for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"] {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target]
        process.currentDirectoryURL = root.appendingPathComponent("engine")
        let pipe = Pipe()
        process.standardOutput = pipe
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw NSError(domain: "cargo metadata 失敗", code: 1) }
        let metadata = try JSONDecoder().decode(Metadata.self, from: data)
        let packages = Dictionary(uniqueKeysWithValues: metadata.packages.map { ($0.id, $0) })
        let nodes = Dictionary(uniqueKeysWithValues: metadata.resolve.nodes.map { ($0.id, $0) })
        guard let ffi = metadata.packages.first(where: { $0.name == "zhuyin-ffi" && metadata.workspace_members.contains($0.id) }) else {
            throw NSError(domain: "找不到 zhuyin-ffi", code: 1)
        }
        var pending = [ffi.id], visited = Set<String>()
        while let id = pending.popLast() {
            guard visited.insert(id).inserted, let node = nodes[id], let package = packages[id] else { continue }
            if !metadata.workspace_members.contains(id) { selected[id] = package }
            // Build dependencies are retained conservatively, including their normal dependencies.
            pending += node.deps.filter { $0.dep_kinds.contains { $0.kind != "dev" } }.map(\.pkg)
        }
    }
    var output = "Rust 套件授權（macOS，包含建置相依套件）\n"
    var missing: [String] = []
    for package in selected.values.sorted(by: { ($0.name, $0.version) < ($1.name, $1.version) }) {
        let directory = URL(fileURLWithPath: package.manifest_path).deletingLastPathComponent()
        let files = try FileManager.default.contentsOfDirectory(atPath: directory.path).filter {
            let name = $0.uppercased()
            return name.hasPrefix("LICENSE") || name.hasPrefix("COPYING") || name.hasPrefix("NOTICE")
        }.sorted()
        if !files.contains(where: { $0.uppercased().hasPrefix("LICENSE") || $0.uppercased().hasPrefix("COPYING") }) {
            missing.append("\(package.name) \(package.version)")
            continue
        }
        output += "\n===== \(package.name) \(package.version) — \(package.license ?? "未標示") =====\n"
        for file in files {
            let text = try String(contentsOf: directory.appendingPathComponent(file), encoding: .utf8)
            output += "\n--- \(file) ---\n\(text)\n"
        }
    }
    guard missing.isEmpty else { throw NSError(domain: "找不到授權檔：\(missing.joined(separator: ", "))", code: 1) }
    try output.write(toFile: CommandLine.arguments[1], atomically: true, encoding: .utf8)
} catch {
    fputs("\(error)\n", stderr)
    exit(1)
}
