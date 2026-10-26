import Foundation
#if canImport(Darwin)
import Darwin
#else
import Glibc
#endif

/// Private JSON-lines socket; a server error aborts evaluation instead of silently dropping model scores.
final class ModelServer {
    private let handle: FileHandle
    private var buffer = Data()

    init(path: String) throws {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8) + [0]
        guard bytes.count <= MemoryLayout.size(ofValue: address.sun_path) else {
            throw NSError(domain: "ModelServer", code: 1, userInfo: [NSLocalizedDescriptionKey: "Socket path too long"])
        }
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes) }
        #if canImport(Darwin)
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        #else
        let fd = socket(AF_UNIX, Int32(SOCK_STREAM.rawValue), 0)
        #endif
        guard fd >= 0 else { throw POSIXError(.EIO) }
        handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        let status = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard status == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        var timeout = timeval(tv_sec: 120, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        signal(SIGPIPE, SIG_IGN)
    }

    func predict(_ request: [String: Any]) -> Any {
        do {
            var data = try JSONSerialization.data(withJSONObject: request)
            data.append(10)
            try handle.write(contentsOf: data)
            while buffer.firstIndex(of: 10) == nil {
                #if os(Linux)
                // Foundation may wait to fill the buffer on sockets; recv returns one available response fragment.
                var bytes = [UInt8](repeating: 0, count: 65536)
                let count = recv(handle.fileDescriptor, &bytes, bytes.count, 0)
                guard count > 0 else { throw POSIXError(count == 0 ? .EPIPE : (POSIXErrorCode(rawValue: errno) ?? .EIO)) }
                buffer.append(contentsOf: bytes.prefix(count))
                #else
                guard let part = try handle.read(upToCount: 65536), !part.isEmpty else { throw POSIXError(.EPIPE) }
                buffer.append(part)
                #endif
                guard buffer.count <= 4_194_304 else { throw POSIXError(.EMSGSIZE) }
            }
            let end = buffer.firstIndex(of: 10)!
            let response = try JSONSerialization.jsonObject(with: buffer[..<end]) as! [String: Any]
            buffer.removeSubrange(...end)
            if let error = response["error"] as? String { fail("model server: " + error) }
            guard let output = response["output"] else { throw POSIXError(.EPROTO) }
            return output
        } catch { fail("model server: " + error.localizedDescription) }
    }

    func rerank(_ ids: [Int32], _ targets: [Int32], _ batch: Int, _ length: Int) throws -> [Double] {
        guard let output = predict(["op": "rerank", "ids": ids, "targets": targets, "batch": batch, "length": length]) as? [Double],
              output.count == ids.count, output.allSatisfy({ $0.isFinite }) else { fail("Invalid reranker response") }
        return output
    }

    func syllable(_ context: [Int32], _ ids: [Int32], _ tones: [Int32], _ costs: [Int32], _ boundary: [Int32],
                  _ candidates: [[Int32]]) throws -> [[Double]] {
        guard let output = predict(["op": "syllable", "context": context, "ids": ids, "tones": tones,
                                    "costs": costs, "boundary": boundary, "candidates": candidates]) as? [[Double]],
              output.count == candidates.count,
              zip(output, candidates).allSatisfy({ $0.count == $1.count && $0.allSatisfy { $0.isFinite } })
        else { fail("Invalid syllable response") }
        return output
    }
}
