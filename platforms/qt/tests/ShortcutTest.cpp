#include "Shortcuts.h"
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QtTest>

namespace {
#ifdef Q_OS_MACOS
constexpr int control = Qt::MetaModifier;
constexpr int appModifier = Qt::ControlModifier;
#else
constexpr int control = Qt::ControlModifier;
constexpr int appModifier = Qt::MetaModifier;
#endif
// Alt with a letter is free on every platform (⌥ letters are free on a Mac; Ctrl letters are the apps' elsewhere).
constexpr int freeMods = Qt::AltModifier;
constexpr auto key = "shortcuts/bindingsV2";
}
class ShortcutTest final : public QObject {
    Q_OBJECT
private slots:
    void defaultsAndNormalization() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        QCOMPARE(shortcuts.commands().size(), 11);
        QVERIFY(shortcuts.error().isEmpty());
        QVERIFY(!shortcuts.anyCustomized());
        QCOMPARE(shortcuts.resolve(Qt::Key_Tab, 0), QString("nextPage"));
        QCOMPARE(shortcuts.resolve(Qt::Key_Backtab, 0), QString("previousPage"));
        QCOMPARE(shortcuts.resolve(Qt::Key_Enter, Qt::ShiftModifier | Qt::KeypadModifier), QString("commitWithNewline"));
        QCOMPARE(shortcuts.values().value("nextPage").toList().size(), 3);
        for (const auto &command : shortcuts.commands()) {
            const auto entry = command.toMap();
            QCOMPARE(entry.value("row").toString(), "keys-" + entry.value("id").toString());
            for (const auto &binding : entry.value("defaults").toList()) {
                const auto b = binding.toMap();
                QCOMPARE(shortcuts.resolve(b.value("key").toInt(), b.value("modifiers").toInt()), entry.value("id").toString());
            }
        }
    }
    void labels() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        QCOMPARE(shortcuts.activeLabel("singleCharacterList"), QString("⇧↓"));
        QCOMPARE(shortcuts.activeLabel("typeZhuyin"), QString("`"));
        const auto next = shortcuts.values().value("nextPage").toList();
#ifdef Q_OS_MACOS
        QCOMPARE(next.at(1).toMap().value("label").toString(), QString("fn↓"));
        QCOMPARE(next.at(1).toMap().value("spoken").toString(), QString("fn ↓（Page Down）"));
        QCOMPARE(next.at(2).toMap().value("label").toString(), QString("⌥↓"));
#else
        QCOMPARE(next.at(1).toMap().value("label").toString(), QString("PgDn"));
        QCOMPARE(next.at(2).toMap().value("label").toString(), QString("Alt+↓"));
        QCOMPARE(next.at(2).toMap().value("spoken").toString(), QString("Alt+↓"));
        QCOMPARE(shortcuts.values().value("previousPage").toList().at(0).toMap().value("spoken").toString(), QString("Shift+Tab"));
#endif
        QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_U, freeMods | Qt::ShiftModifier, 0));
#ifdef Q_OS_MACOS
        QCOMPARE(shortcuts.activeLabel("typeZhuyin"), QString("⌥⇧U"));
#else
        QCOMPARE(shortcuts.activeLabel("typeZhuyin"), QString("Alt+⇧U"));
