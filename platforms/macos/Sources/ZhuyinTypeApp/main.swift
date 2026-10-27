import AppKit
import InputMethodKit

// Headless modes never take the keyboard from the app the user is in.
if !Headless.flags.isDisjoint(with: CommandLine.arguments) { Headless.begin() }

if InstallCommand.handle(arguments: CommandLine.arguments) { exit(0) }

let arguments = CommandLine.arguments
if let index = arguments.firstIndex(where: { $0 == "--render-ui" || $0 == "--render-ui-preview" }) {
    guard arguments.indices.contains(index + 1) else {
        fputs("--render-ui requires an output directory\n", stderr)
        exit(2)
    }
    _ = NSApplication.shared
    exit(UIPreviewRenderer.render(to: URL(fileURLWithPath: arguments[index + 1]),
                                  pages: arguments.firstIndex(of: "--only").flatMap { arguments.indices.contains($0 + 1) ? arguments[$0 + 1] : nil })
         ? 0 : 1)
}

// `--setup-wizard`: open the setup wizard (for the installer's last step), then run as the input method.
if arguments.contains("--setup-wizard") {
    DispatchQueue.main.async { WizardWindowController.shared.show(.quick) }
}

// Development: open the settings window with sample words, print its window number, and keep running.
if let index = arguments.firstIndex(of: "--settings-demo") {
    let application = NSApplication.shared
    application.setActivationPolicy(.regular)
    if arguments.contains("--dark") { application.appearance = NSAppearance(named: .darkAqua) }
    let pane = arguments.indices.contains(index + 1) ? SettingsPane(rawValue: arguments[index + 1]) : nil
    if let log = try? UIPreviewRenderer.sampleCorrectionLog() { CorrectionLog.shared = log }
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
