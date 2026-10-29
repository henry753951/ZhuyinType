#pragma once
#include <QObject>
#include <QSet>
#include <QVariantMap>

// 授權與資料來源: resources/sources.json filtered to this platform, and the texts it names, read inside the app.
// Licence files come from `licenseDirectory` (the build bundles them under :/legal/Licenses); notices named by an
// entry (model attributions) are looked up under `dataDirectory`. Only files the filtered list names can be read.
class LegalDocuments final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantMap sources READ sources CONSTANT)
public:
    LegalDocuments(QString licenseDirectory, QString dataDirectory, QString platform, QObject *parent = nullptr);
    // {groups: [{id, title, entries: [{id, name, use, license, distribution, copyright, url, version, changes,
    //  files: [{name, kind: "text"|"markdown"|"folder", notice}]}]}]}; empty `groups` and an `error` when unreadable.
    QVariantMap sources() const { return sources_; }
    // The text of a file the list names, or an empty string.
    Q_INVOKABLE QString text(const QString &name) const;
    // Shows a notice's folder in the file manager (archives and data lists are not text to read here).
    Q_INVOKABLE bool reveal(const QString &name) const;
    // "windows" or "linux"; the Qt preview on other systems counts as either.
    static QString currentPlatform();
private:
    QString path(const QString &name) const;
    QString licenseDirectory_, dataDirectory_;
    QVariantMap sources_;
    QSet<QString> licenses_, notices_;
};
