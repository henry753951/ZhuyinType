#include "Practice.h"

#include <QDir>

Practice::Practice(QSettings &source, QString libraryPath, QString lexiconPath, QObject *parent)
    : QObject(parent), source_(source), libraryPath_(std::move(libraryPath)), lexiconPath_(std::move(lexiconPath)) {}

Practice::~Practice() { stop(); }

void Practice::copySource() {
    storage_->clear();
    for (const auto &key : source_.allKeys())
        if (!key.startsWith(QLatin1String("wizard/"))) storage_->setValue(key, source_.value(key));
    storage_->sync();
}

bool Practice::start(bool withEngine) {
    stop();
    directory_ = std::make_unique<QTemporaryDir>();
    if (!directory_->isValid()) { directory_.reset(); return false; }
    storage_ = std::make_unique<QSettings>(directory_->filePath("practice.ini"), QSettings::IniFormat);
    copySource();
    settings_ = std::make_unique<Settings>(*storage_);
    shortcuts_ = std::make_unique<Shortcuts>(*storage_);
    session_ = std::make_unique<EngineSession>();
    input_ = std::make_unique<InputController>(*session_, *settings_, *shortcuts_);
    session_->applySettings(settings_->values());
    connect(settings_.get(), &Settings::changed, session_.get(), [this] { session_->applySettings(settings_->values()); });
    if (withEngine && !libraryPath_.isEmpty())
        session_->open(libraryPath_, lexiconPath_, QDir(directory_->path()).filePath("personal"));
    emit changed();
    return true;
}

bool Practice::restart() {
    if (!active()) return start();
    QVariantMap values;
    QSettings fresh(directory_->filePath("fresh.ini"), QSettings::IniFormat);
    fresh.clear();
    for (const auto &key : source_.allKeys()) fresh.setValue(key, source_.value(key));
    fresh.sync();
    // The source's values through a Settings of its own, so defaults fill what the user never changed.
    Settings original(fresh);
    return settings_->assign(original.values());
}

void Practice::stop() {
    if (!active()) return;
    input_.reset();
    session_.reset();
    shortcuts_.reset();
    settings_.reset();
    storage_.reset();
    directory_.reset();
    emit changed();
}
