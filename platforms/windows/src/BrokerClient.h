#pragma once
#include <windows.h>
#include <winrt/Windows.Data.Json.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <optional>
#include <string>

namespace zt {
using Json = winrt::Windows::Data::Json::JsonObject;
struct Reply {
    bool handled = false, candidates = false, english = false;
    std::wstring commit, preedit;
    int cursor = 0;
};
// One outstanding request, no retries after writing: a reply may have drained a commit.
class BrokerClient {
public:
    ~BrokerClient() { disconnect(); }
    BrokerClient() = default;
    BrokerClient(const BrokerClient&) = delete;
    BrokerClient& operator=(const BrokerClient&) = delete;
    bool connect() noexcept;
    void disconnect(bool failed=false) noexcept;
    bool connected() const noexcept { return pipe_ != INVALID_HANDLE_VALUE; }
    bool busy() const noexcept { return busy_; }
    std::optional<Reply> request(const std::wstring& context, const wchar_t* op, Json fields) noexcept;
    static std::wstring endpoint();
private:
    bool transfer(bool write, void* data, DWORD bytes, DWORD& done, ULONGLONG deadline) noexcept;
    HANDLE pipe_ = INVALID_HANDLE_VALUE;
    ULONGLONG retryAt_ = 0;
    unsigned long long serial_ = 0;
    bool sameProcess_ = false, busy_ = false;
    using ProcessHostEvents = void(WINAPI*)();
    ProcessHostEvents processHostEvents_ = nullptr;
};
void Put(Json& json, const wchar_t* key, bool value);
void Put(Json& json, const wchar_t* key, int value);
void PutRect(Json& json, RECT rect, double scale);
}
