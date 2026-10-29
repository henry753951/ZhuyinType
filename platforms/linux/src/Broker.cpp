#include "Broker.h"
#include "Keys.h"
#include <fcitx-utils/utf8.h>
#include <chrono>
#include <algorithm>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <poll.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace zhuyintype {
namespace {
constexpr size_t maxFrame = 256 * 1024;
using Clock = std::chrono::steady_clock;
// The entire exchange, not each read/write, shares one deadline.
bool ready(int fd, short events, Clock::time_point deadline) {
    for (;;) {
        const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(deadline - Clock::now()).count();
        if (remaining <= 0) return false;
        pollfd p{fd, events, 0};
        const int result = poll(&p, 1, static_cast<int>(remaining));
        if (result < 0 && errno == EINTR) continue;
        return result > 0 && (p.revents & events);
    }
}
bool valid(const nlohmann::json &r, int64_t id, const std::string &context) {
    if (!r.is_object() || !r.contains("v") || !r["v"].is_number_integer() || r["v"] != 1 ||
        !r.contains("id") || !r["id"].is_number_integer() || r["id"] != id ||
        !r.contains("context") || r["context"] != context) return false;
    for (const auto *key : {"ok", "handled", "candidates", "english"})
        if (!r.contains(key) || !r[key].is_boolean()) return false;
    for (const auto *key : {"commit", "preedit"}) {
        if (!r.contains(key) || !r[key].is_string()) return false;
        const auto &s = r[key].get_ref<const std::string &>();
        if (s.find('\0') != std::string::npos || !fcitx::utf8::validate(s)) return false;
    }
    // A successful host snapshot may omit an empty error message.
    if (r.contains("error") && (!r["error"].is_string() ||
        !fcitx::utf8::validate(r["error"].get_ref<const std::string &>()))) return false;
    return r.contains("cursor") && r["cursor"].is_number_integer() && r["cursor"] >= 0 &&
        r["cursor"] <= maxFrame && cursorBytes(r["preedit"].get_ref<const std::string &>(), r["cursor"].get<size_t>()).has_value();
}
}
Broker::~Broker() { disconnect(); }
void Broker::disconnect() { if (fd_ >= 0) close(fd_); fd_ = -1; }
std::string Broker::endpoint() {
    if (const char *path = std::getenv("ZT_IME_SOCKET"); path && *path) return path;
    if (const char *path = std::getenv("XDG_RUNTIME_DIR"); path && *path == '/')
        return std::string(path) + "/zhuyintype/ime-v1";
    return "/tmp/zhuyintype-" + std::to_string(getuid()) + "/ime-v1";
}
bool Broker::connect() {
    if (connected()) return true;
    const auto path = endpoint();
    sockaddr_un address{};
    if (path.empty() || path.front() != '/' || path.size() >= sizeof(address.sun_path)) return false;
    address.sun_family = AF_UNIX;
    std::memcpy(address.sun_path, path.c_str(), path.size() + 1);
    fd_ = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd_ < 0) return false;
    const auto deadline = Clock::now() + std::chrono::milliseconds(80);
    const int result = ::connect(fd_, reinterpret_cast<sockaddr *>(&address), sizeof(address));
    if (result < 0 && (errno != EINPROGRESS || !ready(fd_, POLLOUT, deadline))) { disconnect(); return false; }
    int error = 0;
    socklen_t length = sizeof(error);
    ucred peer{};
    socklen_t peerLength = sizeof(peer);
    if (getsockopt(fd_, SOL_SOCKET, SO_ERROR, &error, &length) || error ||
        getsockopt(fd_, SOL_SOCKET, SO_PEERCRED, &peer, &peerLength) || peer.uid != getuid()) {
        disconnect(); return false;
    }
    return true;
}
std::optional<nlohmann::json> Broker::request(const std::string &context, const char *op, nlohmann::json fields) {
    if (!connected()) return std::nullopt;
    fields["v"] = 1; fields["id"] = ++id_; fields["context"] = context; fields["op"] = op;
    const auto wire = fields.dump() + '\n';
    const auto deadline = Clock::now() + std::chrono::milliseconds(80);
    std::string response;
    size_t sent = 0;
    while (wire.size() <= maxFrame && sent < wire.size()) {
        if (!ready(fd_, POLLOUT, deadline)) break;
        const auto n = send(fd_, wire.data() + sent, wire.size() - sent, MSG_NOSIGNAL);
        if (n < 0 && (errno == EINTR || errno == EAGAIN)) continue;
        if (n <= 0) break;
        sent += static_cast<size_t>(n);
    }
    while (sent == wire.size() && response.size() < maxFrame && ready(fd_, POLLIN, deadline)) {
        char buffer[4096];
        const auto n = recv(fd_, buffer, std::min(sizeof(buffer), maxFrame - response.size()), 0);
        if (n < 0 && (errno == EINTR || errno == EAGAIN)) continue;
        if (n <= 0) break;
        response.append(buffer, static_cast<size_t>(n));
        const auto newline = response.find('\n');
        if (newline == std::string::npos) continue;
        if (newline + 1 != response.size()) break; // unsolicited/extra frames invalidate this stream
        const auto parsed = nlohmann::json::parse(response, nullptr, false);
        if (valid(parsed, id_, context) && parsed["ok"].get<bool>()) return std::optional<nlohmann::json>(std::in_place, parsed);
        break;
    }
    disconnect();
    return std::nullopt;
}
}
