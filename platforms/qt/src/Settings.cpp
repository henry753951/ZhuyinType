#include "Settings.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <algorithm>
#include <cmath>

Settings::Settings(QSettings &storage, QObject *parent) : QObject(parent), storage_(storage) {
    Q_INIT_RESOURCE(settings);
    QFile file(":/settings.json");
    if (!file.open(QIODevice::ReadOnly))
        qFatal("Cannot open the settings schema");
    QJsonParseError error;
    const auto document = QJsonDocument::fromJson(file.readAll(), &error);
    if (error.error != QJsonParseError::NoError || !document.isArray())
        qFatal("Invalid settings schema");
    definitions_ = document.array().toVariantList();
    for (const auto &entry : definitions_) {
        const auto definition = entry.toMap();
        const auto key = definition.value("key").toString();
        if (key.isEmpty() || values_.contains(key) || !valid(definition, definition.value("default")))
            qFatal("Invalid setting definition");
        auto value = storage_.value(key, definition.value("default"));
        // INI backends restore scalars as strings after a process restart.
        if (value.metaType().id() == QMetaType::QString) {
            const auto kind = definition.value("kind").toString();
            if (kind == "toggle" && (value == "true" || value == "false"))
                value = value == "true";
            else if (kind == "number") {
                bool ok = false;
                const auto number = value.toString().toDouble(&ok);
                if (ok) value = number;
            }
        }
        values_.insert(key, valid(definition, value) ? value : definition.value("default"));
    }
}

bool Settings::valid(const QVariantMap &definition, const QVariant &value) const {
    const auto kind = definition.value("kind").toString();
    if (kind == "toggle")
        return value.metaType().id() == QMetaType::Bool;
    if (kind == "number") {
        const int type = value.metaType().id();
        if (type != QMetaType::Int && type != QMetaType::LongLong && type != QMetaType::Double)
            return false;
        const double number = value.toDouble();
        return std::isfinite(number) && std::floor(number) == number
            && number >= definition.value("min").toDouble() && number <= definition.value("max").toDouble();
    }
    if (kind == "choice" && value.metaType().id() == QMetaType::QString) {
        for (const auto &option : definition.value("options").toList())
            if (option.toMap().value("value") == value)
                return true;
    }
    return false;
}

bool Settings::setValue(const QString &key, const QVariant &value) {
    for (const auto &entry : definitions_) {
        const auto definition = entry.toMap();
        if (definition.value("key").toString() != key)
            continue;
        if (!valid(definition, value))
            return false;
        if (values_.value(key) == value)
            return true;
        return save({{key, value}});
    }
    return false;
}

bool Settings::assign(const QVariantMap &values) {
    QVariantMap updates;
    for (auto item = values.cbegin(); item != values.cend(); ++item) {
        const auto found = std::find_if(definitions_.cbegin(), definitions_.cend(),
                                        [&](const QVariant &entry) { return entry.toMap().value("key") == item.key(); });
        if (found == definitions_.cend()) continue;
        auto value = item.value();
        // QML numbers arrive as doubles; whole numbers are stored as they were read.
        if (found->toMap().value("kind") == "number" && value.metaType().id() == QMetaType::Double) value = value.toDouble();
        if (!valid(found->toMap(), value)) {
            error_ = QStringLiteral("「%1」的值無效。").arg(found->toMap().value("title").toString());
            emit errorChanged();
            return false;
        }
        if (values_.value(item.key()) != value) updates.insert(item.key(), value);
    }
    return updates.isEmpty() || save(updates);
}

QString Settings::wizardSeenVersion() const { return storage_.value("wizard/seenVersion").toString(); }

void Settings::markWizardSeen(const QString &version) {
    if (wizardSeenVersion() == version) return;
    storage_.setValue("wizard/seenVersion", version);
    storage_.sync();
    emit wizardChanged();
}

bool Settings::restoreDefaults() {
    QVariantMap defaults;
    for (const auto &entry : definitions_) {
        const auto definition = entry.toMap();
        defaults.insert(definition.value("key").toString(), definition.value("default"));
    }
    return save(defaults);
}

bool Settings::save(const QVariantMap &updates) {
    for (auto item = updates.cbegin(); item != updates.cend(); ++item)
        storage_.setValue(item.key(), item.value());
    storage_.sync();
    if (storage_.status() != QSettings::NoError) {
        for (auto item = updates.cbegin(); item != updates.cend(); ++item)
            storage_.setValue(item.key(), values_.value(item.key()));
        error_ = QStringLiteral("無法儲存設定，請檢查設定檔的寫入權限。");
        emit errorChanged();
        return false;
    }
    for (auto item = updates.cbegin(); item != updates.cend(); ++item)
        values_[item.key()] = item.value();
    error_.clear();
    emit errorChanged();
    emit changed();
    return true;
}
