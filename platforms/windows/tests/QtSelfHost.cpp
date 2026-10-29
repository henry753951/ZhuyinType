#include "Identity.h"
#include "EngineSession.h"
#include "ImeHost.h"
#include "InputController.h"
#include "Settings.h"
#include "Shortcuts.h"
#include "TestFocus.h"
#include "KeyboardInput.h"
#include <msctf.h>
#include <wrl/client.h>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QSettings>
#include <QTemporaryDir>
#include <QTimer>
#include <cstdio>

int main(int argc,char** argv) {
    if (argc!=3) { fprintf(stderr,"usage: zt-qt-selfhost ENGINE_DLL LEXICON\n"); return 2; }
    const auto endpoint=QStringLiteral("\\\\.\\pipe\\ZhuyinType-selfhost-%1").arg(GetCurrentProcessId());
    qputenv("ZT_IME_SOCKET",endpoint.toUtf8());
    QGuiApplication app(argc,argv);
    QTemporaryDir directory;
    if (!directory.isValid()) return 1;
    QSettings storage(directory.filePath("settings.ini"),QSettings::IniFormat);
    Settings settings(storage);
    settings.setValue("learningEnabled",false);
    EngineSession session;
    if (!session.applySettings(settings.values()) || !session.open(QString::fromLocal8Bit(argv[1]),QString::fromLocal8Bit(argv[2]),directory.filePath("personal"))) return 1;
    Shortcuts shortcuts(storage);
    InputController input(session,settings,shortcuts);
    ImeHost host(session,input);
    unsigned commits=0;
    QObject::connect(&session,&EngineSession::committedText,&app,[&](const QString&) { ++commits; });
    if (!host.listen(endpoint)) return 1;
    QQmlApplicationEngine engine;
    engine.loadData(R"(
import QtQuick
Window {
    visible: true; width: 600; height: 200
    title: "ZhuyinType 同執行緒輸入測試"
    TextInput { objectName: "field"; x: 20; y: 30; width: 540; height: 100; focus: true }
}
)");
    if (engine.rootObjects().isEmpty()) return 1;
    auto* window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    if (!window) return 1;
    auto* field=window->findChild<QObject*>("field");
    if (!field) return 1;
    const auto native=reinterpret_cast<HWND>(window->winId());
    Microsoft::WRL::ComPtr<ITfInputProcessorProfiles> languages;
    Microsoft::WRL::ComPtr<ITfInputProcessorProfileMgr> profiles;
    if (FAILED(CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&languages))) || FAILED(languages.As(&profiles))) return 1;
    LANGID previous=0; languages->GetCurrentLanguage(&previous);
    QTimer::singleShot(800,&app,[&] {
        ActivateTestWindow(native,native);
        ReleaseTestModifiers();
        window->requestActivate();
        if (FAILED(languages->ChangeCurrentLanguage(zt::language)) ||
            FAILED(profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS))) {
            fprintf(stderr,"FAIL selfhost profile activation\n"); app.exit(1);
        }
    });
    int step=0;
    bool sawComposition=false;
    QString expected;
    QTimer sequence;
    sequence.setInterval(250);
    QObject::connect(&sequence,&QTimer::timeout,&app,[&] {
        if (step<4) {
            if (GetForegroundWindow()!=native) { fprintf(stderr,"FAIL selfhost foreground\n"); app.exit(1); return; }
            const WORD keys[]={WORD('S'),WORD('U'),WORD('3'),VK_RETURN};
            if (step==3) {
                expected=session.composition();
                sawComposition=!expected.isEmpty() && field->property("preeditText").toString()==expected;
            }
            INPUT events[2]{}; for (auto& event:events) { event.type=INPUT_KEYBOARD; event.ki.wVk=keys[step]; }
            events[1].ki.dwFlags=KEYEVENTF_KEYUP;
            if (SendInput(2,events,sizeof(INPUT))!=2) { app.exit(1); return; }
            ++step;
        } else {
            sequence.stop();
            const bool committed=expected==QStringLiteral("你") && field->property("text").toString()==QStringLiteral("你") &&
                field->property("preeditText").toString().isEmpty() && commits==1;
            const bool loaded=GetModuleHandleW(L"zhuyintype-tsf.dll")!=nullptr;
            const bool callback=GetProcAddress(GetModuleHandleW(nullptr),"ZtProcessImeEvents")!=nullptr;
            printf("%s same-process same-GUI-thread Qt host native SendInput composition=%u commit=%u DLL=%u\n",
                committed&&sawComposition&&loaded&&callback?"PASS":"FAIL",sawComposition,committed,loaded);
            printf("EVIDENCE host event callback=%u\n",callback);
            profiles->DeactivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS);
            if (previous) languages->ChangeCurrentLanguage(previous);
            app.exit(committed&&sawComposition&&loaded&&callback?0:1);
        }
    });
    QTimer::singleShot(1500,&app,[&] { sequence.start(); });
    QTimer::singleShot(12000,&app,[&] { fprintf(stderr,"FAIL selfhost deadline\n"); app.exit(1); });
    return app.exec();
}
