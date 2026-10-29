#include "BrokerClient.h"
#include "Keyboard.h"
#include "Coordinates.h"
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <functional>
#include <stdexcept>
#include <thread>
#include <vector>
using namespace zt;
using namespace winrt::Windows::Data::Json;
namespace {
void Check(bool value,const char* name) { if (!value) throw std::runtime_error(name); }
struct Server {
    HANDLE pipe=INVALID_HANDLE_VALUE;
    std::thread thread;
    explicit Server(std::function<std::string(Json)> respond,bool split=false) {
        wchar_t path[128]{};
        swprintf_s(path,L"\\\\.\\pipe\\ZhuyinType-test-%lu-%llu",GetCurrentProcessId(),GetTickCount64());
        SetEnvironmentVariableW(L"ZT_IME_SOCKET",path);
        pipe=CreateNamedPipeW(path,PIPE_ACCESS_DUPLEX,PIPE_TYPE_BYTE|PIPE_READMODE_BYTE|PIPE_WAIT|PIPE_REJECT_REMOTE_CLIENTS,1,4096,4096,0,nullptr);
        Check(pipe!=INVALID_HANDLE_VALUE,"CreateNamedPipe");
        thread=std::thread([this,respond,split] {
            winrt::init_apartment(winrt::apartment_type::multi_threaded);
            if (!ConnectNamedPipe(pipe,nullptr) && GetLastError()!=ERROR_PIPE_CONNECTED) return;
            std::string input; char ch=0; DWORD bytes=0;
            while (ReadFile(pipe,&ch,1,&bytes,nullptr) && bytes && ch!='\n') input+=ch;
            try {
                auto output=respond(Json::Parse(winrt::to_hstring(input)));
                if (split && output.size()>2) {
                    WriteFile(pipe,output.data(),2,&bytes,nullptr);
                    std::this_thread::sleep_for(std::chrono::milliseconds(2));
                    WriteFile(pipe,output.data()+2,static_cast<DWORD>(output.size()-2),&bytes,nullptr);
                } else WriteFile(pipe,output.data(),static_cast<DWORD>(output.size()),&bytes,nullptr);
            } catch (...) { /* A client cancellation may disconnect before the reply is written. */ }
            DisconnectNamedPipe(pipe);
            winrt::uninit_apartment();
        });
    }
    ~Server() { if (thread.joinable()) thread.join(); CloseHandle(pipe); }
};
std::string Response(Json request,const std::function<void(Json&)>& change={}) {
    Json j;
    j.Insert(L"v",request.Lookup(L"v")); j.Insert(L"id",request.Lookup(L"id")); j.Insert(L"context",request.Lookup(L"context"));
    Put(j,L"ok",true); Put(j,L"handled",true); Put(j,L"candidates",false); Put(j,L"english",false); Put(j,L"cursor",2);
    j.Insert(L"error",JsonValue::CreateStringValue(L""));
    j.Insert(L"commit",JsonValue::CreateStringValue(L"測試"));
    j.Insert(L"preedit",JsonValue::CreateStringValue(L"\U0001f600"));
    if (change) change(j);
    return winrt::to_string(j.Stringify())+'\n';
}
struct SameThreadServer {
    HANDLE pipe=INVALID_HANDLE_VALUE;
    BrokerClient* client=nullptr;
    unsigned inputMessages=0;
    bool nestedRejected=false;
    bool serviced=false;
    bool responded=false;
    static LRESULT CALLBACK WindowProc(HWND window,UINT message,WPARAM wp,LPARAM lp) {
        auto* self=reinterpret_cast<SameThreadServer*>(GetWindowLongPtrW(window,GWLP_USERDATA));
        if (message==WM_NCCREATE) {
            self=static_cast<SameThreadServer*>(reinterpret_cast<CREATESTRUCTW*>(lp)->lpCreateParams);
            SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(self));
        }
        if (self && message==WM_APP+1) {
            self->serviced=true;
            self->nestedRejected=!self->client->request(L"nested",L"poll",Json{}).has_value() && self->client->connected();
            char ch=0; DWORD bytes=0; std::string frame;
            while (ReadFile(self->pipe,&ch,1,&bytes,nullptr) && bytes && ch!='\n') frame+=ch;
            try { const auto response=Response(Json::Parse(winrt::to_hstring(frame))); self->responded=WriteFile(self->pipe,response.data(),static_cast<DWORD>(response.size()),&bytes,nullptr)!=FALSE; }
            catch (...) { return 0; }
            return 0;
        }
        if (self && (message==WM_KEYDOWN || message==WM_LBUTTONDOWN)) { ++self->inputMessages; return 0; }
        return DefWindowProcW(window,message,wp,lp);
    }
};
void SameThread() {
    SameThreadServer server;
    wchar_t path[128]{}; swprintf_s(path,L"\\\\.\\pipe\\ZhuyinType-same-thread-%lu",GetCurrentProcessId());
    SetEnvironmentVariableW(L"ZT_IME_SOCKET",path);
    server.pipe=CreateNamedPipeW(path,PIPE_ACCESS_DUPLEX,PIPE_TYPE_BYTE|PIPE_WAIT|PIPE_REJECT_REMOTE_CLIENTS,1,4096,4096,0,nullptr);
    Check(server.pipe!=INVALID_HANDLE_VALUE,"same-thread pipe");
    WNDCLASSW klass{}; klass.hInstance=GetModuleHandleW(nullptr); klass.lpszClassName=L"ZhuyinType.SameThread.Test";
    klass.lpfnWndProc=SameThreadServer::WindowProc; RegisterClassW(&klass);
    const auto window=CreateWindowExW(0,klass.lpszClassName,L"",0,0,0,0,0,HWND_MESSAGE,nullptr,klass.hInstance,&server);
    {
        BrokerClient client; server.client=&client; Check(client.connect(),"same-thread connect");
        ConnectNamedPipe(server.pipe,nullptr);
        PostMessageW(window,WM_KEYDOWN,'A',0); PostMessageW(window,WM_LBUTTONDOWN,0,0); PostMessageW(window,WM_APP+1,0,0);
        const auto start=GetTickCount64();
        const auto reply=client.request(L"same-thread",L"poll",Json{});
        if (!reply) fprintf(stderr,"same-thread serviced=%u responded=%u nested=%u input=%u duration=%llu\n",server.serviced,server.responded,server.nestedRejected,server.inputMessages,GetTickCount64()-start);
        Check(reply.has_value(),"same-thread event-pumped response");
        Check(server.nestedRejected && server.inputMessages==0,"reentry and input ordering");
        MSG message{};
        while (PeekMessageW(&message,window,0,0,PM_REMOVE)) DispatchMessageW(&message);
        Check(server.inputMessages==2,"input remains queued exactly once");
    }
    DestroyWindow(window); UnregisterClassW(klass.lpszClassName,klass.hInstance); CloseHandle(server.pipe);
    printf("PASS same-thread server, reentry, queued keyboard/mouse\n");
}
void Case(const char* label,const std::function<std::string(Json)>& response,bool good,bool split=false) {
    Server server(response,split);
    BrokerClient client;
    Check(client.connect(),"connect");
    const auto started=GetTickCount64();
    auto reply=client.request(L"fixture",L"poll",Json{});
    const auto elapsed=GetTickCount64()-started;
    if (std::string(label)=="deadline") {
        Check(elapsed>=150 && elapsed<450,"200 ms request deadline");
        printf("EVIDENCE deadline elapsed=%llu ms (configured 200 ms)\n",elapsed);
    }
    Check(reply.has_value()==good,label);
    if (reply) Check(reply->commit==L"測試" && reply->cursor==2,label);
    else Check(!client.connected(),"invalid replies disconnect");
    printf("PASS %s\n",label);
}
}
int main(int argc,char** argv) {
    try {
        winrt::init_apartment(winrt::apartment_type::single_threaded);
        if (argc==2 && std::string(argv[1])=="--live") {
            BrokerClient live; Check(live.connect(),"live pipe connect");
            Json focus; Put(focus,L"secure",false); Put(focus,L"capsLock",false); PutRect(focus,RECT{200,200,202,220},1);
            Check(live.request(L"live-probe",L"focus",focus).has_value(),"live focus");
            for (const int key:{int('S'),int('U'),int('3'),0x01000004}) {
                Json fields; Put(fields,L"key",key); Put(fields,L"modifiers",0); Put(fields,L"release",false); Put(fields,L"repeat",false); Put(fields,L"secure",false);
                const auto start=GetTickCount64(); const auto reply=live.request(L"live-probe",L"key",fields);
                if (reply) printf("LIVE reply handled=%u english=%u preedit=%zu commit=%zu elapsed=%llu\n",reply->handled,reply->english,reply->preedit.size(),reply->commit.size(),GetTickCount64()-start);
                Check(reply.has_value(),"live key response");
            }
            Check(live.request(L"live-probe",L"blur",Json{}).has_value(),"live blur");
            return 0;
        }
        (void)Response(Json::Parse(L"{\"v\":1,\"id\":1,\"context\":\"warmup\"}"));
        SameThread();
        Check(QtRect(RECT{2100,300,2104,340},POINT{1920,0},2)==std::array<double,4>{2010,150,2,20},"secondary monitor origin");
        Check(QtRect(RECT{-1800,-200,-1796,-160},POINT{-1920,-300},1.25)==std::array<double,4>{-1824,-220,3.2,32},"negative monitor origin");
        Check(QtRect(RECT{100,200,102,220},POINT{0,0},1)==std::array<double,4>{100,200,2,20},"100 percent coordinates");
        Case("split UTF-8 frame",[](Json j) { return Response(j); },true,true);
        {
            BrokerClient reusable;
            { Server first([](Json j) { return Response(j); }); Check(reusable.connect(),"first healthy connection"); Check(reusable.request(L"first",L"poll",Json{}).has_value(),"first healthy response"); reusable.disconnect(); }
            { Server second([](Json j) { return Response(j); }); Check(reusable.connect(),"intentional close reconnects immediately"); Check(reusable.request(L"second",L"poll",Json{}).has_value(),"second healthy response"); reusable.disconnect(); }
            printf("PASS healthy focus reconnect has no failure backoff\n");
        }
        Case("wrong id",[](Json j) { return Response(j,[](Json& v) { Put(v,L"id",1234); }); },false);
        Case("wrong context",[](Json j) { return Response(j,[](Json& v) { v.Insert(L"context",JsonValue::CreateStringValue(L"other")); }); },false);
        Case("wrong version",[](Json j) { return Response(j,[](Json& v) { Put(v,L"v",2); }); },false);
        Case("surrogate cursor",[](Json j) { return Response(j,[](Json& v) { Put(v,L"cursor",1); }); },false);
        Case("out of range cursor",[](Json j) { return Response(j,[](Json& v) { Put(v,L"cursor",8); }); },false);
        Case("wrong field type",[](Json j) { return Response(j,[](Json& v) { Put(v,L"preedit",3); }); },false);
        Case("invalid UTF-8",[](Json) { return std::string("\xff\n"); },false);
        Case("malformed JSON",[](Json) { return std::string("{\n"); },false);
        Case("unsolicited second frame",[](Json j) { return Response(j)+Response(j); },false);
        Case("oversize",[](Json) { return std::string(256*1024,'x')+'\n'; },false);
        const auto before=GetTickCount64();
        Case("deadline",[](Json j) { std::this_thread::sleep_for(std::chrono::milliseconds(600)); return Response(j); },false);
        Check(GetTickCount64()-before<1000,"timeout bounded");
        SetEnvironmentVariableW(L"ZT_IME_SOCKET",L"\\\\.\\pipe\\ZhuyinType-absent-test");
        BrokerClient absent;
        const auto start=GetTickCount64();
        Check(!absent.connect(),"unavailable");
        Check(GetTickCount64()-start<100,"connect bounded");
        Check(TranslateKey(VK_LEFT,0,false).key==0x01000012,"left mapping");
        Check(TranslateKey(VK_SHIFT,0x2a0000,false).shiftSide==1,"left Shift");
        Check(TranslateKey(VK_SHIFT,0x360000,true).shiftSide==2,"right Shift");
        Check(TranslateKey('A',1LL<<30,false).repeat,"repeat");
        printf("PASS unavailable and keyboard mapping\n");
        SetEnvironmentVariableW(L"ZT_IME_SOCKET",nullptr);
        winrt::uninit_apartment();
        return 0;
    } catch (const std::exception& e) { fprintf(stderr,"FAIL %s\n",e.what()); return 1; }
    catch (...) { fprintf(stderr,"FAIL Windows API\n"); return 1; }
}
