#pragma once

#include <QDateTime>
#include <QElapsedTimer>
#include <QTimer>
#include <QMap>
#include <QNetworkAccessManager>
#include <QObject>
#include <QVariantMap>
#include <array>
#include <functional>

// 使用統計 (the macOS UsageStatsStore and UsageStatsObserver, IMK/UsageStats.swift) and the update check.
//
// Counts only: how many characters, compositions, keys, picks and corrections, per-key latency as a histogram, the
// typing rhythm (key intervals, thinking pauses, the longest runs, characters by weekday and hour), by local day for
// 60 days and in total.  Never the text, the readings, which keys or the App.  Kept in memory and written to
// stats.json (the macOS schema) at most every 10 minutes, when the settings window asks and before the process exits.
//
// The input path reports what happened through `recordUsageEvent(kind, data)` (InputController::usageEvent; IPC.md
// §usage).  Kinds:
//   key        {latencyMs: number, composing: bool}  every key the IME saw; `composing` when it was handled while
//                                                    composing (counted as a key, its latency kept).  The time
//                                                    between keys gives the rhythm unless `pause` events arrive.
//   commit     {characters: int, compositionEnded: bool (default true), clean: bool (optional),
//               correctedShown: bool, correctedCommitted: bool}          text sent to the App
//   pick       {list: "normal" | "corrections" | "single" | "punctuation"}
//   backspace, escape, deleteAfterCommit, literalZhuyin, undoLiteral, phraseAdded,
//   correctionShown, correctionCommitted                                  {} one each
//   pause      {ms: number}  a thinking pause measured by the caller (then key gaps are not measured here)
//   interval   {ms: number}  a key interval inside a burst measured by the caller
//   reset      {}            a password field or a switched-away input: forget the current burst
// Previews and tests never call it with real input.
class DesktopServices final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool usageEnabled READ usageEnabled NOTIFY usageChanged)
    Q_PROPERTY(QString period READ period NOTIFY usageChanged)
    Q_PROPERTY(QVariantMap usage READ usage NOTIFY usageChanged)
    Q_PROPERTY(QString usageError READ usageError NOTIFY usageChanged)
    Q_PROPERTY(int keptDays READ keptDays CONSTANT)
    Q_PROPERTY(bool checkingUpdate READ checkingUpdate NOTIFY updateChanged)
    Q_PROPERTY(QVariantMap update READ update NOTIFY updateChanged)
    Q_PROPERTY(QString updateError READ updateError NOTIFY updateChanged)
public:
    /// `statsPath` empty: in memory only.
    explicit DesktopServices(QString statsPath, QString currentVersion, QObject *parent = nullptr,
                             QNetworkAccessManager *network = nullptr,
                             std::function<QDateTime()> now = [] { return QDateTime::currentDateTime(); });
    ~DesktopServices() override;

    static constexpr int KeptDays = 60;
    static constexpr qint64 SaveIntervalMs = 10 * 60 * 1000;

    bool usageEnabled() const { return enabled_; }
    QString period() const { return period_; }
    QVariantMap usage() const;
    QString usageError() const { return usageError_; }
    int keptDays() const { return KeptDays; }
    bool checkingUpdate() const { return checking_; }
    QVariantMap update() const { return update_; }
    QString updateError() const { return updateError_; }

    Q_INVOKABLE bool setUsageEnabled(bool enabled);
    Q_INVOKABLE bool setPeriod(const QString &period);
    Q_INVOKABLE bool clearUsage();
    /// Writes stats.json now if anything changed (the settings window opening, the process exiting).
    Q_INVOKABLE bool saveIfNeeded();
    /// CPU, memory and (where the system reports it) energy of this process, sampled now.
    Q_INVOKABLE QVariantMap processMetrics() const;
    Q_INVOKABLE void checkForUpdates();
    Q_INVOKABLE bool openReleasePage();
    /// Writes since the start, for tests.
    int writes() const { return writes_; }
    /// Replaces the clock between keys (tests).
    void setUptime(std::function<qint64()> uptime) { uptime_ = std::move(uptime); }

public slots:
    void recordUsageEvent(const QString &kind, const QVariantMap &data);

signals:
    void usageChanged();
    void updateChanged();

public:
    struct Counters {
        qint64 characters = 0, compositions = 0, cleanCompositions = 0, keys = 0;
        qint64 picksNormal = 0, picksCorrections = 0, picksSingle = 0, picksPunctuation = 0;
        qint64 backspaces = 0, escapes = 0, deleteAfterCommit = 0, literalZhuyin = 0, undoLiteral = 0;
        qint64 correctedShown = 0, correctedCommitted = 0, phrasesAdded = 0;
        qint64 typingMilliseconds = 0, thinkingMilliseconds = 0;
        Counters &operator+=(const Counters &other);
        bool isZero() const;
        qint64 picks() const { return picksNormal + picksCorrections + picksSingle + picksPunctuation; }
        qint64 corrections() const { return picks() + backspaces + escapes + deleteAfterCommit + literalZhuyin; }
    };
    struct Buckets {
        QList<double> bounds;
        QList<qint64> counts;
        double maximum = 0;
        explicit Buckets(QList<double> upper = {});
        qint64 total() const;
        void add(double value);
        /// Interpolated inside its bucket; negative when empty.
        double percentile(double fraction) const;
    };

private:
    struct Rhythm {
        Buckets intervals;
        Buckets pauses;
        qint64 longestRun = 0, longestCleanRun = 0;
        std::array<qint64, 7 * 24> hours{};
        Rhythm();
        double pauseThreshold() const;
    };
    void reset();
    bool load();
    bool write();
    void changed();
    void add(const Counters &delta);
    void noteKey(const QVariantMap &data);
    QDate today() const { return now_().date(); }

    QString path_;
    QString currentVersion_;
    QNetworkAccessManager *network_;
    std::function<QDateTime()> now_;
    std::function<qint64()> uptime_;
    QElapsedTimer clock_;
    QTimer notify_;
    bool enabled_ = false;
    bool readable_ = true;
    QString period_ = QStringLiteral("month");
    QDateTime since_;
    Counters totals_;
    QMap<QDate, Counters> days_;
    Buckets latency_;
    Rhythm rhythm_;
    QString usageError_;
    bool dirty_ = false;
    qint64 lastSave_ = -1;
    int writes_ = 0;
    // The observer: one input session's burst and corrections.
    double sessionThreshold_ = 2;
    qint64 lastKey_ = -1;
    qint64 burst_ = 0, cleanRun_ = 0;
    bool corrected_ = false, sawCorrected_ = false, committedCorrected_ = false;
    bool externalRhythm_ = false;

    bool checking_ = false;
    QVariantMap update_;
    QString updateError_;
    bool parseRelease(const QByteArray &body);
};
