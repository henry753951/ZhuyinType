// STUB — replaced by the packaging implementation.  The signature below is the contract; keep it.
import Foundation

enum InstallCommand {
    /// Handles `--register`, `--enable`, `--unregister` (and anything else this command line tool needs) before the
    /// input method server starts.  Returns true when it handled the arguments and the process should exit.
    static func handle(arguments: [String]) -> Bool { false }
}
