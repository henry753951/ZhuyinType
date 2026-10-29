#pragma once
#include <QMap>
#include <QObject>
#include <QSettings>
#include <QStringList>
#include <QVariantList>

// 在候選字視窗顯示組字的 App: the list, and the apps to offer in its ＋ menu. An app is named by its executable
// (notepad.exe on Windows, firefox on Linux), the name the input method frameworks report.
class PlatformSettings final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QStringList apps READ apps NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
public:
    explicit PlatformSettings(QSettings &storage, QObject *parent = nullptr);
    QStringList apps() const { return apps_; }
    QString error() const { return error_; }
    Q_INVOKABLE bool addApp(const QString &id);
    Q_INVOKABLE bool removeApp(const QString &id);
    // Apps with a window open now, not yet in the list: [{id, name}], sorted by name. Empty where it cannot be known.
    Q_INVOKABLE QVariantList runningApps() const;
    // An app's display name: the desktop entry's Name on Linux, the executable without .exe on Windows.
    Q_INVOKABLE QString appName(const QString &id) const;
    // Desktop entries under `directories` (each an applications folder): executable → localized Name, skipping
    // hidden entries. Exposed for tests.
    static QMap<QString, QString> desktopEntries(const QStringList &directories);
signals:
    void changed();
private:
    bool saveApps(const QStringList &apps);
    bool fail(const QString &message);
    QSettings &storage_;
    QString error_;
    QStringList apps_;
};
