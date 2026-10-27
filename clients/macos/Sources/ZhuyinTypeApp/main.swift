import AppKit
import InputMethodKit

if InstallCommand.handle(arguments: CommandLine.arguments) { exit(0) }

let arguments = CommandLine.arguments
if let index = arguments.firstIndex(of: "--render-ui-preview") {
    guard arguments.indices.contains(index + 1) else {
        fputs("--render-ui-preview requires an output directory\n", stderr)
        exit(2)
    }
    _ = NSApplication.shared
    UIPreviewRenderer.render(to: URL(fileURLWithPath: arguments[index + 1]))
    exit(0)
}

// Development: open the settings window with sample words, print its window number, and keep running.
if let index = arguments.firstIndex(of: "--settings-demo") {
    let application = NSApplication.shared
    application.setActivationPolicy(.regular)
    if arguments.contains("--dark") { application.appearance = NSAppearance(named: .darkAqua) }
    let pane = arguments.indices.contains(index + 1) ? SettingsPane(rawValue: arguments[index + 1]) : nil
    SettingsWindowController.shared.engine = try? UIPreviewRenderer.sampleEngine()
    let window = SettingsWindowController.shared.makeWindow(pane: pane)
    DispatchQueue.main.async {
        application.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        print(window.windowNumber)
        fflush(stdout)
    }
    application.run()
}

if arguments.contains("--self-test") {
    var lexiconURL = Bundle.main.url(forResource: "lexicon", withExtension: "tsv")
        ?? URL(fileURLWithPath: "data/build/lexicon.tsv")
    if let index = arguments.firstIndex(of: "--lexicon") {
        guard arguments.indices.contains(index + 1) else {
            fputs("--lexicon requires a path\n", stderr)
            exit(2)
        }
        lexiconURL = URL(fileURLWithPath: arguments[index + 1])
    }
    exit(ControllerCoreSelfTest.run(lexiconURL: lexiconURL))
}

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let inputMethodServer = IMKServer(name: EngineHost.connectionName, bundleIdentifier: Bundle.main.bundleIdentifier)
EngineHost.load()
Updater.shared.start()
withExtendedLifetime(inputMethodServer) { application.run() }
