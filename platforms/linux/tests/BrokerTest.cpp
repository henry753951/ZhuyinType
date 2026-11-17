#include "Broker.h"
#include "Keys.h"
#include <chrono>
#include <future>
#include <iostream>
#include <stdexcept>
#include <sys/socket.h>
#include <sys/un.h>
#include <thread>
#include <unistd.h>

#define CHECK(x) do { if (!(x)) throw std::runtime_error("check failed at line " + std::to_string(__LINE__)); } while (false)
using namespace zhuyintype;
using Json = nlohmann::json;
namespace {
struct Server {
    std::string path = "/tmp/zt-broker-test-" + std::to_string(getpid());
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    Server() {
        sockaddr_un address{}; address.sun_family = AF_UNIX;
        std::copy(path.begin(), path.end(), address.sun_path);
        CHECK(bind(fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) == 0);
        CHECK(listen(fd, 1) == 0);
        CHECK(setenv("ZT_IME_SOCKET", path.c_str(), 1) == 0);
    }
    ~Server() { close(fd); unlink(path.c_str()); }
    Json read(int peer) {
        std::string line; char c;
        while (recv(peer, &c, 1, 0) == 1 && c != '\n') line += c;
        return Json::parse(line);
    }
};
Json response(const Json &r) {
    return {{"v",1},{"id",r["id"]},{"context",r["context"]},{"ok",true},{"error",""},
        {"handled",true},{"commit","你"},{"preedit","𠀀好"},{"cursor",2},{"candidates",false},{"english",false}};
}
void exchange(const std::function<std::string(Json)> &reply, bool success) {
    Server server;
    auto worker = std::async(std::launch::async, [&] {
        int peer = accept(server.fd, nullptr, nullptr);
        CHECK(peer >= 0);
        const auto request = server.read(peer);
        CHECK(request["v"] == 1 && request["op"] == "focus" && request["context"] == "a");
        const auto wire = reply(request);
        for (size_t i = 0; i < wire.size(); i += 997)
            if (send(peer, wire.data() + i, std::min(size_t(997), wire.size() - i), MSG_NOSIGNAL) <= 0) break;
        close(peer);
    });
    Broker broker;
    CHECK(broker.connect());
    const auto result = broker.request("a", "focus", {{"secure",false}});
    CHECK(result.has_value() == success);
    CHECK(broker.connected() == success);
    worker.get();
}
}
int main() {
    try {
        CHECK(cursorBytes("𠀀好",2) == 4);
        CHECK(!cursorBytes("𠀀好",1));
        CHECK(!cursorBytes("\xff",0));
        CHECK(!cursorBytes("好",2));
        // Focus out: what Fcitx or the application already committed is not sent again.
        CHECK(undelivered("你好", "你好").empty());
        CHECK(undelivered("我你好", "你好") == "我");
        CHECK(undelivered("你好", "") == "你好");
        CHECK(undelivered("你號", "你好").empty());
        {
            const auto parts = preeditParts("你好嗎𠀀", {{0,2},{2,3},{3,5}}, 1, std::nullopt);
            CHECK(parts.size() == 3);
            CHECK(parts[0].text == "你好" && parts[0].mark == Mark::Word);
            CHECK(parts[1].text == "嗎" && parts[1].mark == Mark::Focus);
            CHECK(parts[2].text == "𠀀" && parts[2].mark == Mark::Word);
            const auto target = preeditParts("你好嗎", {{0,2}}, std::nullopt, 0);
            CHECK(target.size() == 2 && target[0].mark == Mark::Target && target[1].text == "嗎");
            const auto bad = preeditParts("你好", {{0,3}}, 0, std::nullopt);
            CHECK(bad.size() == 1 && bad[0].text == "你好" && bad[0].mark == Mark::Word);
            CHECK(preeditParts("ㄓ", {}, std::nullopt, std::nullopt).size() == 1);
            CHECK(preeditParts("", {}, std::nullopt, std::nullopt).empty());
        }
        CHECK(translateKey(fcitx::Key(FcitxKey_Shift_R))->shiftSide == 2);
        CHECK(translateKey(fcitx::Key(FcitxKey_c,fcitx::KeyState::Ctrl))->modifiers == 0x04000000);
        CHECK(translateKey(fcitx::Key(FcitxKey_KP_2))->key == '2');
        CHECK(translateKey(fcitx::Key(FcitxKey_KP_2))->modifiers == 0x20000000);
        CHECK(translateKey(fcitx::Key(FcitxKey_F12))->key == 0x0100003b);
        exchange([](Json r){ return response(r).dump() + '\n'; },true);
        exchange([](Json r){ auto v=response(r); v.erase("error"); return v.dump()+'\n'; },true);
        exchange([](Json r){ auto v=response(r); v["error"]=9; return v.dump()+'\n'; },false);
        exchange([](Json r){ auto v=response(r); v["id"]=9; return v.dump()+'\n'; },false);
        exchange([](Json r){ auto v=response(r); v["context"]="stale"; return v.dump()+'\n'; },false);
        exchange([](Json r){ auto v=response(r); v["cursor"]=1; return v.dump()+'\n'; },false);
        exchange([](Json r){ auto v=response(r); v["handled"]=1; return v.dump()+'\n'; },false);
        exchange([](Json r){ auto v=response(r); v["preedit"]=std::string("a\0b",3); return v.dump()+'\n'; },false);
        exchange([](Json r){ return response(r).dump()+"\n{}\n"; },false);
        exchange([](Json){ return std::string(256*1024,'x'); },false);
        exchange([](Json){ return "{bad}\n"; },false);
        const auto start=std::chrono::steady_clock::now();
        exchange([](Json){ std::this_thread::sleep_for(std::chrono::milliseconds(160)); return ""; },false);
        CHECK(std::chrono::steady_clock::now()-start < std::chrono::milliseconds(500));
        Broker unavailable;
        CHECK(!unavailable.connect());
        std::cout << "PASS framing, fragmented UTF-8, identity, cursor, size, timeout, disconnect and Qt keys\n";
    } catch (const std::exception &e) { std::cerr << e.what() << '\n'; return 1; }
}
