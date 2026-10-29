#pragma once

#include <QObject>
#include <QSettings>
#include <QVariantMap>

// QSettings is injected so tests and visual previews never touch personal settings.
class Settings final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantMap values READ values NOTIFY changed)
    Q_PROPERTY(QVariantList definitions READ definitions CONSTANT)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    /// The setup wizard's version the user has seen; empty before the first opening (WizardProgress on macOS).
    Q_PROPERTY(QString wizardSeenVersion READ wizardSeenVersion NOTIFY wizardChanged)
public:
    explicit Settings(QSettings &storage, QObject *parent = nullptr);
    QVariantMap values() const { return values_; }
    QVariantList definitions() const { return definitions_; }
    QString error() const { return error_; }
    Q_INVOKABLE bool restoreDefaults();
    Q_INVOKABLE bool setValue(const QString &key, const QVariant &value);
    /// Several values at once, all or none (the wizard's sandbox copy and 套用).
    Q_INVOKABLE bool assign(const QVariantMap &values);
    QString wizardSeenVersion() const;
    Q_INVOKABLE void markWizardSeen(const QString &version);
signals:
    void changed();
    void errorChanged();
    void wizardChanged();
private:
    bool save(const QVariantMap &updates);
    bool valid(const QVariantMap &definition, const QVariant &value) const;
    QSettings &storage_;
    QVariantMap values_;
    QVariantList definitions_;
    QString error_;
};
