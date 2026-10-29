#include "ImeHost.h"
#include "Settings.h"
#include "Shortcuts.h"
#include <QFile>
#include <QJsonDocument>
#include <QJsonArray>
#include <QTemporaryDir>
#include <QElapsedTimer>
#include <QtTest>

class ImeHostTest final : public QObject {
    Q_OBJECT
    static QJsonObject request(QLocalSocket &client, QJsonObject request) {
        const int sequence = client.property("sequence").toInt() + 1;
        client.setProperty("sequence", sequence);
        request.insert("v", 1); request.insert("id", sequence);
        if (!request.contains("context")) request.insert("context", "field");
        client.write(QJsonDocument(request).toJson(QJsonDocument::Compact) + '\n');
        client.flush();
        QElapsedTimer elapsed; elapsed.start();
        while (!client.canReadLine() && elapsed.elapsed() < 2000) QTest::qWait(1);
        const auto result = QJsonDocument::fromJson(client.readLine()).object();
        if (result.value("id").toInt() != sequence) return {};
        return result;
    }
    static QJsonObject key(QLocalSocket &client, int value) {
        return request(client, {{"op", "key"}, {"secure", false}, {"key", value}, {"modifiers", 0}, {"release", false}});
    }
    struct Fixture {
        QTemporaryDir directory;
        QSettings storage{directory.filePath("settings.ini"), QSettings::IniFormat};
        Settings settings{storage};
        Shortcuts shortcuts{storage};
        EngineSession engine;
        InputController input{engine, settings, shortcuts};
        ImeHost host{engine, input};
        QLocalSocket client;
        bool start() {
            QFile lexicon(directory.filePath("lexicon.tsv"));
            if (!lexicon.open(QIODevice::WriteOnly)) return false;
            lexicon.write(QStringLiteral("ㄋㄧˇ\t你\t-3\nㄋㄧˇ\t妳\t-4\nㄋㄧˇ\t擬\t-5\nㄋㄧˇ\t旎\t-6\n").toUtf8());
            lexicon.close();
            engine.applySettings(settings.values());
            QObject::connect(&settings, &Settings::changed, &engine, [this] { engine.applySettings(settings.values()); });
            if (!engine.open(qEnvironmentVariable("ZT_ENGINE_LIBRARY"), lexicon.fileName(), directory.filePath("personal"))) return false;
#ifdef Q_OS_WIN
            const auto endpoint = QStringLiteral("ZhuyinType-test-") + QUuid::createUuid().toString(QUuid::WithoutBraces);
#else
            const auto endpoint = directory.filePath("ime");
#endif
            if (!host.listen(endpoint)) return false;
            client.connectToServer(endpoint);
            return client.waitForConnected(1000);
        }
    };
private slots:
    void deliverySecureAndExactlyOnce() {
        Fixture f; QVERIFY2(f.start(), qPrintable(f.engine.error() + f.host.error()));
        QVERIFY(request(f.client, {{"op", "focus"}, {"secure", false}}).value("ok").toBool());
        key(f.client, 'S'); key(f.client, 'U');
        QCOMPARE(key(f.client, '3').value("preedit").toString(), QStringLiteral("你"));
        QVERIFY(key(f.client, Qt::Key_Down).value("candidates").toBool());
        QVERIFY(f.host.visible());
        f.host.pick(1);
        const auto picked = request(f.client, {{"op", "poll"}});
        // A pick edits the composer; Enter then delivers it once to the OS adapter.
        QVERIFY(picked.value("ok").toBool());
        const auto commit = key(f.client, Qt::Key_Return);
        QCOMPARE(picked.value("commit").toString() + commit.value("commit").toString(), QStringLiteral("妳"));
        QVERIFY(request(f.client, {{"op", "poll"}}).value("commit").toString().isEmpty());
        key(f.client, 'S'); key(f.client, 'U'); key(f.client, '3');
        const auto secure = request(f.client, {{"op", "secure"}, {"secure", true}});
        QVERIFY(secure.value("preedit").toString().isEmpty());
        QVERIFY(secure.value("commit").toString().isEmpty());
        QVERIFY(!f.host.visible());
        auto secureKey = request(f.client, {{"op", "key"}, {"secure", true}, {"key", 'S'}, {"modifiers", 0}, {"release", false}});
        QVERIFY(!secureKey.value("handled").toBool());
        QVERIFY(secureKey.value("preedit").toString().isEmpty());
        QVERIFY(request(f.client, {{"op", "blur"}}).value("commit").toString().isEmpty());
    }
    void focusIsolationAndValidation() {
        Fixture f; QVERIFY(f.start());
        QVERIFY(!key(f.client, 'S').value("ok").toBool());
        request(f.client, {{"op", "focus"}, {"secure", false}, {"rect", QJsonArray{20, 30, 1, 18}}});
        key(f.client, 'S'); key(f.client, 'U'); key(f.client, '3');
        const auto invalid = request(f.client, {{"op", "key"}, {"secure", false}, {"key", 3.5}, {"modifiers", 0}, {"release", false}});
        QVERIFY(!invalid.value("ok").toBool());
        QCOMPARE(f.engine.composition(), QStringLiteral("你"));
        for (const auto *field : {"secure", "repeat", "capsLock", "inlinePreedit"}) {
            QJsonObject malformed{{"op", "focus"}, {"secure", false}};
            malformed.insert(field, "false");
            QVERIFY(!request(f.client, malformed).value("ok").toBool());
            QCOMPARE(f.engine.composition(), QStringLiteral("你"));
        }
        QVERIFY(!request(f.client, {{"op", "poll"}, {"rect", QJsonArray{20, 30, 1, 18}},
                                   {"scale", "1"}}).value("ok").toBool());
        const auto stale = request(f.client, {{"op", "reset"}, {"context", "other"}});
        QVERIFY(!stale.value("ok").toBool());
        QCOMPARE(f.engine.composition(), QStringLiteral("你"));
        QCOMPARE(request(f.client, {{"op", "blur"}}).value("commit").toString(), QStringLiteral("你"));
        QVERIFY(!request(f.client, {{"op", "poll"}}).value("ok").toBool());
        request(f.client, {{"op", "focus"}, {"secure", false}, {"context", "other"}});
        QVERIFY(f.engine.composition().isEmpty());
        f.client.disconnectFromServer();
        QTRY_VERIFY(f.client.state() == QLocalSocket::UnconnectedState);
        QCoreApplication::processEvents();
        QVERIFY(!f.host.visible());
    }
    void initialCapsLockAndLiveSettings() {
        Fixture f; QVERIFY(f.start());
        request(f.client, {{"op", "focus"}, {"secure", false}, {"capsLock", true}});
        QVERIFY(!key(f.client, 'S').value("handled").toBool());
        QVERIFY(f.engine.composition().isEmpty());
        key(f.client, Qt::Key_CapsLock);
        key(f.client, 'S'); key(f.client, 'U'); key(f.client, '3');
        QVERIFY(f.settings.setValue("candidatesPerPage", 3));
        key(f.client, Qt::Key_Down);
        QCOMPARE(f.host.pageSize(), 3);
        QCOMPARE(f.host.rows().size(), 3);
        f.host.turnPage(1);
        QCOMPARE(f.host.page(), 1);
        QVERIFY(f.host.rows().size() > 0);
        // Secure transition must also discard a mouse action pending its next poll.
        f.host.pick(0);
        request(f.client, {{"op", "secure"}, {"secure", true}});
        QVERIFY(request(f.client, {{"op", "poll"}}).value("commit").toString().isEmpty());
    }
    void compositionFallbackDoesNotRequireCandidateList() {
        Fixture f; QVERIFY(f.start());
        request(f.client, {{"op", "focus"}, {"secure", false}, {"inlinePreedit", false}});
        key(f.client, 'S'); key(f.client, 'U'); key(f.client, '3');
        QVERIFY(f.host.rows().isEmpty());
        QVERIFY(f.host.visible());
        QCOMPARE(f.host.composition(), QStringLiteral("你"));
        request(f.client, {{"op", "secure"}, {"secure", true}});
        QVERIFY(!f.host.visible());
    }
    void precedingTextModeToastAndPlacement() {
        Fixture f; QVERIFY(f.start());
        QVERIFY(request(f.client, {{"op", "focus"}, {"secure", false}, {"app", "org.example.editor"},
                                   {"before", QStringLiteral("我說")}}).value("ok").toBool());
        key(f.client, 'S'); key(f.client, 'U'); key(f.client, '3');
        QCOMPARE(f.input.recentText(), QStringLiteral("我說"));
        QVERIFY(request(f.client, {{"op", "context"}, {"before", QStringLiteral("前文")}}).value("ok").toBool());
        QVERIFY(!request(f.client, {{"op", "context"}, {"before", 3}}).value("ok").toBool());
        QVERIFY(!request(f.client, {{"op", "focus"}, {"secure", false}, {"app", QString(300, 'a')}}).value("ok").toBool());
        // A Shift tap in the focused field asks the popup for the 中／英 toast.
        QSignalSpy toast(&f.host, &ImeHost::modeSwitched);
        request(f.client, {{"op", "key"}, {"secure", false}, {"key", int(Qt::Key_Shift)}, {"modifiers", int(Qt::ShiftModifier)}, {"release", false}});
        request(f.client, {{"op", "key"}, {"secure", false}, {"key", int(Qt::Key_Shift)}, {"modifiers", 0}, {"release", true}});
        QCOMPARE(toast.count(), 1);
        QCOMPARE(toast.first().first().toBool(), true);
        // The window goes below the caret, or above it when asked, with the candidate text over the composition.
        request(f.client, {{"op", "poll"}, {"rect", QJsonArray{300, 300, 1, 18}}});
        const auto below = f.host.popupPosition(200, 100, false, 30);
        QCOMPARE(below, QPointF(270, 322));
        const auto above = f.host.popupPosition(200, 100, true, 30);
        QCOMPARE(above, QPointF(270, 196));
    }
    void listedAppsShowCompositionInTheWindow() {
        Fixture f; QVERIFY(f.start());
        f.host.setPopupCompositionApps({"Notepad.exe"});
        request(f.client, {{"op", "focus"}, {"secure", false}, {"app", "notepad.exe"}});
        key(f.client, 'S'); key(f.client, 'U');
        const auto response = key(f.client, '3');
        QVERIFY(response.value("preedit").toString().isEmpty());
        QVERIFY(!f.host.inlinePreedit());
        QVERIFY(f.host.visible());
        QCOMPARE(f.host.composition(), QStringLiteral("你"));
        request(f.client, {{"op", "blur"}});
        request(f.client, {{"op", "focus"}, {"secure", false}, {"app", "editor"}});
        QVERIFY(f.host.inlinePreedit());
    }
    void refusesOccupiedFile() {
#ifndef Q_OS_WIN
        Fixture f;
        const auto endpoint = f.directory.filePath("occupied");
        QFile file(endpoint); QVERIFY(file.open(QIODevice::WriteOnly));
        file.write("keep"); file.close();
        QVERIFY(!f.host.listen(endpoint));
        QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(file.readAll(), QByteArray("keep"));
#endif
    }
    void duplicateSequenceDisconnects() {
        Fixture f; QVERIFY(f.start());
        request(f.client, {{"op", "focus"}, {"secure", false}});
        f.client.write("{\"v\":1,\"id\":1,\"context\":\"field\",\"op\":\"poll\"}\n");
        f.client.flush();
        QTRY_COMPARE(f.client.state(), QLocalSocket::UnconnectedState);
    }
};
QTEST_MAIN(ImeHostTest)
#include "ImeHostTest.moc"
