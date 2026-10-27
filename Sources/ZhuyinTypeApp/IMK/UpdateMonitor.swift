import CoreServices
import Foundation

/// Watches directories (recursively, through FSEvents) and calls back on the main thread once changes settle: an
/// installer replaces many files in a row, and the callback should see the finished result.
final class UpdateMonitor {
    private var stream: FSEventStreamRef?
    private var pending: DispatchWorkItem?
    private let settle: TimeInterval
    private let onChange: () -> Void

    init(paths: [String], settle: TimeInterval = 2, onChange: @escaping () -> Void) {
        self.settle = settle
        self.onChange = onChange
        var context = FSEventStreamContext(version: 0, info: Unmanaged.passUnretained(self).toOpaque(),
                                           retain: nil, release: nil, copyDescription: nil)
        let callback: FSEventStreamCallback = { _, info, _, _, _, _ in
            guard let info else { return }
            Unmanaged<UpdateMonitor>.fromOpaque(info).takeUnretainedValue().changed()
        }
        // Watching a directory that does not exist yet (the models folder) is fine: FSEvents reports it once created.
        stream = FSEventStreamCreate(nil, callback, &context, paths as CFArray,
                                     FSEventStreamEventId(kFSEventStreamEventIdSinceNow), 0.5,
                                     FSEventStreamCreateFlags(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagNoDefer))
        guard let stream else { return }
        FSEventStreamSetDispatchQueue(stream, .main)
        FSEventStreamStart(stream)
    }

    deinit {
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
    }

    private func changed() {
        pending?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.onChange() }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + settle, execute: work)
    }
}
