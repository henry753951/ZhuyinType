#include <QFontDatabase>
#include <QGuiApplication>
#include <QtQuickTest/quicktest.h>

class QuickTestSetup : public QObject {
    Q_OBJECT
public slots:
    void applicationAvailable() {
        QGuiApplication::setFont(QFontDatabase::systemFont(QFontDatabase::GeneralFont));
    }
};
QUICK_TEST_MAIN_WITH_SETUP(desktop_interactions, QuickTestSetup)
#include "QuickTest.moc"