#endif
        shortcuts.setFeatures({{"feature.typeZhuyin", false}});
        QCOMPARE(shortcuts.activeLabel("typeZhuyin"), QString());
    }
    void persistsOnlyChangedCommands() {
        QTemporaryDir dir;
        const auto path = dir.filePath("settings.ini");
        {
            QSettings storage(path, QSettings::IniFormat);
            storage.setValue("unrelated", "keep");
            Shortcuts shortcuts(storage);
            QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_U, freeMods));
            QVERIFY(shortcuts.isCustomized("typeZhuyin"));
            QVERIFY(!shortcuts.isCustomized("nextPage"));
            QCOMPARE(shortcuts.values().value("typeZhuyin").toList().size(), 2);
            QVERIFY(shortcuts.remove("typeZhuyin", 0));
            QCOMPARE(shortcuts.resolve(Qt::Key_QuoteLeft, 0), QString());
            const auto stored = QJsonDocument::fromJson(storage.value(key).toByteArray()).object();
            QCOMPARE(stored.keys(), QStringList({"typeZhuyin"}));
        }
        QSettings storage(path, QSettings::IniFormat);
        Shortcuts restored(storage);
        QCOMPARE(restored.resolve(Qt::Key_U, freeMods), QString("typeZhuyin"));
        QCOMPARE(restored.values().value("typeZhuyin").toList().size(), 1);
        QVERIFY(restored.reset());
        QVERIFY(!restored.isCustomized("typeZhuyin"));
        QCOMPARE(storage.value("unrelated").toString(), QString("keep"));
    }
    void legacyStorageKeepsOnlyChanges() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        QJsonObject all;
        {
            Shortcuts defaults(storage);
            for (const auto &command : defaults.commands()) {
                QJsonArray list;
                for (const auto &binding : command.toMap().value("defaults").toList())
                    list.append(QJsonObject{{"key", binding.toMap().value("key").toInt()}, {"modifiers", binding.toMap().value("modifiers").toInt()}});
                all.insert(command.toMap().value("id").toString(), list);
            }
        }
        all.insert("typeZhuyin", QJsonArray{QJsonObject{{"key", int(Qt::Key_U)}, {"modifiers", freeMods}}});
        storage.setValue("shortcuts/bindingsV1", QJsonDocument(all).toJson());
        Shortcuts shortcuts(storage);
        QVERIFY(shortcuts.error().isEmpty());
        QVERIFY(shortcuts.isCustomized("typeZhuyin"));
        QVERIFY(!shortcuts.isCustomized("nextPage"));
        QCOMPARE(shortcuts.resolve(Qt::Key_U, freeMods), QString("typeZhuyin"));
    }
    void reservedKeysAndInvalidInputs() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        const auto original = shortcuts.values();
        QSignalSpy changed(&shortcuts, &Shortcuts::changed);
        QList<QPair<int, int>> invalid = {{Qt::Key_A, 0}, {Qt::Key_Space, 0}, {Qt::Key_Escape, 0},
            {Qt::Key_Left, 0}, {Qt::Key_Home, 0}, {Qt::Key_PageDown, 0}, {Qt::Key_Backspace, 0},
            {Qt::Key_A, Qt::ShiftModifier}, {Qt::Key_1, Qt::AltModifier}, {Qt::Key_A, appModifier},
            {Qt::Key_Comma, control}, {Qt::Key_Control, 0}, {Qt::Key_unknown, 0}, {Qt::Key_A, Qt::GroupSwitchModifier},
            {-1, 0}, {Qt::Key_A, -1}, {Qt::Key_Exclam, Qt::ShiftModifier}};
#ifndef Q_OS_MACOS
        invalid.append({Qt::Key_C, control});
        invalid.append({Qt::Key_T, control | Qt::ShiftModifier});
