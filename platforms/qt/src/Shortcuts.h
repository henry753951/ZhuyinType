#pragma once

#include <QObject>
#include <QSettings>
#include <QMap>
#include <QVariantMap>

// The rebindable commands of the 快捷鍵 page, as macOS Shared/Shortcuts.swift and Preferences.bindings: each command
// has up to five keys (a key is one Qt key with modifiers); only the commands the user changed are stored, the others
// follow their defaults minus any default the user gave to another command (`withDefaults`).  Callers apply the
// composition-context gates; the feature switches come from Settings through `setFeatures`.
//
// Platform words: ⌃ is Ctrl, ⌥ is Alt, ⌘ is Win (Windows) or Super (Linux) and stays reserved; fn ↑ ↓ ← → ⌫ are
// PgUp, PgDn, Home, End and Delete.  On Windows and Linux Ctrl with a letter is the apps' (Ctrl+C, Ctrl+V …).
class Shortcuts final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantMap values READ values NOTIFY changed)
    Q_PROPERTY(QVariantList commands READ commands CONSTANT)
    Q_PROPERTY(QVariantList conflicts READ conflicts NOTIFY changed)
    Q_PROPERTY(bool customized READ anyCustomized NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(int maximumBindings READ maximumBindings CONSTANT)
public:
    explicit Shortcuts(QSettings &storage, QObject *parent = nullptr);
    QVariantMap values() const;
    QVariantList commands() const;
    QVariantList conflicts() const;
    bool anyCustomized() const;
    QString error() const { return error_; }
    int maximumBindings() const { return 5; }
    /// Sets binding `index` of `command` (-1 or past the end adds one), unless the key is reserved or another binding
    /// has it; false with `error` saying why.
    Q_INVOKABLE bool record(const QString &command, int key, int modifiers, int index = -1);
    /// Removes one key, default or not; a required command keeps its last one.
    Q_INVOKABLE bool remove(const QString &command, int index);
    /// Restores the command's defaults that no other command uses now; fails only when every one is taken.  With no
    /// command, every command's (全部還原).
    Q_INVOKABLE bool reset(const QString &command = QString());
    Q_INVOKABLE QString resolve(int key, int modifiers) const;
    Q_INVOKABLE bool isCustomized(const QString &command) const;
    /// Bindings of `command` another command that is on also uses: {index: other command id}.
    Q_INVOKABLE QVariantMap conflictsOf(const QString &command) const;
    /// The command's first key in keycap form ("⇧↓"), or "" when it has none or its feature is off.
    Q_INVOKABLE QString activeLabel(const QString &command) const;
    /// The modifiers held so far, as the recorder shows them ("⌃⌥", "Ctrl+Alt+").
    Q_INVOKABLE QString modifierLabel(int modifiers) const;
    Q_INVOKABLE bool isEnabled(const QString &command) const;
    /// The feature switches (`feature.*`) from Settings: commands whose feature is off have no conflicts, and ⌃ with
    /// a symbol key is reserved while 中文標點 is on.
    void setFeatures(const QVariantMap &settings);
signals:
    void changed();
    void errorChanged();
private:
    struct Binding {
        int key;
        int modifiers;
        bool operator==(const Binding &) const = default;
    };
    struct Command {
        QString id, label, title, page, feature;
        bool required;
        QList<Binding> defaults;
    };
    using State = QMap<QString, QList<Binding>>;
    static const QList<Command> &registry();
    static const Command *definition(const QString &id);
    static bool normalize(Binding &binding);
    static QStringList keycaps(const Binding &binding);
    static QString compact(const Binding &binding);
    static QString spoken(const Binding &binding);
    QString reserved(const Binding &binding, const QString &command) const;
    QString problem(const Binding &binding, const QString &command, int index, const State &all) const;
    static State withDefaults(const State &user);
    State bindings() const { return withDefaults(user_); }
    bool store(const State &next);
    bool fail(const QString &message);
    QVariantMap describe(const Binding &binding) const;
    QSettings &storage_;
    State user_;
    QVariantMap features_;
    QString error_;
};
