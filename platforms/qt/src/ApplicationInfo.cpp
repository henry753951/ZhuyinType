#include "ApplicationInfo.h"
#include "LegalDocuments.h"
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>

static QString textResource(const QString &path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) return {};
    return QString::fromUtf8(file.readAll());
}

// Where the lexicon and models are, as main.cpp finds them (model attributions are read from there).
static QString dataDirectory() {
    QString data = qEnvironmentVariable("ZHUYIN_DATA");
    if (data.isEmpty()) data = QDir(QCoreApplication::applicationDirPath()).filePath("data");
#ifdef ZT_DEVELOPMENT_DATA
    if (!QFileInfo::exists(QDir(data).filePath("lexicon.tsv"))) data = QStringLiteral(ZT_DEVELOPMENT_DATA);
#endif
    return data;
}

QVariantMap applicationInfo() {
    auto info = QJsonDocument::fromJson(textResource(":/metadata/metadata.json").toUtf8()).object().toVariantMap();
    info.insert("licenseText", textResource(":/legal/Licenses/AGPL-3.0.txt"));
    auto *documents = new LegalDocuments(QStringLiteral(":/legal/Licenses"), dataDirectory(), LegalDocuments::currentPlatform(),
                                         QCoreApplication::instance());
    info.insert("documents", QVariant::fromValue<QObject *>(documents));
    return info;
}
