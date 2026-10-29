#pragma once

#include "KeyInput.h"
#include "Presentation.h"
#include <QJsonObject>
#include <QObject>
#include "Settings.h"
#include <QUrl>
#include <functional>
#include <memory>
#include <variant>

// The correction log (「記錄改字」, docs/product/DESIGN.md §2.7), ported from macOS IMK/CorrectionRecorder.swift:
// every composition in which the user corrected the engine becomes one JSON line of corrections.jsonl, in the same
// schema as the Mac (snake_case keys, sorted), so `zt eval corrections` reads both.

/// What InputController reports to the observers.
struct ObservedInput {
    enum Kind { Key, Click, ToggleCorrections, Commit };
    Kind kind = Key;
    KeyInput key;
    /// Seconds on the monotonic clock when the key went down.
    double time = 0;
    int index = -1;
};

/// Engine settings that change what the keys produce, recorded with each composition.
QJsonObject correctionSettings(const QVariantMap &values);

class CorrectionRecorder {
public:
    using Sink = std::function<void(const QJsonObject &)>;
    explicit CorrectionRecorder(Sink sink) : sink_(std::move(sink)) {}
    /// Seconds after a commit in which deleting it and typing again is a correction.
    static constexpr double window = 30;
    bool recordsContext = false;
    QJsonObject settings;
    QString appVersion = QStringLiteral("dev");
    QString selectionKeys = QStringLiteral("123456789");
    std::function<double()> clock;
    std::function<QString()> today;

    void breakContext();
    void observe(const ObservedInput &input, const Presentation &before, const Presentation &after, const QString &commit,
                 bool handled, bool secure, const std::function<QString()> &context);

private:
    struct Shape {
        int converted = 0;
        QString pending;
        bool operator==(const Shape &) const = default;
        Shape() = default;
        explicit Shape(const QString &text);
    };
    struct Key { QString label; double time = 0; };
    struct Retyping { int event; int target; int position; QList<Key> keys; };
    struct Session {
        double start = 0;
        std::optional<QString> context;
        QString keys;
        std::optional<double> lastKey;
        QList<Key> typing, original;
        QList<QPair<int, Shape>> history{{0, Shape(QString())}};
        bool tracking = true, replayable = true, originalReplayable = true;
        QList<QJsonObject> events;
        QStringList corrections;
        std::optional<QString> shown;
        std::optional<Retyping> retyping;
        bool selecting = false;
        std::optional<int> literal;
        std::optional<QPair<QString, QString>> afterCommit;
        QString sent;
    };
    struct LastCommit {
        double time; QString text, committed; int passed; int deleted = 0; QString typing; bool replayable;
        std::optional<QString> context;
    };
    Sink sink_;
    std::shared_ptr<Session> session_;
    std::optional<QPair<std::shared_ptr<Session>, double>> suspended_;
    std::optional<LastCommit> lastCommit_;

    void listKey(Session &s, const KeyInput &key, const CandidateListState &list, const Presentation &before,
                 const Presentation &after, int t);
    void pick(Session &s, int index, const QString &via, const Presentation &before, const Presentation &after, int t);
    void composingKey(Session &s, const KeyInput &key, const QString &label, const Presentation &before,
                      const Presentation &after, bool committed, int t);
    void typed(Session &s, const Key &key, const Presentation &after, bool mid, int t);
    void appendTyping(Session &s, const Key &key, const Presentation &after);
    void deletion(Session &s, const KeyInput &key, const QString &label, const Presentation &before,
                  const Presentation &after, bool mid, int t);
    void endRetyping(Session &s, const Presentation &after);
    void correction(Session &s, const QString &kind, const QString &shown);
    void carry(Session &s, const QString &committed, const Presentation &after, int t);
    void finish(Session &s, const QString &final, const ObservedInput &input, bool handled, double time, int t);
    void followDeletion(double time);
    QString keyLabel(const ObservedInput &input, const std::optional<CandidateListState> &list) const;
    bool isListKey(const KeyInput &key) const;
    static QString serialize(const QList<Key> &keys);
    static QString kind(const CandidateListState &list);
    static std::optional<QString> focused(const Presentation &p);
public:
    /// Where two texts differ: the start (in Unicode scalars), the part of `a` replaced and the part of `b` in its place.
    static std::tuple<int, QString, QString> difference(const QString &a, const QString &b);
};

/// corrections.jsonl: appended, counted, exported and cleared for the 進階 page (`correctionLog` in QML).
class CorrectionLog final : public QObject {
    Q_OBJECT
    Q_PROPERTY(int count READ count NOTIFY changed)
    Q_PROPERTY(QUrl location READ location CONSTANT)
    // 一併記錄游標前的文字: Settings' `recordsCorrectionContext` (the one copy), exposed here for convenience.
    Q_PROPERTY(bool recordsContext READ recordsContext WRITE setRecordsContext NOTIFY recordsContextChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
public:
    static constexpr auto fileName = "corrections.jsonl";
    CorrectionLog(Settings &settings, const QString &directory, QObject *parent = nullptr);
    int count() const { return count_; }
    QUrl location() const;
    QString path() const { return path_; }
    bool recordsContext() const;
    void setRecordsContext(bool value);
    QString error() const { return error_; }
    bool append(const QJsonObject &record);
    /// Opens the folder with the file selected where the platform allows it.
    Q_INVOKABLE bool showLog();
    Q_INVOKABLE bool exportLog(const QUrl &destination);
    Q_INVOKABLE bool clearLog();
    static QByteArray encode(const QJsonObject &record);
signals:
    void changed();
    void recordsContextChanged();
    void errorChanged();
private:
    Settings &settings_;
    QString path_, error_;
    int count_ = 0;
    void recount();
    bool fail(const QString &message);
};
