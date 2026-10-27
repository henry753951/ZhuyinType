import Foundation
#if canImport(Darwin)
import Darwin
#else
import Glibc
#endif

/// The models through `zt`'s model server (ml/src/zhuyintype_ml/remote/model_server.py) on a private Unix socket, in
/// its binary frames: an 8-byte little-endian header (marker 0, kind, version 1 as u16, payload bytes as u32), then
/// int32 values in requests and float32 values in replies.  Requests: kind 1 reranker (B, L, ids, targets), kind 2
/// character model (C, N, W, context, ids, tones, costs, boundary, candidates as N rows of W).  A reply of kind 1
/// carries an error message.  A server error aborts evaluation instead of silently dropping model scores.
final class ModelServer {
    private let fd: Int32
    private var buffer: [UInt8] = []

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
        self.fd = fd
        let status = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard status == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        var timeout = timeval(tv_sec: 120, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        signal(SIGPIPE, SIG_IGN)
    }

    deinit { close(fd) }

    private func sendAll(_ bytes: [UInt8]) throws {
        var sent = 0
        while sent < bytes.count {
            let count = bytes.withUnsafeBytes { write(fd, $0.baseAddress! + sent, bytes.count - sent) }
            guard count > 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
            sent += count
        }
    }

    /// Reads until `buffer` holds at least `count` bytes.
    private func fill(_ count: Int) throws {
        var chunk = [UInt8](repeating: 0, count: 65536)
        while buffer.count < count {
            let read = recv(fd, &chunk, chunk.count, 0)
            guard read > 0 else { throw POSIXError(read == 0 ? .EPIPE : (POSIXErrorCode(rawValue: errno) ?? .EIO)) }
            buffer.append(contentsOf: chunk.prefix(read))
        }
    }

    private static func le32(_ bytes: ArraySlice<UInt8>) -> UInt32 {
        bytes.reversed().reduce(UInt32(0)) { $0 << 8 | UInt32($1) }
    }

    /// One request (kind, int32 values) and its reply (float32 values).
    func call(_ kind: UInt8, _ values: [Int32]) -> [Float] {
        do {
            let length = UInt32(values.count * 4)
            var frame: [UInt8] = [0, kind, 1, 0]
            frame.reserveCapacity(8 + values.count * 4)
            for shift in stride(from: 0, to: 32, by: 8) { frame.append(UInt8(truncatingIfNeeded: length >> UInt32(shift))) }
            values.map(\.littleEndian).withUnsafeBytes { frame.append(contentsOf: $0) }
            try sendAll(frame)
            try fill(8)
            guard buffer[0] == 0, buffer[2] == 1, buffer[3] == 0 else { throw POSIXError(.EPROTO) }
            let status = buffer[1]
            let size = Int(Self.le32(buffer[4..<8]))
            guard size <= 16_777_216 else { throw POSIXError(.EMSGSIZE) }
            try fill(8 + size)
            let payload = buffer[8..<(8 + size)]
            defer { buffer.removeSubrange(0..<(8 + size)) }
            if status == 1 { fail("model server: " + String(decoding: payload, as: UTF8.self)) }
            guard status == 0, size % 4 == 0 else { throw POSIXError(.EPROTO) }
            return payload.withUnsafeBytes { raw in
                (0..<(size / 4)).map { Float(bitPattern: UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: $0 * 4, as: UInt32.self))) }
            }
        } catch { fail("model server: " + error.localizedDescription) }
    }

    func rerank(_ ids: [Int32], _ targets: [Int32], _ batch: Int, _ length: Int) throws -> [Double] {
        let output = call(1, [Int32(batch), Int32(length)] + ids + targets)
        guard output.count == ids.count, output.allSatisfy({ $0.isFinite }) else { fail("Invalid reranker response") }
        return output.map(Double.init)
    }

    func syllable(_ context: [Int32], _ ids: [Int32], _ tones: [Int32], _ costs: [Int32], _ boundary: [Int32],
                  _ candidates: [[Int32]]) throws -> [[Double]] {
        let width = candidates.first?.count ?? 0
        guard width > 0, candidates.allSatisfy({ $0.count == width }) else { fail("Invalid syllable request") }
        let output = call(2, [Int32(context.count), Int32(ids.count), Int32(width)] + context + ids + tones + costs
                          + boundary + candidates.flatMap { $0 })
        guard output.count == candidates.count * width, output.allSatisfy({ $0.isFinite }) else {
            fail("Invalid syllable response")
        }
        return (0..<candidates.count).map { row in output[(row * width)..<((row + 1) * width)].map(Double.init) }
    }
}
