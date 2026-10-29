#include "PlatformSettings.h"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>
#include <limits>

namespace {
bool writeFile(const QString &path, const QByteArray &bytes) {
    QFile file(path);
    return file.open(QIODevice::WriteOnly) && file.write(bytes) == bytes.size() && file.flush();
}
}

class PlatformSettingsTest : public QObject {
    Q_OBJECT
private slots:
    void appsPersistNormalizeAndRemove() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto settingsPath = directory.filePath("settings.ini");
        {
            QSettings storage(settingsPath, QSettings::IniFormat);
            PlatformSettings settings(storage);
            QVERIFY(settings.addApp("  org.example.Editor  "));
            QVERIFY(settings.addApp("example-terminal.desktop"));
            QVERIFY(settings.addApp("org.example.Editor"));
            QCOMPARE(settings.apps(), QStringList({"org.example.Editor", "example-terminal.desktop"}));
            QVERIFY(!settings.addApp(""));
            QVERIFY(!settings.error().isEmpty());
            QSignalSpy changed(&settings, &PlatformSettings::changed);
            QVERIFY(settings.addApp("org.example.Editor"));
            QVERIFY(settings.error().isEmpty());
            QVERIFY(!changed.isEmpty());
        }
        QSettings storage(settingsPath, QSettings::IniFormat);
        PlatformSettings restored(storage);
        QCOMPARE(restored.apps(), QStringList({"org.example.Editor", "example-terminal.desktop"}));
        QVERIFY(restored.removeApp("org.example.Editor"));
        PlatformSettings reread(storage);
        QCOMPARE(reread.apps(), QStringList({"example-terminal.desktop"}));
        QVERIFY(restored.removeApp("missing"));
        QCOMPARE(restored.apps(), reread.apps());
    }
    void invalidApps_data() {
        QTest::addColumn<QString>("id");
        QTest::newRow("empty") << QString();
        QTest::newRow("whitespace") << QString("   ");
        QTest::newRow("too-long") << QString(257, 'x');
        QTest::newRow("line-feed") << QString("org.\nexample");
        QTest::newRow("carriage-return") << QString("org.\rexample");
        QTest::newRow("nul") << QString("org.") + QChar::Null + "example";
        QTest::newRow("tab") << QString("org.\texample");
        QTest::newRow("line-separator") << QString("org.") + QChar(0x2028) + "example";
    }
    void invalidApps() {
        QFETCH(QString, id);
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        PlatformSettings settings(storage);
        QVERIFY(!settings.addApp(id));
        QVERIFY(settings.apps().isEmpty());
        QVERIFY(!storage.contains("compatibility/apps"));
        storage.setValue("compatibility/apps", QStringList{id, "org.valid"});
        PlatformSettings restored(storage);
        QCOMPARE(restored.apps(), QStringList{"org.valid"});
    }
    void persistedAppsNormalizeBeforeDeduplication() {
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        storage.setValue("compatibility/apps", QStringList{" org.example ", "org.example", "org.other"});
        PlatformSettings settings(storage);
        QCOMPARE(settings.apps(), QStringList({"org.example", "org.other"}));
        QVERIFY(settings.removeApp("org.example"));
        QCOMPARE(storage.value("compatibility/apps").toStringList(), QStringList{"org.other"});
    }
    void appSaveFailurePreservesState() {
        QTemporaryDir directory;
        QSettings storage(directory.path(), QSettings::IniFormat);
        PlatformSettings settings(storage);
        QVERIFY(!settings.addApp("org.valid"));
        QVERIFY(settings.apps().isEmpty());
        QVERIFY(!settings.error().isEmpty());
        QVERIFY(!storage.contains("compatibility/apps"));
    }
    void desktopEntriesNameTheirPrograms() {
        QTemporaryDir directory;
        const auto applications = directory.filePath("applications");
        QVERIFY(QDir().mkpath(applications + "/nested"));
        QVERIFY(writeFile(applications + "/editor.desktop",
            "[Desktop Entry]\nType=Application\nName=Editor\nName[zh_TW]=編輯器\nExec=/usr/bin/editor %F\n"
            "[Desktop Action new]\nName=Other\nExec=other\n"));
        QVERIFY(writeFile(applications + "/nested/term.desktop",
            "[Desktop Entry]\nType=Application\nName=Terminal\nExec=env A=1 B=2 term --new\n"));
        QVERIFY(writeFile(applications + "/hidden.desktop", "[Desktop Entry]\nType=Application\nName=H\nExec=hidden\nNoDisplay=true\n"));
        QVERIFY(writeFile(applications + "/link.desktop", "[Desktop Entry]\nType=Link\nName=L\nExec=link\n"));
        const auto entries = PlatformSettings::desktopEntries({applications});
        QCOMPARE(entries.value("term"), QStringLiteral("Terminal"));
        QVERIFY(entries.value("editor") == QStringLiteral("Editor") || entries.value("editor") == QStringLiteral("編輯器"));
        QVERIFY(!entries.contains("other"));
        QVERIFY(!entries.contains("hidden"));
        QVERIFY(!entries.contains("link"));
    }
    void runningAppsLeaveOutListedApps() {
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        PlatformSettings settings(storage);
        const auto running = settings.runningApps();
        for (const auto &app : running) {
            const auto map = app.toMap();
            QVERIFY(!map.value("id").toString().isEmpty());
            QVERIFY(!map.value("name").toString().isEmpty());
        }
        if (!running.isEmpty()) {
            const auto id = running.first().toMap().value("id").toString();
            QVERIFY(settings.addApp(id));
            for (const auto &app : settings.runningApps()) QVERIFY(app.toMap().value("id") != id);
        }
        QCOMPARE(settings.appName("org.unknown"), QStringLiteral("org.unknown"));
    }
};
QTEST_GUILESS_MAIN(PlatformSettingsTest)
#include "PlatformSettingsTest.moc"
