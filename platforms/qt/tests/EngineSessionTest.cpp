#include "EngineSession.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>
#include <limits>

class EngineSessionTest final : public QObject {
    Q_OBJECT
    QString library_;
    static QString lexicon(const QTemporaryDir &directory) {
        const QString path = directory.filePath("lexicon.tsv");
        QFile file(path);
        if (!file.open(QIODevice::WriteOnly)) return {};
        const QByteArray data = QStringLiteral("ㄋㄧˇ\t你\t-3\nㄋㄧˇ\t妳\t-4\nㄋㄧˇ\t擬\t-5\nㄋㄧˇ\t旎\t-6\n"
            "ㄏㄠˇ\t好\t-3\nㄏㄠˇ\t郝\t-6\nㄋㄧˇ ㄏㄠˇ\t你好\t-2\n").toUtf8();
        return file.write(data) == data.size() ? path : QString();
    }
    static void type(EngineSession &engine, const QString &keys) {
        for (const QChar c : keys) engine.key(c.toUpper().unicode());
    }
    static bool contains(const QVariantList &list, const QString &word) {
        for (const auto &value : list) if (value.toMap().value("word") == word) return true;
        return false;
    }
private slots:
    void initTestCase() {
        library_ = qEnvironmentVariable("ZT_ENGINE_LIBRARY");
        const bool explicitLibrary = qEnvironmentVariableIsSet("ZT_ENGINE_LIBRARY");
        if (!explicitLibrary) library_ = QFINDTESTDATA("../../../engine/target/release/libzhuyin.dylib");
        if (!explicitLibrary && library_.isEmpty()) QSKIP("Build the current public C ABI library and set ZT_ENGINE_LIBRARY");
        QVERIFY2(!library_.isEmpty() && QFileInfo(library_).isFile(), "ZT_ENGINE_LIBRARY must name a current, existing C ABI library");
        library_ = QFileInfo(library_).absoluteFilePath();
    }
    void unopenedAndFailedReopen() {
        EngineSession engine;
        QVERIFY(!engine.ready());
        QVERIFY(!engine.key(Qt::Key_S));
        QVERIFY(!engine.error().isEmpty());
        QVERIFY(!engine.addPhrase("你好", "ㄋㄧˇ ㄏㄠˇ"));
        QTemporaryDir directory;
        QVERIFY(!engine.open(directory.filePath("missing.dylib"), lexicon(directory), directory.filePath("personal")));
        QVERIFY(!engine.ready());
        QVERIFY2(engine.open(library_, lexicon(directory), directory.filePath("personal")), qPrintable(engine.error()));
        type(engine, "su3");
        const QString before = engine.composition();
        QVERIFY(!engine.open(library_, directory.filePath("missing.tsv"), directory.filePath("other")));
        QVERIFY(engine.ready());
        QCOMPARE(engine.composition(), before);
        QVERIFY(engine.reset());
        QVERIFY(engine.composition().isEmpty());
    }
    void realCompositionCandidatesAndRepeatedCommits() {
        QTemporaryDir directory;
        EngineSession engine;
        QVERIFY(engine.applySettings({{"learningEnabled", false}, {"spaceAction", "candidates"}, {"candidatesPerPage", 3}}));
        QVERIFY2(engine.open(library_, lexicon(directory), directory.filePath("personal")), qPrintable(engine.error()));
        QSignalSpy commits(&engine, &EngineSession::committedText);
        QSignalSpy timings(&engine, &EngineSession::keyHandled);
        for (int i = 0; i < 2; ++i) {
            type(engine, "su3cl3");
            QCOMPARE(engine.composition(), QStringLiteral("你好"));
            QCOMPARE(engine.cursor(), 2);
            QVERIFY(engine.composing());
            QVERIFY(!engine.clauses().isEmpty());
            QVERIFY(engine.key(Qt::Key_Return));
            QCOMPARE(engine.committed(), QStringLiteral("你好"));
            QVERIFY(!engine.composing());
        }
        QCOMPARE(commits.count(), 2);
        QCOMPARE(commits.at(0).at(0).toString(), commits.at(1).at(0).toString());
        QVERIFY(timings.count() >= 14);
        for (const auto &timing : timings) QVERIFY(timing.at(0).toDouble() >= 0);
        type(engine, "su3");
        QVERIFY(engine.key(Qt::Key_Space));
        QVERIFY(engine.rows().size() >= 4);
        QCOMPARE(engine.pageSize(), 3);
        QCOMPARE(engine.page(), 0);
        QVERIFY(engine.pageCount() >= 2);
        QVERIFY(engine.key(Qt::Key_PageDown));
        QCOMPARE(engine.page(), 1);
        const int picked = engine.selected();
        const auto word = engine.rows().at(picked).toMap().value("text").toString();
        QVERIFY(!engine.choose(-1));
        QVERIFY(!engine.choose(int(engine.rows().size())));
        const int beforePick = timings.count();
        QVERIFY(engine.choose(picked));
        QCOMPARE(timings.count(), beforePick + 1);
        engine.key(Qt::Key_Return);
        QCOMPARE(engine.committed(), word);
    }
    void settingsValidationModifiersAndPassThrough() {
        QTemporaryDir directory;
        EngineSession engine;
        QVERIFY(engine.open(library_, lexicon(directory), directory.filePath("personal")));
        QVERIFY(!engine.applySettings({{"candidatesPerPage", 3.5}}));
        QVERIFY(!engine.applySettings({{"candidatesPerPage", std::numeric_limits<double>::quiet_NaN()}}));
        QVERIFY(!engine.applySettings({{"learningEnabled", "false"}}));
        QVERIFY(!engine.applySettings({{"spaceAction", "bogus"}}));
#ifdef Q_OS_MACOS
        const int command = Qt::ControlModifier;
        const int control = Qt::MetaModifier;
#else
        const int command = Qt::MetaModifier;
        const int control = Qt::ControlModifier;
#endif
        QVERIFY(!engine.key(Qt::Key_A, command));
        QVERIFY(!engine.key(Qt::Key_A, control | Qt::AltModifier));
        QVERIFY(engine.composition().isEmpty());
        QVERIFY(engine.applySettings({{"punctuationKeys", "mac"}}));
        QVERIFY(engine.key(Qt::Key_Less, Qt::ShiftModifier));
        QVERIFY((engine.composition() + engine.committed()).contains(QStringLiteral("，")));
        QVERIFY(engine.reset());
        QVERIFY(engine.applySettings({{"feature.controlPunctuation", false}}));
        QVERIFY(!engine.key(Qt::Key_Comma, control));
        QVERIFY(engine.applySettings({{"feature.controlPunctuation", true}}));
        QVERIFY(engine.key(Qt::Key_Comma, control));
        QVERIFY(engine.reset());
        QVERIFY(engine.applySettings({{"feature.deleteWord", false}}));
        type(engine, "su3");
        const auto before = engine.composition();
        QVERIFY(!engine.key(Qt::Key_Backspace, Qt::AltModifier));
        QCOMPARE(engine.composition(), before);
        QVERIFY(engine.applySettings({{"spaceAction", "commit"}}));
        QSignalSpy passthroughTimings(&engine, &EngineSession::keyHandled);
        QVERIFY(!engine.key(Qt::Key_Space));
        QCOMPARE(passthroughTimings.count(), 1);
        QCOMPARE(engine.committed(), QStringLiteral("你"));
        QVERIFY(engine.composition().isEmpty());
    }
    void fuzzySoundsChangeConversion() {
        QTemporaryDir directory;
        const QString path = directory.filePath("lexicon.tsv");
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(QStringLiteral("ㄕㄨ\t書\t-3\nㄙㄨ\t蘇\t-4\nㄈㄨˊ\t服\t-3\nㄈㄨˊ\t福\t-4\n"
            "ㄕㄨ ㄈㄨˊ\t舒服\t-2\n").toUtf8());
        file.close();
        const auto converts = [](EngineSession &engine) {
            engine.reset();
            type(engine, "nj zj6");  // ㄙㄨ ㄈㄨˊ
            bool found = engine.composition().contains(QStringLiteral("舒服"));
            if (!found && engine.key(Qt::Key_Down))
                for (const auto &row : engine.rows()) found |= row.toMap().value("text").toString() == QStringLiteral("舒服");
            engine.reset();
            return found;
        };
        // Settings applied before the engine opens.
        EngineSession early;
        QVERIFY(early.applySettings({{"learningEnabled", false}, {"fuzzy.sSh", true}}));
        QVERIFY2(early.open(library_, path, directory.filePath("early")), qPrintable(early.error()));
        QVERIFY(converts(early));
        // Toggled on a live engine, off and on again.
        EngineSession engine;
        QVERIFY(engine.applySettings({{"learningEnabled", false}}));
        QVERIFY2(engine.open(library_, path, directory.filePath("live")), qPrintable(engine.error()));
        QVERIFY(!converts(engine));
        QVERIFY(engine.applySettings({{"fuzzy.sSh", true}}));
        QVERIFY(converts(engine));
        QVERIFY(engine.applySettings({{"fuzzy.sSh", false}}));
        QVERIFY(!converts(engine));
        QVERIFY(engine.applySettings({{"fuzzy.zZh", true}, {"fuzzy.enEng", true}}));
        QVERIFY(!converts(engine));
    }
    void commitAllAndSecureLearning() {
        QTemporaryDir directory;
        EngineSession engine;
        QVERIFY(engine.open(library_, lexicon(directory), directory.filePath("personal")));
        QSignalSpy commits(&engine, &EngineSession::committedText);
        QSignalSpy timings(&engine, &EngineSession::keyHandled);
        type(engine, "su3cl3");
        QVERIFY(engine.key(Qt::Key_Down));
        QVERIFY(!engine.rows().isEmpty());
        const QString before = engine.composition();
        const int beforeCommit = timings.count();
        QVERIFY(engine.commitAll());
        QCOMPARE(timings.count(), beforeCommit + 1);
        QCOMPARE(engine.committed(), before);
        QCOMPARE(commits.count(), 1);
        QVERIFY(engine.rows().isEmpty());
        QVERIFY(!engine.composing());
        QVERIFY(engine.commitAll());
        QCOMPARE(commits.count(), 1);
        QVERIFY(engine.setLearningSuspended(true));
        QVERIFY(engine.setPrecedingText("前文"));
        type(engine, "su3");
        QVERIFY(engine.key(Qt::Key_Down));
        QVERIFY(engine.choose(1));
        QVERIFY(engine.commitAll());
        QVERIFY(engine.learnedEntries().isEmpty());
        QVERIFY(engine.setLearningSuspended(false));
        QVERIFY(!engine.setPrecedingText(QString("前") + QChar::Null + "文"));
        QVERIFY(engine.setPrecedingText(""));
        type(engine, "su3");
        QVERIFY(engine.key(Qt::Key_Down));
        QVERIFY(engine.choose(1));
        QVERIFY(engine.commitAll());
        QVERIFY(contains(engine.learnedEntries(), "妳"));
    }
    void personalRoundTripAndValidation() {
        QTemporaryDir directory;
        const QString path = lexicon(directory), user = directory.filePath("personal");
        const QUrl backup = QUrl::fromLocalFile(directory.filePath("backup.txt"));
        {
            EngineSession engine;
            QVERIFY(engine.open(library_, path, user));
            QVERIFY(!engine.addPhrase("", ""));
            QVERIFY(!engine.addPhrase(QString("壞") + QChar::Null + "詞", "ㄏㄠˇ"));
            QVERIFY(!engine.addPhrase("好", "invalid"));
            QVERIFY2(engine.addPhrase("郝好", "ㄏㄠˇ ㄏㄠˇ"), qPrintable(engine.error()));
            QVERIFY(contains(engine.userPhrases(), "郝好"));
            QCOMPARE(engine.userPhrases().first().toMap().value("reading").toString(), QStringLiteral("ㄏㄠˇ ㄏㄠˇ"));
            QVERIFY(engine.userPhrases().first().toMap().contains("count"));
            type(engine, "su3");
            QVERIFY(engine.key(Qt::Key_Down));
            QVERIFY(engine.choose(1));
            QVERIFY(engine.key(Qt::Key_Return));
            QVERIFY(contains(engine.learnedEntries(), "妳"));
            QVERIFY(engine.excludeLearned("妳", "ㄋㄧˇ"));
            QVERIFY(!contains(engine.learnedEntries(), "妳"));
            QVERIFY(contains(engine.excludedEntries(), "妳"));
            QVERIFY(engine.exportPersonal(backup));
            QVERIFY(!engine.exportPersonal(QUrl("https://example.invalid/backup")));
            QVERIFY(engine.removePhrase("郝好", "ㄏㄠˇ ㄏㄠˇ"));
            QVERIFY(engine.includeLearned("妳", "ㄋㄧˇ"));
            QVERIFY(engine.userPhrases().isEmpty());
            QVERIFY(engine.excludedEntries().isEmpty());
            QVERIFY2(engine.importPersonal(backup) >= 2, qPrintable(engine.error()));
            QVERIFY(contains(engine.userPhrases(), "郝好"));
            QVERIFY(contains(engine.excludedEntries(), "妳"));
            QVERIFY(engine.includeLearned("妳", "ㄋㄧˇ"));
            type(engine, "su3"); engine.key(Qt::Key_Down); engine.choose(1); engine.key(Qt::Key_Return);
            QVERIFY(engine.forgetLearned("妳", "ㄋㄧˇ"));
            QVERIFY(!contains(engine.learnedEntries(), "妳"));
            const QUrl bad = QUrl::fromLocalFile(directory.filePath("bad.txt"));
            QFile file(bad.toLocalFile()); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("not json"); file.close();
            QCOMPARE(engine.importPersonal(bad), -1);
            QVERIFY(contains(engine.userPhrases(), "郝好"));
        }
        EngineSession reopened;
        QVERIFY(reopened.open(library_, path, user));
        QVERIFY(contains(reopened.userPhrases(), "郝好"));
        QVERIFY(reopened.learnedEntries().isEmpty());
    }
    void readingsEditForgetAllAndLoadTimes() {
        QTemporaryDir directory;
        EngineSession engine;
        QVERIFY(engine.readings("你好").isEmpty());
        QVERIFY(!engine.forgetAllLearned());
        QVERIFY(engine.loadTimes().value("lexicon").isNull());
        QVERIFY(engine.open(library_, lexicon(directory), directory.filePath("personal")));
        QVERIFY(engine.loadTimes().value("lexicon").toDouble() >= 0);
        QVERIFY(!engine.loadTimes().value("lexicon").isNull());
        QVERIFY(engine.loadTimes().value("reranker").isNull());
        QVERIFY(engine.readings("你好").contains(QStringLiteral("ㄋㄧˇ ㄏㄠˇ")));
        QVERIFY(engine.readings("").isEmpty());
        QVERIFY(engine.addPhrase("郝好", "ㄏㄠˇ ㄏㄠˇ"));
        QVERIFY2(engine.editPhrase("郝好", "ㄏㄠˇ ㄏㄠˇ", "好你", "ㄏㄠˇ ㄋㄧˇ"), qPrintable(engine.error()));
        QVERIFY(contains(engine.userPhrases(), "好你"));
        QVERIFY(!contains(engine.userPhrases(), "郝好"));
        // A rejected new phrase keeps the old one.
        QVERIFY(!engine.editPhrase("好你", "ㄏㄠˇ ㄋㄧˇ", "好", "invalid"));
        QVERIFY(contains(engine.userPhrases(), "好你"));
        type(engine, "su3");
        QVERIFY(engine.key(Qt::Key_Down));
        QVERIFY(engine.choose(1));
        QVERIFY(engine.key(Qt::Key_Return));
        QVERIFY(contains(engine.learnedEntries(), "妳"));
        QVERIFY(engine.excludeLearned("你", "ㄋㄧˇ"));
        QVERIFY(engine.forgetAllLearned());
        QVERIFY(engine.learnedEntries().isEmpty());
        QVERIFY(contains(engine.userPhrases(), "好你"));
        QVERIFY(contains(engine.excludedEntries(), "你"));
    }
    void failedFlushIsReportedAndRetryable() {
        QTemporaryDir directory;
        const QString path = lexicon(directory), user = directory.filePath("personal");
        EngineSession engine;
        QVERIFY(engine.open(library_, path, user));
        QVERIFY(QDir(user).removeRecursively());
        QFile blocker(user); QVERIFY(blocker.open(QIODevice::WriteOnly)); blocker.close();
        QVERIFY(!engine.addPhrase("郝好", "ㄏㄠˇ ㄏㄠˇ"));
        QVERIFY(!engine.error().isEmpty());
        QVERIFY(contains(engine.userPhrases(), "郝好"));
        QVERIFY(QFile::remove(user)); QVERIFY(QDir().mkpath(user));
        QVERIFY2(engine.flush(), qPrintable(engine.error()));
        EngineSession reopened;
        QVERIFY(reopened.open(library_, path, user));
        QVERIFY(contains(reopened.userPhrases(), "郝好"));
    }
};
QTEST_GUILESS_MAIN(EngineSessionTest)
#include "EngineSessionTest.moc"
