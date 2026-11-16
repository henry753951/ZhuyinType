#pragma once
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace zhuyintype {
// One outstanding request; a failed exchange is never replayed.
class Broker {
public:
    Broker() = default;
    ~Broker();
    Broker(const Broker &) = delete;
    Broker &operator=(const Broker &) = delete;
    bool connect();
    bool connected() const { return fd_ >= 0; }
    void disconnect();
    std::optional<nlohmann::json> request(const std::string &context, const char *op,
                                         nlohmann::json fields = nlohmann::json::object());
    static std::string endpoint();
    // Starts the host (`zhuyintype --background`) when nothing serves the default endpoint, at most once every few
    // seconds.  Never for an endpoint chosen with ZT_IME_SOCKET (tests run their own host).
    static void launchHost();
private:
    int fd_ = -1;
    int64_t id_ = 0;
};
}
