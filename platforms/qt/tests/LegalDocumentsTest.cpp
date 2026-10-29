#include "LegalDocuments.h"
#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <QtTest>

namespace {
bool writeFile(const QString &path, const QByteArray &bytes) {
    QDir().mkpath(QFileInfo(path).absolutePath());
    QFile file(path);
    return file.open(QIODevice::WriteOnly) && file.write(bytes) == bytes.size();
}
const QByteArray Sources = R"({"version": 1,
  "groups": [{"id": "app", "title": "ZhuyinType"}, {"id": "libraries", "title": "程式庫"}, {"id": "models", "title": "模型"}],
  "entries": [
    {"id": "zhuyintype", "group": "app", "name": "ZhuyinType", "use": "輸入法程式", "distribution": "bundled",
     "license": "AGPL-3.0-only", "licenseFiles": ["AGPL-3.0.txt", "NOTICE.txt"]},
    {"id": "sparkle", "group": "libraries", "name": "Sparkle", "use": "更新", "distribution": "bundled", "license": "MIT",
     "licenseFiles": ["Sparkle-LICENSE.txt"], "platforms": ["macos"]},
    {"id": "qt", "group": "libraries", "name": "Qt 6", "use": "介面", "distribution": "bundled", "license": "LGPL-3.0-only",
     "licenseFiles": ["LGPL-3.0.txt", "missing.txt"], "platforms": ["linux", "windows"]},
    {"id": "fcitx", "group": "libraries", "name": "JSON", "use": "外掛", "distribution": "bundled", "license": "MIT",
     "licenseFiles": ["json.txt"], "platforms": ["linux"]},
    {"id": "lm", "group": "models", "name": "模型", "use": "語言模型", "distribution": "bundled", "license": "CC BY-SA 4.0",
     "licenseFiles": [], "notices": ["lm/NOTICE.md", "lm/articles.tsv.gz", "absent/NOTICE.md", "../escape.md"]}
  ]})";
}

class LegalDocumentsTest : public QObject {
    Q_OBJECT
    QTemporaryDir directory_;
    QString licenses() const { return directory_.filePath("Licenses"); }
    QString data() const { return directory_.filePath("data"); }
    static QStringList names(const QVariantMap &sources) {
        QStringList result;
        for (const auto &group : sources.value("groups").toList())
            for (const auto &entry : group.toMap().value("entries").toList()) result.append(entry.toMap().value("id").toString());
        return result;
    }
private slots:
    void initTestCase() {
        QVERIFY(writeFile(licenses() + "/sources.json", Sources));
        for (const auto *name : {"AGPL-3.0.txt", "NOTICE.txt", "Sparkle-LICENSE.txt", "LGPL-3.0.txt", "json.txt"})
            QVERIFY(writeFile(licenses() + "/" + name, QByteArray("text of ") + name));
        QVERIFY(writeFile(data() + "/lm/NOTICE.md", "# 署名"));
        QVERIFY(writeFile(data() + "/lm/articles.tsv.gz", "gz"));
        QVERIFY(writeFile(directory_.filePath("escape.md"), "secret"));
    }
    void filtersByPlatform() {
        QCOMPARE(names(LegalDocuments(licenses(), data(), "linux").sources()), QStringList({"zhuyintype", "qt", "fcitx", "lm"}));
        QCOMPARE(names(LegalDocuments(licenses(), data(), "windows").sources()), QStringList({"zhuyintype", "qt", "lm"}));
        QCOMPARE(names(LegalDocuments(licenses(), data(), "macos").sources()), QStringList({"zhuyintype", "sparkle", "lm"}));
        // The Qt preview elsewhere shows what Linux and Windows ship.
        QCOMPARE(names(LegalDocuments(licenses(), data(), "").sources()), QStringList({"zhuyintype", "qt", "fcitx", "lm"}));
    }
    void filesAndTexts() {
        LegalDocuments documents(licenses(), data(), "linux");
        const auto groups = documents.sources().value("groups").toList();
        QCOMPARE(groups.size(), 3);
        const auto qt = groups[1].toMap().value("entries").toList()[0].toMap();
        QCOMPARE(qt.value("version").toString(), QString::fromLatin1(qVersion()));
        QCOMPARE(qt.value("files").toList().size(), 1);  // missing.txt is left out
        const auto lm = groups[2].toMap().value("entries").toList()[0].toMap();
        const auto files = lm.value("files").toList();
        QCOMPARE(files.size(), 2);
        QCOMPARE(files[0].toMap().value("kind").toString(), QStringLiteral("markdown"));
        QCOMPARE(files[1].toMap().value("kind").toString(), QStringLiteral("folder"));
        QCOMPARE(documents.text("LGPL-3.0.txt"), QStringLiteral("text of LGPL-3.0.txt"));
        QCOMPARE(documents.text("lm/NOTICE.md"), QStringLiteral("# 署名"));
        QVERIFY(documents.text("lm/articles.tsv.gz").isEmpty());
        QVERIFY(documents.text("Sparkle-LICENSE.txt").isEmpty());  // not listed on Linux
        QVERIFY(documents.text("../escape.md").isEmpty());
        QVERIFY(documents.text("sources.json").isEmpty());
        QVERIFY(!documents.reveal("LGPL-3.0.txt"));
    }
    void unreadableList() {
        QTemporaryDir empty;
        LegalDocuments documents(empty.path(), data(), "linux");
        QVERIFY(documents.sources().value("groups").toList().isEmpty());
        QVERIFY(!documents.sources().value("error").toString().isEmpty());
    }
};
QTEST_GUILESS_MAIN(LegalDocumentsTest)
#include "LegalDocumentsTest.moc"
