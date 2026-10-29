#include "Candidates.h"
#include "Settings.h"
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>
#include <limits>

class StateTest : public QObject {
    Q_OBJECT
private slots:
    void emptyCandidatesAndBoundaryIntents() {
        Candidates candidates;
        QCOMPARE(candidates.selected(), -1);
        QVERIFY(candidates.rows().isEmpty());
        QSignalSpy boundaries(&candidates, &Candidates::boundaryReached);
        QSignalSpy changes(&candidates, &Candidates::changed);
        candidates.turnPage(1);
        candidates.moveSelection(-1);
        QCOMPARE(boundaries.size(), 0);
        candidates.present({QVariantMap{{"text", "字"}}}, "");
        const auto revision = candidates.revision();
        changes.clear();
        candidates.turnPage(0);
        candidates.moveSelection(0);
        QCOMPARE(boundaries.size(), 0);
        candidates.turnPage(std::numeric_limits<int>::min());
        candidates.moveSelection(std::numeric_limits<int>::max());
        QCOMPARE(boundaries.size(), 2);
        QCOMPARE(boundaries[0][0].toInt(), -1);
        QCOMPARE(boundaries[1][0].toInt(), 1);
        QCOMPARE(changes.size(), 0);
        QCOMPARE(candidates.revision(), revision);
        QCOMPARE(candidates.page(), 0);
        QCOMPARE(candidates.selected(), 0);
    }
    void resizePreservesAbsoluteSelection() {
        Candidates candidates;
        QVariantList entries;
        for (int i = 0; i < 23; ++i) entries.append(QVariantMap{{"text", QString::number(i)}});
        candidates.present(entries, "", 22);
        QSignalSpy picks(&candidates, &Candidates::picked);
        for (int size = 3; size <= 9; ++size) {
            candidates.setPageSize(size);
            QCOMPARE(candidates.page() * size + candidates.selected(), 22);
            candidates.pick(candidates.selected());
            QCOMPARE(picks.last().first().toInt(), 22);
        }
        candidates.present({}, "");
        candidates.setPageSize(3);
        QCOMPARE(candidates.page(), 0);
        QCOMPARE(candidates.selected(), -1);
    }
    void revisionsSeparateListsFromNavigation() {
        Candidates candidates;
        QVariantList entries;
        for (int i = 0; i < 12; ++i) entries.append(QVariantMap{{"text", QString::number(i)}});
        candidates.present(entries, "ㄇㄧㄥˊ");
        const auto revision = candidates.revision();
        candidates.turnPage(1);
        candidates.moveSelection(1);
        candidates.pick(0);
        QCOMPARE(candidates.revision(), revision);
        candidates.present(entries, "ㄇㄧㄥˊ", 10);
        QCOMPARE(candidates.revision(), revision);
        entries[0] = QVariantMap{{"text", "名"}};
        candidates.present(entries, "ㄇㄧㄥˊ");
        QVERIFY(candidates.revision() > revision);
        const auto updated = candidates.revision();
        candidates.setPageSize(4);
        QVERIFY(candidates.revision() > updated);
    }
    void settingsValidateAndPersist() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto path = directory.filePath("settings.ini");
        {
            QSettings storage(path, QSettings::IniFormat);
            Settings settings(storage);
            QVERIFY(!settings.setValue("candidateFontSize", 29));
            QVERIFY(!settings.setValue("candidateFontSize", 18.5));
            QVERIFY(!settings.setValue("candidateFontSize", std::numeric_limits<double>::quiet_NaN()));
            QVERIFY(!settings.setValue("candidateFontSize", "18"));
            QVERIFY(!settings.setValue("panelAppearance", "glass"));
            QVERIFY(!settings.setValue("showsZhuyinHint", "false"));
            QVERIFY(!settings.setValue("unknown", true));
            QVERIFY(settings.setValue("candidateFontSize", 24));
            QVERIFY(settings.setValue("showsZhuyinHint", false));
            QVERIFY(settings.setValue("panelAppearance", "dark"));
        }
        QSettings storage(path, QSettings::IniFormat);
        Settings restored(storage);
        QCOMPARE(restored.values().value("candidateFontSize").toInt(), 24);
        QCOMPARE(restored.values().value("showsZhuyinHint").toBool(), false);
        QCOMPARE(restored.values().value("panelAppearance").toString(), QString("dark"));
    }
    void corruptValuesUseDefaults() {
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        storage.setValue("candidatesPerPage", -99);
        storage.setValue("panelAppearance", "invalid");
        Settings settings(storage);
        QCOMPARE(settings.values().value("candidatesPerPage").toInt(), 9);
        QCOMPARE(settings.values().value("panelAppearance").toString(), QString("system"));
    }
    void restoreOnlyKnownPreferences() {
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        storage.setValue("unrelated", "keep");
        Settings settings(storage);
        QVERIFY(settings.setValue("candidateFontSize", 27));
        QSignalSpy changes(&settings, &Settings::changed);
        QVERIFY(settings.restoreDefaults());
        QCOMPARE(changes.size(), 1);
        QCOMPARE(settings.values().value("candidateFontSize").toInt(), 18);
        QCOMPARE(storage.value("unrelated").toString(), QString("keep"));
    }
    void iniScalarsAndFilterIntents() {
        QTemporaryDir directory;
        QSettings storage(directory.filePath("settings.ini"), QSettings::IniFormat);
        storage.setValue("candidateFontSize", QString("21"));
        storage.setValue("showsZhuyinHint", QString("false"));
        Settings settings(storage);
        QCOMPARE(settings.values().value("candidateFontSize").toInt(), 21);
        QCOMPARE(settings.values().value("showsZhuyinHint").toBool(), false);
        Candidates candidates;
        QSignalSpy filters(&candidates, &Candidates::filterRequested);
        candidates.toggleSingle();
        candidates.toggleCorrections();
        QCOMPARE(filters.size(), 2);
        QCOMPARE(filters.last(), QList<QVariant>({true, true}));
    }
    void pageBoundariesAndSelection() {
        Candidates candidates;
        QVariantList entries;
        for (int i = 0; i < 10; ++i) entries.append(QVariantMap{{"text", QString::number(i)}});
        candidates.present(entries, "reading", 8);
        QSignalSpy picks(&candidates, &Candidates::picked);
        candidates.turnPage(1);
        QCOMPARE(candidates.selected(), 0);
        candidates.pick(1);
        QCOMPARE(picks.size(), 0);
        candidates.pick(0);
        QCOMPARE(picks.first().first().toInt(), 9);
        candidates.setPageSize(3);
        QCOMPARE(candidates.page(), 3);
        candidates.turnPage(std::numeric_limits<int>::max());
        QCOMPARE(candidates.page(), 3);
        candidates.turnPage(-100);
        QCOMPARE(candidates.page(), 0);
        candidates.present({}, "");
        QCOMPARE(candidates.selected(), -1);
        candidates.pick(0);
        QCOMPARE(picks.size(), 1);
        candidates.setPageSize(0);
        QCOMPARE(candidates.pageSize(), 3);
    }
    void navigationDoesNotCommit() {
        Candidates candidates;
        QVariantList entries;
        for (int i = 0; i < 10; ++i) entries.append(QVariantMap{{"text", QString::number(i)}});
        candidates.present(entries, "", 8);
        QSignalSpy picks(&candidates, &Candidates::picked);
        candidates.moveSelection(1);
        QCOMPARE(candidates.page(), 1);
        QCOMPARE(candidates.selected(), 0);
        candidates.moveSelection(std::numeric_limits<int>::max());
        QCOMPARE(candidates.selected(), 0);
        candidates.moveSelection(-1);
        QCOMPARE(candidates.page(), 0);
        QCOMPARE(candidates.selected(), 8);
        QCOMPARE(picks.size(), 0);
        candidates.present({}, "");
        candidates.moveSelection(-1);
        QCOMPARE(candidates.selected(), -1);
    }
};
QTEST_GUILESS_MAIN(StateTest)
#include "StateTest.moc"
