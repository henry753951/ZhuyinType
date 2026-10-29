#pragma once

#include "EngineSession.h"
#include "InputController.h"
#include "Settings.h"
#include "Shortcuts.h"

#include <QObject>
#include <QSettings>
#include <QTemporaryDir>
#include <memory>

// The setup wizard's sandbox (macOS WizardModel.sandbox + PracticeField): a copy of the user's settings and keys in a
// temporary store that the wizard's choices change, and an engine and input controller of its own on that copy, with
// empty temporary user data, for the practice field.  Nothing reaches the user's settings, keys or words until the
// wizard applies them; stop() throws everything away.
class Practice final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool active READ active NOTIFY changed)
    Q_PROPERTY(QObject *settings READ settings NOTIFY changed)
    Q_PROPERTY(QObject *shortcuts READ shortcuts NOTIFY changed)
    Q_PROPERTY(QObject *engineSession READ engineSession NOTIFY changed)
    Q_PROPERTY(QObject *inputController READ inputController NOTIFY changed)
public:
    /// `source`: the user's settings store, copied on start.  Empty `libraryPath`: no practice engine (renders, tests).
    Practice(QSettings &source, QString libraryPath, QString lexiconPath, QObject *parent = nullptr);
    ~Practice() override;
    bool active() const { return settings_ != nullptr; }
    QObject *settings() const { return settings_.get(); }
    QObject *shortcuts() const { return shortcuts_.get(); }
    QObject *engineSession() const { return session_.get(); }
    QObject *inputController() const { return input_.get(); }
    /// A fresh copy of the user's settings and keys (and, with `withEngine`, a practice engine).
    Q_INVOKABLE bool start(bool withEngine = true);
    /// Starts the copy over from the user's settings, keeping the engine (a preset chosen again).
    Q_INVOKABLE bool restart();
    Q_INVOKABLE void stop();
signals:
    void changed();
private:
    void copySource();
    QSettings &source_;
    QString libraryPath_, lexiconPath_;
    std::unique_ptr<QTemporaryDir> directory_;
    std::unique_ptr<QSettings> storage_;
    std::unique_ptr<Settings> settings_;
    std::unique_ptr<Shortcuts> shortcuts_;
    std::unique_ptr<EngineSession> session_;
    std::unique_ptr<InputController> input_;
};