#endif
        for (const auto &pair : invalid) {
            QVERIFY2(!shortcuts.record("typeZhuyin", pair.first, pair.second, 0), qPrintable(QString::number(pair.first)));
            QVERIFY(!shortcuts.error().isEmpty());
            QCOMPARE(shortcuts.values(), original);
        }
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_PageDown, 0, 0));
        QVERIFY(shortcuts.error().contains("用來翻頁"));
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_Home, 0, 0));
        QVERIFY(shortcuts.error().contains("移到組字的開頭"));
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_Comma, control, 0));
        QVERIFY(shortcuts.error().contains("中文標點"));
        QVERIFY(!shortcuts.record("unknown", Qt::Key_U, freeMods));
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_U, freeMods, -2));
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_U, freeMods, 999));
        QCOMPARE(changed.count(), 0);
        // With 中文標點 off, ⌃ (Ctrl) with a symbol key is free.
        shortcuts.setFeatures({{"feature.controlPunctuation", false}});
        QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_Comma, control, 0));
    }
    void conflictsAndPartialReset() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_Tab, 0));
        QCOMPARE(shortcuts.error(), QString("已用於「下一頁」"));
        QVERIFY(!shortcuts.record("nextPage", Qt::Key_PageDown, 0, 0));
        QCOMPARE(shortcuts.error(), QString("已是這個功能的另一個按鍵"));
        QVERIFY(shortcuts.remove("nextPage", 0));
        QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_Tab, 0));
        // 還原預設 brings back the defaults no other command uses: Tab stays with 打出注音本身.
        QVERIFY(shortcuts.reset("nextPage"));
        QCOMPARE(shortcuts.values().value("nextPage").toList().size(), 2);
        QCOMPARE(shortcuts.resolve(Qt::Key_Tab, 0), QString("typeZhuyin"));
        // Every default taken: the reset says which command has it.
        QVERIFY(shortcuts.record("commitWithNewline", Qt::Key_J, freeMods, 0));
        QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_Return, Qt::ShiftModifier));
        const auto previous = shortcuts.values();
        QVERIFY(!shortcuts.reset("commitWithNewline"));
        QCOMPARE(shortcuts.error(), QString("已用於「打出注音本身」"));
        QCOMPARE(shortcuts.values(), previous);
        QVERIFY(shortcuts.reset());
        QCOMPARE(shortcuts.resolve(Qt::Key_Tab, 0), QString("nextPage"));
        QVERIFY(shortcuts.error().isEmpty());
    }
    void defaultsTakenByAnotherCommandConflict() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        storage.setValue(key, QJsonDocument(QJsonObject{{"typeZhuyin", QJsonArray{QJsonObject{
            {"key", int(Qt::Key_Return)}, {"modifiers", int(Qt::ShiftModifier)}}}}}).toJson());
        storage.setValue("shortcuts/bindingsV1", "ignored once V2 exists");
        Shortcuts shortcuts(storage);
        QVERIFY(shortcuts.error().isEmpty());
        // 送出並換行 keeps its only default although 打出注音本身 has it: one conflict, shown on both.
        QCOMPARE(shortcuts.conflicts().size(), 1);
        QCOMPARE(shortcuts.conflictsOf("commitWithNewline").value("0").toString(), QString("typeZhuyin"));
        QCOMPARE(shortcuts.conflictsOf("typeZhuyin").value("0").toString(), QString("commitWithNewline"));
        // A command whose feature is off has no conflicts.
        shortcuts.setFeatures({{"feature.typeZhuyin", false}});
        QVERIFY(shortcuts.conflicts().isEmpty());
        QCOMPARE(shortcuts.resolve(Qt::Key_Return, Qt::ShiftModifier), QString("commitWithNewline"));
    }
    void limitsAndRequiredCommands() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        for (int k = Qt::Key_A; k < Qt::Key_E; ++k) QVERIFY(shortcuts.record("typeZhuyin", k, freeMods));
        QCOMPARE(shortcuts.values().value("typeZhuyin").toList().size(), 5);
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_F, freeMods));
        QVERIFY(!shortcuts.remove("commitWithNewline", 0));
        QCOMPARE(shortcuts.error(), QString("「送出並換行」至少要有一個按鍵"));
        QVERIFY(shortcuts.record("commitWithNewline", Qt::Key_G, freeMods));
        QVERIFY(shortcuts.remove("commitWithNewline", 0));
        QVERIFY(!shortcuts.remove("commitWithNewline", 0));
        QVERIFY(!shortcuts.remove("unknown", 0));
        QVERIFY(!shortcuts.remove("typeZhuyin", -1));
        QVERIFY(!shortcuts.remove("typeZhuyin", 5));
        // A command that is not required may be left with no key.
        QVERIFY(shortcuts.remove("singleCharacterList", 0));
        QCOMPARE(shortcuts.activeLabel("singleCharacterList"), QString());
    }
    void corruptStorageIsPreserved() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        for (const QByteArray &bytes : {QByteArray("not-json"), QByteArray("{\"unknown\":[]}"),
                                        QByteArray("{\"typeZhuyin\":[{\"key\":65.5,\"modifiers\":0}]}")}) {
            storage.setValue(key, bytes);
            Shortcuts shortcuts(storage);
            QVERIFY(!shortcuts.error().isEmpty());
            QCOMPARE(shortcuts.resolve(Qt::Key_Tab, 0), QString("nextPage"));
            QCOMPARE(shortcuts.resolve(Qt::Key_QuoteLeft, 0), QString("typeZhuyin"));
            QCOMPARE(storage.value(key).toByteArray(), bytes);
        }
        storage.setValue(key, QByteArray("{}"));
        Shortcuts empty(storage);
        QVERIFY(empty.error().isEmpty());
    }
    void shiftedPunctuationCanonicalizes() {
        QTemporaryDir dir;
        QSettings storage(dir.filePath("settings.ini"), QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        QVERIFY(shortcuts.record("typeZhuyin", Qt::Key_Exclam, control, 0));
        QCOMPARE(shortcuts.resolve(Qt::Key_1, control | Qt::ShiftModifier), QString("typeZhuyin"));
        QVERIFY(!shortcuts.record("selectLeft", Qt::Key_1, control | Qt::ShiftModifier));
    }
    void failedWriteDoesNotPublish() {
        QTemporaryDir dir;
        const auto path = dir.filePath("file");
        QFile blocker(path);
        QVERIFY(blocker.open(QIODevice::WriteOnly));
        blocker.close();
        QSettings storage(path + "/settings.ini", QSettings::IniFormat);
        Shortcuts shortcuts(storage);
        const auto before = shortcuts.values();
        QSignalSpy changed(&shortcuts, &Shortcuts::changed);
        QVERIFY(!shortcuts.record("typeZhuyin", Qt::Key_U, freeMods));
        QCOMPARE(shortcuts.values(), before);
        QCOMPARE(changed.count(), 0);
        QVERIFY(!shortcuts.error().isEmpty());
    }
};
QTEST_GUILESS_MAIN(ShortcutTest)
#include "ShortcutTest.moc"
