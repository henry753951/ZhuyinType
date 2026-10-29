#include "BrokerClient.h"
#include "Coordinates.h"
#include <sddl.h>
#include <array>
#include <cmath>
#include <limits>
#include <stdexcept>
#include <vector>

namespace zt {
using namespace winrt::Windows::Data::Json;
namespace {
constexpr size_t maxFrame = 256 * 1024;
std::wstring Wide(const std::string& text) {
    const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
    if (!count) throw std::runtime_error("UTF-8");
    std::wstring result(count, L'\0');
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), count))
        throw std::runtime_error("UTF-8");
    return result;
}
std::string Utf8(const winrt::hstring& text) {
    const int count = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
    if (!count) throw std::runtime_error("UTF-16");
    std::string result(count, '\0');
    if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), count, nullptr, nullptr))
        throw std::runtime_error("UTF-16");
    return result;
}
bool Boundary(const std::wstring& text, size_t index) {
    return index == 0 || index == text.size() || !(text[index] >= 0xdc00 && text[index] <= 0xdfff);
}
bool WaitForIo(HANDLE event,ULONGLONG deadline,bool pump,void(WINAPI* processHostEvents)()) noexcept {
    while (true) {
        if (processHostEvents) {
            try { processHostEvents(); } catch (...) { return false; }
            if (WaitForSingleObject(event,0)==WAIT_OBJECT_0) return true;
        }
        const auto now=GetTickCount64();
        const auto remaining=now<deadline?static_cast<DWORD>(deadline-now):0;
        if (!pump) return WaitForSingleObject(event,remaining)==WAIT_OBJECT_0;
        const auto result=MsgWaitForMultipleObjectsEx(1,&event,remaining,
            QS_SENDMESSAGE|QS_POSTMESSAGE|QS_TIMER|QS_PAINT,MWMO_INPUTAVAILABLE);
        if (result==WAIT_OBJECT_0) return true;
        if (result!=WAIT_OBJECT_0+1 || now>=deadline) return false;
        // Dispatch IPC/COM/Qt wakeups, but keep keyboard, raw-input and mouse events
        // queued until this transaction completes. Never translate nested input.
        MSG message{};
        for (unsigned i=0;i<64;++i) {
            const auto flags=PM_REMOVE|PM_QS_POSTMESSAGE|PM_QS_SENDMESSAGE|PM_QS_PAINT;
            const bool found=PeekMessageW(&message,nullptr,0,WM_INPUT-1,flags) ||
                PeekMessageW(&message,nullptr,WM_KEYLAST+1,WM_MOUSEFIRST-1,flags) ||
                PeekMessageW(&message,nullptr,WM_MOUSELAST+1,0xffff,flags);
            if (!found) break;
            if (message.message==WM_QUIT) { PostQuitMessage(static_cast<int>(message.wParam)); return false; }
            DispatchMessageW(&message);
            if (WaitForSingleObject(event,0)==WAIT_OBJECT_0) return true;
            if (GetTickCount64()>=deadline) return false;
        }
    }
}
}
void Put(Json& j, const wchar_t* key, bool value) { j.Insert(key, JsonValue::CreateBooleanValue(value)); }
void Put(Json& j, const wchar_t* key, int value) { j.Insert(key, JsonValue::CreateNumberValue(value)); }
void PutRect(Json& j, RECT r, double scale) {
    const auto previous=SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    MONITORINFO monitor{sizeof(monitor)};
    const auto screen=MonitorFromRect(&r,MONITOR_DEFAULTTONEAREST);
    GetMonitorInfoW(screen,&monitor);
    if (previous) SetThreadDpiAwarenessContext(previous);
    const auto rect=QtRect(r,POINT{monitor.rcMonitor.left,monitor.rcMonitor.top},scale);
    JsonArray a;
    for (const auto n : rect) a.Append(JsonValue::CreateNumberValue(n));
    j.Insert(L"rect", a);
    j.Insert(L"scale", JsonValue::CreateNumberValue(1));
}
std::wstring BrokerClient::endpoint() {
    std::wstring value(32768, L'\0');
    const DWORD length = GetEnvironmentVariableW(L"ZT_IME_SOCKET", value.data(), static_cast<DWORD>(value.size()));
    if (length && length < value.size()) { value.resize(length); return value; }
    HANDLE token = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return {};
    DWORD bytes = 0;
    GetTokenInformation(token, TokenUser, nullptr, 0, &bytes);
    std::vector<BYTE> storage(bytes);
    const BOOL ok = GetTokenInformation(token, TokenUser, storage.data(), bytes, &bytes);
    CloseHandle(token);
    if (!ok) return {};
    LPWSTR sid = nullptr;
    if (!ConvertSidToStringSidW(reinterpret_cast<TOKEN_USER*>(storage.data())->User.Sid, &sid)) return {};
    value = L"\\\\.\\pipe\\ZhuyinType-ime-v1-";
    value += sid;
    LocalFree(sid);
    return value;
}
bool BrokerClient::connect() noexcept {
    if (connected()) return true;
    if (GetTickCount64() < retryAt_) return false;
    try {
        const auto path = endpoint();
        // Reject remote pipes; input must remain on this computer.
        if (!path.starts_with(L"\\\\.\\pipe\\")) return false;
        pipe_ = CreateFileW(path.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING,
                            FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, nullptr);
        if (!connected() && GetLastError() == ERROR_PIPE_BUSY && WaitNamedPipeW(path.c_str(), 10))
            pipe_ = CreateFileW(path.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING,
                                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, nullptr);
    } catch (...) { disconnect(true); }
    if (!connected()) retryAt_ = GetTickCount64() + 250;
    else {
        ULONG server=0;
        sameProcess_=GetNamedPipeServerProcessId(pipe_,&server) && server==GetCurrentProcessId();
        if (sameProcess_) {
            processHostEvents_=reinterpret_cast<ProcessHostEvents>(GetProcAddress(GetModuleHandleW(nullptr),"ZtProcessImeEvents"));
        }
    }
    return connected();
}
void BrokerClient::disconnect(bool failed) noexcept {
    processHostEvents_=nullptr;
    if (connected()) CloseHandle(pipe_);
    pipe_ = INVALID_HANDLE_VALUE;
    sameProcess_ = false;
    retryAt_ = failed ? GetTickCount64() + 250 : 0;
}
bool BrokerClient::transfer(bool write, void* data, DWORD bytes, DWORD& done, ULONGLONG deadline) noexcept {
    OVERLAPPED ov{};
    ov.hEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!ov.hEvent) return false;
    BOOL ok = write ? WriteFile(pipe_, data, bytes, &done, &ov) : ReadFile(pipe_, data, bytes, &done, &ov);
    if (!ok && GetLastError() == ERROR_IO_PENDING) {
        if (WaitForIo(ov.hEvent,deadline,sameProcess_,processHostEvents_))
            ok = GetOverlappedResult(pipe_, &ov, &done, FALSE);
        else {
            CancelIoEx(pipe_, &ov);
            // Keep OVERLAPPED and buffer alive until kernel cancellation has completed.
            GetOverlappedResult(pipe_, &ov, &done, TRUE);
            ok = FALSE;
        }
    }
    CloseHandle(ov.hEvent);
    return ok && done > 0;
}
std::optional<Reply> BrokerClient::request(const std::wstring& context, const wchar_t* op, Json fields) noexcept {
    if (!connected() || busy_) return {};
    struct Transaction { bool& busy; explicit Transaction(bool& b):busy(b) { busy=true; } ~Transaction() { busy=false; } } transaction(busy_);
    try {
        if (++serial_ >= (1ULL << 53)) { disconnect(); return {}; }
        const double id = static_cast<double>(serial_);
        fields.Insert(L"v", JsonValue::CreateNumberValue(1));
        fields.Insert(L"id", JsonValue::CreateNumberValue(id));
        fields.Insert(L"context", JsonValue::CreateStringValue(context));
        fields.Insert(L"op", JsonValue::CreateStringValue(op));
        auto outgoing = Utf8(fields.Stringify());
        if (outgoing.size() + 1 > maxFrame) { disconnect(); return {}; }
        outgoing += '\n';
        const auto deadline = GetTickCount64() + 200;
        std::string incoming;
        size_t offset = 0;
        while (offset < outgoing.size()) {
            if (GetTickCount64()>=deadline) throw std::runtime_error("deadline");
            DWORD done = 0;
            if (!transfer(true, outgoing.data()+offset, static_cast<DWORD>(outgoing.size()-offset), done, deadline))
                throw std::runtime_error("write");
            offset += done;
        }
        std::array<char, 4096> block{};
        while (incoming.empty() || incoming.back() != '\n') {
            if (GetTickCount64()>=deadline) throw std::runtime_error("deadline");
            DWORD done = 0;
            if (!transfer(false, block.data(), static_cast<DWORD>(block.size()), done, deadline)) throw std::runtime_error("read");
            incoming.append(block.data(), done);
            if (incoming.size() > maxFrame || incoming.find('\n') != incoming.size()-1) {
                if (incoming.find('\n') != std::string::npos || incoming.size() >= maxFrame) throw std::runtime_error("frame");
            }
        }
        if (incoming.empty() || incoming.back()!='\n' || incoming.find('\n')!=incoming.size()-1) throw std::runtime_error("frame");
        const auto response = Json::Parse(Wide(incoming));
        if (response.GetNamedNumber(L"v") != 1 || response.GetNamedNumber(L"id") != id ||
            response.GetNamedString(L"context") != context || !response.GetNamedBoolean(L"ok")) throw std::runtime_error("identity");
        if (response.HasKey(L"error")) (void)response.GetNamedString(L"error");
        Reply reply;
        reply.handled = response.GetNamedBoolean(L"handled");
        reply.candidates = response.GetNamedBoolean(L"candidates");
        reply.english = response.GetNamedBoolean(L"english");
        reply.commit = response.GetNamedString(L"commit");
        reply.preedit = response.GetNamedString(L"preedit");
        const double cursor = response.GetNamedNumber(L"cursor");
        if (!std::isfinite(cursor) || std::floor(cursor) != cursor || cursor < 0 || cursor > reply.preedit.size() ||
            !Boundary(reply.preedit, static_cast<size_t>(cursor))) throw std::runtime_error("cursor");
        reply.cursor = static_cast<int>(cursor);
        // Older hosts send no `composing`; their composition is the preedit.
        reply.composing = response.HasKey(L"composing") ? response.GetNamedBoolean(L"composing") : !reply.preedit.empty() || reply.candidates;
        return reply;
    } catch (...) { disconnect(true); return {}; }
}
}
