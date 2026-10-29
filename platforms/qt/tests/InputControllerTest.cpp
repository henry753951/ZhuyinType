#include "InputController.h"
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>
#include <algorithm>

namespace {
#ifdef Q_OS_MACOS
constexpr int control = Qt::MetaModifier;
constexpr int appModifier = Qt::ControlModifier;
#else
constexpr int control = Qt::ControlModifier;
constexpr int appModifier = Qt::MetaModifier;
#endif
struct Fixture {
    QTemporaryDir directory;
    QSettings storage{directory.filePath("settings.ini"), QSettings::IniFormat};
    Settings settings{storage};
    Shortcuts shortcuts{storage};
    EngineSession engine;
    InputController input{engine, settings, shortcuts};
    bool open(const QString &library) {
        QFile file(directory.filePath("lexicon.tsv"));
        if (!file.open(QIODevice::WriteOnly)) return false;
        const QByteArray lexicon = QStringLiteral("ㄋㄧˇ\t你\t-3\nㄋㄧˇ\t妳\t-4\nㄏㄠˇ\t好\t-3\n"
            "ㄋㄧˇ ㄏㄠˇ\t你好\t-2\nㄕㄨ\t書\t-3\nㄙㄨ\t蘇\t-4\nㄈㄨˊ\t服\t-3\nㄕㄨ ㄈㄨˊ\t舒服\t-2\n").toUtf8();
        if (file.write(lexicon) != lexicon.size()) return false;
        file.close();
        QObject::connect(&settings, &Settings::changed, &engine, [this] { engine.applySettings(settings.values()); });
        return engine.applySettings(settings.values())
            && engine.open(library, file.fileName(), directory.filePath("personal"));
    }
    void type(const QString &keys = "su3cl3") {
        for (const QChar c : keys) input.press(c.toUpper().unicode(), 0);
    }
};
}

