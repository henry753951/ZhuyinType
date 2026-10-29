#include "LegalDocuments.h"
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QUrl>
#include <QtGlobal>

namespace {
constexpr qint64 MaxText = 8 * 1024 * 1024;

bool listsPlatform(const QJsonObject &entry, const QString &platform) {
    if (!entry.contains("platforms")) return true;
    const auto platforms = entry.value("platforms").toArray();
    for (const auto &value : platforms) {
        const auto name = value.toString();
        if (name == platform || (platform.isEmpty() && name != QLatin1String("macos"))) return true;
    }
    return false;
}

QString kind(const QString &name) {
    if (name.endsWith(".md", Qt::CaseInsensitive)) return QStringLiteral("markdown");
    if (name.endsWith(".txt", Qt::CaseInsensitive) || !name.contains('.') || name.endsWith("LICENSE")) return QStringLiteral("text");
    return QStringLiteral("folder");
}

bool safe(const QString &name) {
    return !name.isEmpty() && !name.startsWith('/') && !name.contains('\\') && !name.contains(':')
        && !name.split('/').contains(QStringLiteral(".."));
}
}

QString LegalDocuments::currentPlatform() {
#if defined(Q_OS_WIN)
    return QStringLiteral("windows");
#elif defined(Q_OS_LINUX)
    return QStringLiteral("linux");
#else
    return {};
#endif
}

LegalDocuments::LegalDocuments(QString licenseDirectory, QString dataDirectory, QString platform, QObject *parent)
    : QObject(parent), licenseDirectory_(std::move(licenseDirectory)), dataDirectory_(std::move(dataDirectory)) {
    QFile file(QDir(licenseDirectory_).filePath("sources.json"));
    QJsonParseError error{};
    const auto document = file.open(QIODevice::ReadOnly) ? QJsonDocument::fromJson(file.readAll(), &error) : QJsonDocument();
    const auto root = document.object();
    const auto groups = root.value("groups").toArray();
    const auto entries = root.value("entries").toArray();
    QSet<QString> groupIds, entryIds;
    bool valid = !document.isNull() && error.error == QJsonParseError::NoError && root.value("version").toInt() == 1;
    for (const auto &group : groups) {
        const auto id = group.toObject().value("id").toString();
        valid = valid && !id.isEmpty() && !groupIds.contains(id);
        groupIds.insert(id);
    }
    for (const auto &value : entries) {
        const auto entry = value.toObject();
        const auto id = entry.value("id").toString();
        valid = valid && !id.isEmpty() && !entryIds.contains(id) && groupIds.contains(entry.value("group").toString())
            && QStringList{"bundled", "training", "evaluation"}.contains(entry.value("distribution").toString());
        entryIds.insert(id);
    }
    if (!valid) {
        sources_ = {{"groups", QVariantList()}, {"error", QStringLiteral("無法讀取授權與資料來源")}};
        return;
    }
    QVariantList shownGroups;
    for (const auto &groupValue : groups) {
        const auto group = groupValue.toObject();
        QVariantList shown;
        for (const auto &value : entries) {
            const auto entry = value.toObject();
            if (entry.value("group") != group.value("id") || !listsPlatform(entry, platform)) continue;
            QVariantMap item = entry.toVariantMap();
            item.remove("platforms");
            item.remove("licenseFiles");
            item.remove("notices");
            if (entry.value("id") == QLatin1String("qt")) item.insert("version", QString::fromLatin1(qVersion()));
            QVariantList files;
            for (const auto &license : entry.value("licenseFiles").toArray()) {
                const auto name = license.toString();
                if (!safe(name) || !QFileInfo::exists(QDir(licenseDirectory_).filePath(name))) continue;
                licenses_.insert(name);
                files.append(QVariantMap{{"name", name}, {"kind", kind(name)}, {"notice", false}});
            }
            for (const auto &notice : entry.value("notices").toArray()) {
                const auto name = notice.toString();
                // Model attributions ship with the model's data; a model this install lacks has none to show.
                if (!safe(name) || dataDirectory_.isEmpty() || !QFileInfo(QDir(dataDirectory_).filePath(name)).isFile()) continue;
                notices_.insert(name);
                files.append(QVariantMap{{"name", name}, {"kind", kind(name)}, {"notice", true}});
            }
            item.insert("files", files);
            shown.append(item);
        }
        if (!shown.isEmpty())
            shownGroups.append(QVariantMap{{"id", group.value("id").toString()}, {"title", group.value("title").toString()}, {"entries", shown}});
    }
    sources_ = {{"groups", shownGroups}, {"error", QString()}};
}

QString LegalDocuments::path(const QString &name) const {
    if (licenses_.contains(name)) return QDir(licenseDirectory_).filePath(name);
    if (notices_.contains(name)) return QDir(dataDirectory_).filePath(name);
    return {};
}

QString LegalDocuments::text(const QString &name) const {
    const auto location = path(name);
    if (location.isEmpty() || kind(name) == QLatin1String("folder")) return {};
    QFile file(location);
    if (!file.open(QIODevice::ReadOnly) || file.size() > MaxText) return {};
    return QString::fromUtf8(file.readAll());
}

bool LegalDocuments::reveal(const QString &name) const {
    if (!notices_.contains(name)) return false;
    return QDesktopServices::openUrl(QUrl::fromLocalFile(QFileInfo(QDir(dataDirectory_).filePath(name)).absolutePath()));
}
