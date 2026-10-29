#include <memory>
#include "TrayIcon.h"
#include "Candidates.h"
#include "Settings.h"
#include "ApplicationInfo.h"
#include "PreviewCapture.h"
#include "EngineSession.h"
#include "Shortcuts.h"
#include "DesktopServices.h"
#include "PlatformSettings.h"
#include "InputController.h"
#include "ImeHost.h"
#include "FontSetup.h"
#include "ImeEndpoint.h"
#include "Practice.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <QStandardPaths>
#include <QFileInfo>

#include <QCommandLineParser>
#include <QDir>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickWindow>
#include <QQuickItem>
#include <QInputMethod>
#include <QTemporaryDir>
#include <QTimer>
#include <QImage>
#include <QFontDatabase>
#include <QIcon>
#include <QScreen>

// Only the preview executable supplies fixtures; the reusable presentation model has no demo data.
static void presentExample(Candidates &candidates, bool single = false, bool corrections = false) {
    const QStringList words = single ? QStringList{"明", "名", "鳴", "銘", "冥", "茗", "酩", "溟", "瞑", "螟"}
                                    : QStringList{"曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "曉名", "小鳴", "小銘"};
    QVariantList entries;
    for (const auto &word : words)
        entries.append(QVariantMap{{"text", word}, {"annotation", corrections ? QStringLiteral("ㄒㄧㄠˇ ㄇㄧㄥˊ") : QString()}});
    candidates.present(entries, single ? QStringLiteral("ㄇㄧㄥˊ") : QStringLiteral("ㄒㄧㄠˇ ㄇㄧㄥˊ"));
}

// The renders' 分析 page shows a month of typing, like the macOS previews; the real store starts empty.
static void writeExampleStats(const QString &path) {
    const auto today = QDate::currentDate();
    QJsonObject days;
    QMap<QString, double> totals;
    for (int back = 0; back < 30; ++back) {
        const int characters = (back % 7 == 5 || back % 9 == 4) ? 0 : 400 + (back * 733) % 1900;
        if (!characters) continue;
        const QJsonObject day{{"characters", characters}, {"compositions", characters / 9}, {"cleanCompositions", characters / 11},
                                    {"keys", characters * 3}, {"picksNormal", characters / 60}, {"picksCorrections", characters / 470},
                                    {"picksSingle", characters / 590}, {"picksPunctuation", characters / 960},
                                    {"backspaces", characters / 46}, {"escapes", characters / 340}, {"deleteAfterCommit", characters / 800},
                                    {"literalZhuyin", characters / 3200}, {"correctedShown", characters / 250},
                                    {"correctedCommitted", characters / 300}, {"phrasesAdded", back == 3 ? 2 : 0},
                                    {"typingMilliseconds", characters * 1360}, {"thinkingMilliseconds", characters * 700}};
        days.insert(today.addDays(-back).toString(Qt::ISODate), day);
        for (auto it = day.begin(); it != day.end(); ++it) totals[it.key()] += it.value().toDouble() * 1.6;
    }
    QJsonObject total;
    for (auto it = totals.cbegin(); it != totals.cend(); ++it) total.insert(it.key(), qRound64(it.value()));
    QJsonArray latency{0, 1900, 2100, 1300, 900, 600, 400, 210, 120, 80, 40, 20, 12, 6, 3, 1, 0, 0, 0, 0, 0, 0};
    QJsonArray hours;
    for (int i = 0; i < 7 * 24; ++i) {
        const int hour = i % 24, weekday = i / 24;
        hours.append(hour < 8 || hour > 22 ? 0 : (weekday == 0 || weekday == 6 ? 40 : 160) + (hour * 37 + weekday * 13) % 220);
    }
    QJsonArray intervals, pauses;
    for (int i = 0; i < 23; ++i) intervals.append(i == 5 ? 900 : i < 10 ? 120 : 10);
    for (int i = 0; i < 16; ++i) pauses.append(i == 3 ? 60 : 12);
    const QJsonObject object{{"schema", 2}, {"since", QDateTime(today.addDays(-45), QTime(9, 0)).toUTC().toString(Qt::ISODate)},
                             {"totals", total}, {"days", days},
                             {"latency", QJsonObject{{"counts", latency}, {"maximum", 87.0}}},
                             {"rhythm", QJsonObject{{"intervals", QJsonObject{{"counts", intervals}, {"maximum", 5.0}}},
                                                    {"pauses", QJsonObject{{"counts", pauses}, {"maximum", 58.0}}},
                                                    {"longestRun", 186}, {"longestCleanRun", 74}, {"hours", hours}}}};
    QFile file(path);
    if (file.open(QIODevice::WriteOnly)) file.write(QJsonDocument(object).toJson(QJsonDocument::Compact));
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    FontSetup::apply();
    app.setOrganizationName("ZhuyinType");
    app.setApplicationName("Desktop");
    app.setWindowIcon(QIcon(":/branding/AppIcon.icns"));
    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addOption({"ime-host", "Run the native input-method service without showing settings."});
    parser.addOption({"ime-socket", "Override the user-local IPC endpoint.", "path"});
    parser.addOption({"render-dir", "Render isolated light/dark previews, then exit.", "directory"});
    parser.addOption({"floating", "Also show the non-activating candidate window."});
    parser.addOption({"lexicon", "Path to lexicon.tsv or lexicon.bin.", "file"});
    parser.addOption({"engine-library", "Path to the public Rust C ABI library.", "file"});
    parser.process(app);
    QTemporaryDir sandbox;
    if (!sandbox.isValid())
        return 1;
    QSettings persistent;
    QSettings isolated(sandbox.filePath("settings.ini"), QSettings::IniFormat);
    auto &storage = parser.isSet("render-dir") ? isolated : persistent;
    Settings settings(storage);
    const auto info = applicationInfo();
    Shortcuts shortcuts(storage);
    const auto userDirectory = parser.isSet("render-dir") ? sandbox.path() : QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (parser.isSet("render-dir")) writeExampleStats(QDir(userDirectory).filePath("stats.json"));
    DesktopServices services(QDir(userDirectory).filePath("stats.json"), info.value("version").toString());
    PlatformSettings platformSettings(storage);
    EngineSession session;
    InputController input(session, settings, shortcuts);
    // 改字記錄: corrections.jsonl beside the personal data, in the Mac's format (`correctionLog` in QML).
    CorrectionLog correctionLog(settings, userDirectory);
    input.setCorrectionLog(&correctionLog, info.value("version").toString());
    ImeHost host(session, input);
    // 在候選字視窗顯示組字的 App: their composition shows in the candidate window, not in the app.
    host.setPopupCompositionApps(platformSettings.apps());
    QObject::connect(&platformSettings, &PlatformSettings::changed, &host, [&] { host.setPopupCompositionApps(platformSettings.apps()); });
    const auto endpoint = parser.isSet("ime-socket") ? parser.value("ime-socket") : imeEndpoint();
    if (!parser.isSet("render-dir")) {
        QLocalSocket existing;
        existing.connectToServer(endpoint);
        if (existing.waitForConnected(150)) {
            if (!parser.isSet("ime-host")) {
                existing.write(QJsonDocument(QJsonObject{{"v", 1}, {"id", 1}, {"context", "settings"}, {"op", "show-settings"}}).toJson(QJsonDocument::Compact) + '\n');
                existing.waitForBytesWritten(500);
            }
            return 0;
        }
        if (!host.reserve(endpoint)) { qCritical().noquote() << host.error(); return 1; }
        app.setQuitOnLastWindowClosed(false);
    }
    auto applyRuntimeSettings = [&] {
        session.applySettings(settings.values());
        services.setUsageEnabled(settings.values().value("recordsUsageStats").toBool());
    };
    QObject::connect(&settings, &Settings::changed, &session, applyRuntimeSettings);
    applyRuntimeSettings();
    QString library, lexicon;
    if (!parser.isSet("render-dir")) {
        library = parser.value("engine-library");
        if (library.isEmpty()) library = QDir(app.applicationDirPath()).filePath(ZT_ENGINE_LIBRARY_NAME);
        lexicon = parser.value("lexicon");
        if (lexicon.isEmpty()) {
            QString data = qEnvironmentVariable("ZHUYIN_DATA");
            if (data.isEmpty()) data = QDir(app.applicationDirPath()).filePath("data");
            if (!QFileInfo::exists(QDir(data).filePath("lexicon.tsv"))) data = QStringLiteral(ZT_DEVELOPMENT_DATA);
            lexicon = QDir(data).filePath("lexicon.tsv");
        }
        session.open(QFileInfo(library).absoluteFilePath(), QFileInfo(lexicon).absoluteFilePath(), QDir(userDirectory).filePath("personal"));
    }
    // 使用統計: the input path's events (IPC.md §usage).  Until InputController reports them, only keys and sent text
    // are counted, from the engine session.
    if (input.metaObject()->indexOfSignal("usageEvent(QString,QVariantMap)") >= 0) {
        QObject::connect(&input, SIGNAL(usageEvent(QString,QVariantMap)), &services, SLOT(recordUsageEvent(QString,QVariantMap)));
    } else {
        QObject::connect(&session, &EngineSession::keyHandled, &services, [&](double ms) {
            services.recordUsageEvent(QStringLiteral("key"), {{"latencyMs", ms}, {"composing", true}});
        });
        QObject::connect(&session, &EngineSession::committedText, &services, [&](const QString &text) {
            services.recordUsageEvent(QStringLiteral("commit"), {{"characters", int(text.toUcs4().size())}});
        });
    }
    QObject::connect(&app, &QCoreApplication::aboutToQuit, &session, [&] {
        if (session.ready()) session.flush();
        services.saveIfNeeded();
    });
    Candidates candidates;
    Candidates correctionCandidates;
    presentExample(correctionCandidates, false, true);
    presentExample(candidates);
    auto applySettings = [&] {
        const auto size = settings.values().value("candidatesPerPage").toInt();
        candidates.setPageSize(size);
        correctionCandidates.setPageSize(size);
    };
    QObject::connect(&settings, &Settings::changed, &candidates, applySettings);
    QObject::connect(&candidates, &Candidates::filterRequested, &candidates,
                     [&](bool single, bool corrections) { presentExample(candidates, single, corrections); });
    QObject::connect(&correctionCandidates, &Candidates::filterRequested, &correctionCandidates,
                     [&](bool single, bool corrections) { presentExample(correctionCandidates, single, corrections); });
    correctionCandidates.toggleCorrections();
    applySettings();

    // The setup wizard's sandbox: a copy of the settings and keys, and a practice engine of its own while it is open.
    Practice practice(storage, library.isEmpty() ? QString() : QFileInfo(library).absoluteFilePath(),
                      lexicon.isEmpty() ? QString() : QFileInfo(lexicon).absoluteFilePath());
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("correctionLog", &correctionLog);
    if (info.value("licenseName").toString().isEmpty() || info.value("licenseText").toString().isEmpty()) return 1;
    engine.setInitialProperties({{"settings", QVariant::fromValue(&settings)}, {"candidates", QVariant::fromValue(&candidates)}, {"correctionCandidates", QVariant::fromValue(&correctionCandidates)}, {"appInfo", info}, {"engineSession", QVariant::fromValue(&session)}, {"inputController", QVariant::fromValue(&input)}, {"shortcuts", QVariant::fromValue(&shortcuts)}, {"services", QVariant::fromValue(&services)}, {"platformSettings", QVariant::fromValue(&platformSettings)}, {"practice", QVariant::fromValue(&practice)}});
    if (parser.isSet("render-dir"))
        QObject::connect(&engine, &QQmlEngine::warnings, &app, [](const QList<QQmlError> &) { QCoreApplication::exit(1); });
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app, [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("ZhuyinType.Desktop", "Main");
    if (engine.rootObjects().isEmpty())
        return 1;
    QQmlApplicationEngine popupEngine;
    // Windows: the logo in the notification area while the service runs; a click opens the settings.
    std::unique_ptr<TrayIcon> tray;
    if (!parser.isSet("render-dir")) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (!window) return 1;
        window->setProperty("imeRunning", true);
        if (parser.isSet("ime-host")) window->hide();
        else QMetaObject::invokeMethod(window, "showWizardOnFirstUse", Qt::QueuedConnection);
        QObject::connect(&host, &ImeHost::showSettingsRequested, window, [window, &services] {
            services.saveIfNeeded();
            window->show(); window->raise(); window->requestActivate();
            QMetaObject::invokeMethod(window, "showWizardOnFirstUse", Qt::QueuedConnection);
        });
#ifdef Q_OS_WIN
        if (parser.isSet("ime-host")) {
            tray = std::make_unique<TrayIcon>();
            QObject::connect(tray.get(), &TrayIcon::activated, &host, [&host] { emit host.showSettingsRequested(); });
        }
#endif
        popupEngine.setInitialProperties({{"host", QVariant::fromValue(&host)}, {"settings", QVariant::fromValue(&settings)}, {"shortcuts", QVariant::fromValue(&shortcuts)}});
        popupEngine.loadFromModule("ZhuyinType.Desktop", "ImePopup");
        if (popupEngine.rootObjects().isEmpty()) return 1;
    }
    auto disablePracticeIme = [](QObject *object) {
        if (object && object->objectName() == "liveDocument") {
            if (auto *item = qobject_cast<QQuickItem *>(object)) {
                // The practice field already dispatches through the shared controller.
                item->setFlag(QQuickItem::ItemAcceptsInputMethod, false);
                QGuiApplication::inputMethod()->update(Qt::ImEnabled);
            }
        }
    };
    disablePracticeIme(engine.rootObjects().first()->findChild<QObject *>("liveDocument"));
    QObject::connect(&app, &QGuiApplication::focusObjectChanged, &app, disablePracticeIme);
    if (parser.isSet("floating")) {
        auto *floating = engine.rootObjects().first()->findChild<QQuickWindow *>("floatingPreview");
        if (!floating) return 1;
        const auto bounds = app.primaryScreen()->availableGeometry();
        floating->setPosition(bounds.center() - QPoint(floating->width() / 2, floating->height() / 2));
        floating->show();
    }
    if (parser.isSet("render-dir")) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (!window) return 1;
        auto *capture = new PreviewCapture(*window, settings, parser.value("render-dir"), &app);
        if (!capture->start()) return 1;
    }
    // Publish the socket only after the engine and QML are ready to handle the first key.
    if (!parser.isSet("render-dir") && !host.listen(endpoint)) { qCritical().noquote() << host.error(); return 1; }
    return app.exec();
}