class InputControllerTest final : public QObject {
    Q_OBJECT
    QString library_;
private slots:
    void initTestCase() {
        library_ = qEnvironmentVariable("ZT_ENGINE_LIBRARY");
        if (!qEnvironmentVariableIsSet("ZT_ENGINE_LIBRARY"))
            library_ = QFINDTESTDATA("../../../engine/target/release/libzhuyin.dylib");
        QVERIFY2(!library_.isEmpty() && QFileInfo(library_).isFile(), "Set ZT_ENGINE_LIBRARY to the current real C ABI library");
        library_ = QFileInfo(library_).absoluteFilePath();
    }
    void passThroughCommits_data() {
        QTest::addColumn<QString>("command");
        QTest::addColumn<QString>("feature");
        QTest::addColumn<int>("key");
        QTest::addColumn<int>("mods");
        QTest::newRow("disabled-delete") << QString() << QString("feature.deleteWord") << int(Qt::Key_Backspace) << int(Qt::AltModifier);
        QTest::newRow("disabled-punctuation") << QString() << QString("feature.controlPunctuation") << int(Qt::Key_Comma) << control;
        QTest::newRow("removed-tab") << QString("nextPage") << QString() << int(Qt::Key_Tab) << 0;
        QTest::newRow("remapped-backtab") << QString("previousPage") << QString() << int(Qt::Key_Backtab) << 0;
        QTest::newRow("remapped-keypad-enter") << QString("commitWithNewline") << QString() << int(Qt::Key_Enter) << int(Qt::ShiftModifier | Qt::KeypadModifier);
        QTest::newRow("app-shortcut") << QString() << QString() << int(Qt::Key_C) << appModifier;
        QTest::newRow("unmapped-function") << QString() << QString() << int(Qt::Key_F1) << 0;
    }
    void passThroughCommits() {
        QFETCH(QString, command); QFETCH(QString, feature); QFETCH(int, key); QFETCH(int, mods);
        Fixture f;
        QVERIFY2(f.open(library_), qPrintable(f.engine.error()));
        if (!feature.isEmpty()) QVERIFY(f.settings.setValue(feature, false));
        if (command == "nextPage") QVERIFY(f.shortcuts.remove(command, 0));
        else if (!command.isEmpty()) QVERIFY(f.shortcuts.record(command, Qt::Key_J, Qt::AltModifier, 0));
        f.type();
        QCOMPARE(f.engine.composition(), QStringLiteral("你好"));
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        QVERIFY(!f.input.press(key, mods));
        QVERIFY2(!f.engine.composing(), "A host-owned key must not leave the previous composition stranded");
        QCOMPARE(sent.count(), 1);
        QCOMPARE(sent.first().first().toString(), QStringLiteral("你好"));
    }
    void shiftTapRules() {
        Fixture f; QVERIFY(f.open(library_));
        QSignalSpy mode(&f.input, &InputController::modeChanged);
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier | control);
        f.input.release(Qt::Key_Shift, control);
        QVERIFY(!f.input.english());
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        QTest::qWait(380);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(!f.input.english());
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, true);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(!f.input.english());
        f.type();
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(f.input.english());
        QCOMPARE(mode.count(), 1);
        QCOMPARE(sent.count(), 1);
        QVERIFY(!f.engine.composing());
        f.input.release(Qt::Key_Shift, 0);
        QCOMPARE(mode.count(), 1);
    }
    void remappedCommandWaitsForModifierChord() {
        Fixture f; QVERIFY(f.open(library_));
        QVERIFY(f.shortcuts.record("deleteWord", Qt::Key_D, Qt::AltModifier, 0));
        f.type();
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        QVERIFY(!f.input.press(Qt::Key_Alt, Qt::AltModifier));
        QCOMPARE(f.engine.composition(), QStringLiteral("你好"));
        QVERIFY(f.input.press(Qt::Key_D, Qt::AltModifier));
        QVERIFY(f.engine.composition().isEmpty());
        QCOMPARE(sent.count(), 0);
    }
    void remappedNewlineInsertsAfterCommit() {
        Fixture f; QVERIFY(f.open(library_));
        QVERIFY(f.shortcuts.record("commitWithNewline", Qt::Key_J, Qt::AltModifier, 0));
        QStringList inserted;
        connect(&f.engine, &EngineSession::committedText, &f.input, [&](const QString &text) { inserted.append(text); });
        connect(&f.input, &InputController::insertTextRequested, &f.input, [&](const QString &text) { inserted.append(text); });
        f.type();
        QVERIFY(f.input.press(Qt::Key_J, Qt::AltModifier));
        QCOMPARE(inserted, QStringList({QStringLiteral("你好"), QStringLiteral("\n")}));
        QVERIFY(!f.engine.composing());
        inserted.clear();
        QVERIFY(f.shortcuts.reset("commitWithNewline"));
        f.type();
        QVERIFY(!f.input.press(Qt::Key_Return, Qt::ShiftModifier));
        QCOMPARE(inserted, QStringList({QStringLiteral("你好")}));
    }
    void shiftSideAndOverlappingKeys() {
        Fixture f; QVERIFY(f.open(library_));
        QVERIFY(f.settings.setValue("shiftToggle", "left"));
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(!f.input.english());
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, false, InputController::RightShift);
        f.input.release(Qt::Key_Shift, 0, InputController::RightShift);
        QVERIFY(!f.input.english());
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, false, InputController::LeftShift);
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, false, InputController::RightShift);
        f.input.release(Qt::Key_Shift, Qt::ShiftModifier, InputController::LeftShift);
        f.input.release(Qt::Key_Shift, 0, InputController::RightShift);
        QVERIFY(!f.input.english());
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, false, InputController::LeftShift);
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier, false, InputController::LeftShift);
        f.input.release(Qt::Key_Shift, 0, InputController::LeftShift);
        QVERIFY(f.input.english());
    }
    void capsLockObservedStateAndDisabledFeature() {
        Fixture f; QVERIFY(f.open(library_));
        QVERIFY(f.settings.setValue("feature.capsLockEnglish", false));
        f.type();
        f.input.press(Qt::Key_CapsLock, 0);
        QVERIFY(!f.input.english());
        QCOMPARE(f.engine.composition(), QStringLiteral("你好"));
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        QVERIFY(f.settings.setValue("feature.capsLockEnglish", true));
        QVERIFY(f.input.english());
        QCOMPARE(sent.count(), 1);
        f.input.press(Qt::Key_CapsLock, 0, true);
        QVERIFY(f.input.english());
        QVERIFY(!f.input.press(Qt::Key_S, 0));
        f.input.press(Qt::Key_CapsLock, 0);
        QVERIFY(!f.input.english());
        f.type("su3");
        QVERIFY(f.engine.composing());
    }
    void englishRejectsCandidateClicks() {
        Fixture f; QVERIFY(f.open(library_));
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(f.input.english());
        for (const QChar c : QString("su3")) f.engine.key(c.toUpper().unicode());
        f.engine.key(Qt::Key_Down);
        QVERIFY(!f.engine.rows().isEmpty());
        const QString before = f.engine.composition();
        QSignalSpy picked(&f.input, &InputController::candidateChosen);
        QVERIFY(!f.input.choose(1));
        QCOMPARE(f.engine.composition(), before);
        QCOMPARE(picked.count(), 0);
    }
    void focusLossCancelsPendingTapAndCommitsOnce() {
        Fixture f; QVERIFY(f.open(library_));
        f.type("su3");
        f.input.press(Qt::Key_Down, 0);
        QVERIFY(!f.engine.rows().isEmpty());
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        f.input.deactivate();
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(!f.input.english());
        QVERIFY(!f.engine.composing());
        QCOMPARE(sent.count(), 1);
        f.input.deactivate();
        QCOMPARE(sent.count(), 1);
    }
    void secureTransitionsDiscardAndNeverCommitToHost() {
        Fixture f; QVERIFY(f.open(library_));
        f.type();
        QSignalSpy sent(&f.engine, &EngineSession::committedText);
        f.input.setSecure(true);
        QVERIFY(!f.engine.composing());
        QVERIFY(!f.input.press(Qt::Key_S, 0));
        QVERIFY(!f.input.choose(0));
        f.input.press(Qt::Key_Shift, Qt::ShiftModifier);
        f.input.release(Qt::Key_Shift, 0);
        QVERIFY(!f.input.english());
        // An external caller must not leak a stale secure composition during focus loss.
        for (const QChar c : QString("su3")) f.engine.key(c.toUpper().unicode());
        QVERIFY(f.engine.composing());
        f.input.deactivate();
        QCOMPARE(sent.count(), 0);
        for (const QChar c : QString("su3")) f.engine.key(c.toUpper().unicode());
        f.input.setSecure(false);
        QVERIFY(!f.engine.composing());
        QCOMPARE(sent.count(), 0);
        f.type("su3"); f.input.press(Qt::Key_Down, 0); QVERIFY(f.input.choose(1));
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QVERIFY(!f.engine.learnedEntries().isEmpty());
    }
    void fuzzySettingReachesConversion() {
        Fixture f;
        QVERIFY2(f.open(library_), qPrintable(f.engine.error()));
        // ㄙㄨ ㄈㄨˊ: as typed, no 舒服 (read ㄕㄨ ㄈㄨˊ) unless ㄙ and ㄕ are fuzzy (typing corrections off: they too
        // would offer it in the ⇧↑ list).
        QVERIFY(f.settings.setValue("typoTolerance", "off"));
        const auto typed = [&f] {
            f.input.deactivate();
            f.type("nj zj6");
            bool found = f.engine.composition() == QStringLiteral("舒服");
            if (f.input.press(Qt::Key_Up, Qt::ShiftModifier))
                for (const auto &row : f.engine.rows()) found |= row.toMap().value("text") == QStringLiteral("舒服");
            f.input.deactivate();
            return found;
        };
        QVERIFY(!typed());
        QVERIFY(f.settings.setValue("fuzzy.sSh", true));
        QVERIFY2(f.engine.error().isEmpty(), qPrintable(f.engine.error()));
        QVERIFY(typed());
        QVERIFY(f.settings.setValue("fuzzy.sSh", false));
        QVERIFY(!typed());
        // A fresh process reads the saved toggle back from storage and opens with it.
        QVERIFY(f.settings.setValue("fuzzy.sSh", true));
        f.storage.sync();
        QSettings reread(f.directory.filePath("settings.ini"), QSettings::IniFormat);
        Settings restored(reread);
        QCOMPARE(restored.values().value("fuzzy.sSh"), QVariant(true));
        EngineSession engine;
        QVERIFY2(engine.applySettings(restored.values()), qPrintable(engine.error()));
        QVERIFY2(engine.open(library_, f.directory.filePath("lexicon.tsv"), f.directory.filePath("restored")), qPrintable(engine.error()));
        for (const QChar c : QStringLiteral("nj zj6")) engine.key(c.toUpper().unicode());
        QCOMPARE(engine.composition(), QStringLiteral("舒服"));
    }
    void usageEventsFollowMacStats() {
        Fixture f; QVERIFY(f.open(library_));
        QVERIFY(f.settings.setValue("recordsUsageStats", true));
        QStringList kinds;
        QVariantMap pick, commit;
        connect(&f.input, &InputController::usageEvent, &f.input, [&](const QString &kind, const QVariantMap &data) {
            kinds << kind;
            if (kind == "pick") pick = data;
            if (kind == "commit") commit = data;
        });
        f.type("su3");
        QVERIFY(kinds.contains("key"));
        QVERIFY(kinds.contains("interval"));
        QVERIFY(f.input.press(Qt::Key_Down, 0));
        QVERIFY(f.input.press(Qt::Key_2, 0));
        QCOMPARE(pick.value("list").toString(), QString("normal"));
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QCOMPARE(commit.value("characters").toInt(), 1);
        QVERIFY(commit.value("compositionEnded").toBool());
        QCOMPARE(commit.value("clean").toBool(), false);
        kinds.clear();
        f.type();
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QCOMPARE(commit.value("clean").toBool(), true);
        QVERIFY(!f.input.press(Qt::Key_Backspace, 0));
        QVERIFY(kinds.contains("deleteAfterCommit"));
        // Nothing is counted with the setting off, or under secure input.
        QVERIFY(f.settings.setValue("recordsUsageStats", false));
        kinds.clear();
        f.type("su3");
        QVERIFY(kinds.isEmpty());
    }
    void correctionLogWritesMacRecords() {
        Fixture f; QVERIFY(f.open(library_));
        CorrectionLog log(f.settings, f.directory.filePath("user"));
        f.input.setCorrectionLog(&log, "test");
        QVERIFY(f.settings.setValue("recordsCorrections", true));
        f.type("su3");
        QVERIFY(f.input.press(Qt::Key_Down, 0));
        QVERIFY(f.input.press(Qt::Key_2, 0));
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QCOMPARE(log.count(), 1);
        QFile file(QFileInfo(log.location().toLocalFile()).absoluteFilePath());
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto line = file.readLine();
        // Windows cannot delete a file another handle has open; clearLog() below must not meet this one.
        file.close();
        QVERIFY(line.endsWith('\n'));
        const auto record = QJsonDocument::fromJson(line).object();
        QCOMPARE(record.value("schema").toInt(), 1);
        QCOMPARE(record.value("app_version").toString(), QString("test"));
        QCOMPARE(record.value("corrections").toArray().first().toString(), QString("pick"));
        QVERIFY(record.value("keys").toString().startsWith("s<dt:"));
        QVERIFY(record.value("keys").toString().contains("<down>"));
        QCOMPARE(record.value("typing").toString().left(1), QString("s"));
        QVERIFY(record.value("settings").toObject().contains("typo_tolerance"));
        QVERIFY(!record.contains("context"));
        const auto events = record.value("events").toArray();
        QCOMPARE(events.last().toObject().value("type").toString(), QString("commit"));
        bool picked = false;
        for (const auto &event : events) picked |= event.toObject().value("type") == "pick" && event.toObject().value("rank") == 2;
        QVERIFY(picked);
        // A composition with no correction is not recorded; the log is cleared on request.
        f.type();
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QCOMPARE(log.count(), 1);
        QVERIFY(log.clearLog());
        QCOMPARE(log.count(), 0);
    }
    void precedingTextAndSameAppCarryOver() {
        Fixture f; QVERIFY(f.open(library_));
        qint64 clock = 1'000'000;
        f.input.now = [&clock] { return clock; };
        f.input.setClient("editor", QStringLiteral("我說"));
        f.type();
        QCOMPARE(f.input.recentText(), QStringLiteral("我說"));
        QVERIFY(f.input.press(Qt::Key_Return, 0));
        QCOMPARE(f.input.recentText(), QStringLiteral("我說你好"));
        // Another app, then back without the app's text: what was sent there within 10 minutes is the context.
        f.input.setClient("other", std::nullopt);
        f.type("su3");
        QCOMPARE(f.input.recentText(), QString());
        f.input.deactivate();
        f.input.setClient("editor", std::nullopt);
        clock += 9 * 60'000;
        f.type("su3");
        QVERIFY(f.input.recentText().startsWith(QStringLiteral("我說你好")));
        f.input.deactivate();
        f.input.setClient("x", std::nullopt);
        f.input.setClient("editor", std::nullopt);
        clock += 11 * 60'000;
        f.type("su3");
        QCOMPARE(f.input.recentText(), QString());
        // Secure input forgets it.
        f.input.deactivate();
        f.input.setSecure(true);
        QCOMPARE(f.input.recentText(), QString());
    }
    void dispatchLatency() {
        Fixture f; QVERIFY(f.open(library_));
        QList<qint64> samples;
        for (int i = 0; i < 300; ++i) {
            QElapsedTimer clock; clock.start();
            f.input.press(Qt::Key_F1, 0);
            samples.append(clock.nsecsElapsed());
        }
        std::sort(samples.begin(), samples.end());
        qInfo("Idle host-key dispatch ns p50=%lld p95=%lld p99=%lld", static_cast<long long>(samples[150]),
              static_cast<long long>(samples[285]), static_cast<long long>(samples[297]));
    }
};
QTEST_GUILESS_MAIN(InputControllerTest)
#include "InputControllerTest.moc"
