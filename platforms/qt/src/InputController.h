#pragma once
#include <QObject>
#include <QElapsedTimer>
#include <QHash>
#include <QSet>
#include <functional>
#include <limits>
#include <memory>
#include <optional>
#include "CorrectionLog.h"
#include "EngineSession.h"
#include "Settings.h"
#include "Shortcuts.h"

// Hosts own focus, secure-input detection and insertion of committedText/insertTextRequested.
// Like macOS ControllerCore, it also keeps the text before the insertion point (the app's, or what was sent in the
// same app within 10 minutes) for the engine, and reports every key, pick and commit to the observers: the usage
// events (`usageEvent`, see IPC.md §使用統計) and the correction log.
class InputController final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool english READ english NOTIFY modeChanged)
    // 前文 (macOS ControllerCore): characters read before the insertion point, kept, and the same-app window in seconds.
    Q_PROPERTY(int contextRead MEMBER contextReadValue CONSTANT)
    Q_PROPERTY(int contextKept MEMBER contextKeptValue CONSTANT)
    Q_PROPERTY(int contextWindow MEMBER contextWindowValue CONSTANT)
public:
    enum ShiftSide { UnknownShift = 0, LeftShift = 1, RightShift = 2 };
    Q_ENUM(ShiftSide)
    InputController(EngineSession &engine, Settings &settings, Shortcuts &shortcuts, QObject *parent = nullptr);
    bool english() const;
    // Qt key codes do not identify the physical Shift side. Native hosts may supply it;
    // with UnknownShift, only the "both" preference permits a tap to toggle modes.
    Q_INVOKABLE bool press(int key, int modifiers, bool repeat = false, int shiftSide = UnknownShift);
    Q_INVOKABLE bool choose(int absoluteIndex);
    Q_INVOKABLE void release(int key, int modifiers, int shiftSide = UnknownShift);
    Q_INVOKABLE void setSecure(bool secure);
    Q_INVOKABLE void deactivate();
    // The candidate window's own controls, observed like keys.
    Q_INVOKABLE bool feed(int key, int modifiers = 0);
    Q_INVOKABLE void toggleCorrections();
    /// The focused field: its app (for the 10-minute carry-over) and, when the platform reports it, the text before
    /// the insertion point (nullopt: unknown).  Read when a composition starts.
    void setClient(const QString &app, std::optional<QString> before);
    void synchronizeCapsLock(bool enabled);
    void setCorrectionLog(CorrectionLog *log, const QString &appVersion);
    /// Monotonic seconds, for the key times the observers use; tests replace it.
    std::function<double()> clock;
    /// Wall-clock milliseconds, for the 10-minute context window; tests replace it.
    std::function<qint64()> now;
    /// What the engine reads as the text before the composition (for the tests).
    QString recentText() const { return recentText_; }
    static constexpr qint64 contextWindowMs = 600'000;
    static constexpr int contextRead = 128;
    static constexpr int contextKept = 255;
    int contextReadValue = contextRead, contextKeptValue = contextKept, contextWindowValue = int(contextWindowMs / 1000);
    /// The gap that ends a burst of typing (macOS: four times the usual key interval, at least 2 s); the usage store
    /// may set it from its rhythm.
    double pauseThreshold = 2.0;
    static constexpr double awayAfter = 60;
signals:
    void modeChanged();
    /// The user switched 中/英 (Shift tap or Caps Lock): the mode toast shows `english`.
    void modeSwitched(bool english);
    void candidateChosen(double elapsedMs);
    void candidateBoundaryReached(int direction);
    void insertTextRequested(const QString &text);
    /// One macOS UsageStats event; the kinds and fields are listed in IPC.md §使用統計.
    void usageEvent(const QString &kind, const QVariantMap &data);
private:
    struct ShiftTap { int side; QElapsedTimer timer; };
    struct UsageState {
        double lastCommit = -1e18;
        bool deletedSinceCommit = false;
        std::optional<QString> literalBefore;
        bool corrected = false;
        std::optional<double> lastKey;
        int burst = 0, cleanRun = 0;
    };
    EngineSession &engine_;
    Settings &settings_;
    Shortcuts &shortcuts_;
    const QVariantList commands_;
    bool english_ = false, capsLock_ = false, secure_ = false;
    QSet<int> heldShifts_;
    std::optional<ShiftTap> shiftTap_;
    QString pendingCommit_;
    QString recentText_;
    qint64 recentTime_ = std::numeric_limits<qint64>::min() / 2;
    QString app_;
    std::optional<QString> clientBefore_;
    QHash<QString, QPair<QString, qint64>> appContext_;
    UsageState usage_;
    CorrectionLog *log_ = nullptr;
    std::unique_ptr<CorrectionRecorder> recorder_;
    bool pressImpl(int key, int modifiers, bool repeat, int shiftSide, double time);
    bool passThrough();
    bool dispatch(const QString &command, int originalKey, int originalModifiers, double time);
    bool engineKey(int key, int modifiers, double time);
    bool isRemovedDefault(int key, int modifiers) const;
    void shiftPressed(int modifiers, bool repeat, int side);
    void clearShift();
    void discardSecureComposition();
    void prepareContext();
    void remember();
    void observe(const ObservedInput &input, const Presentation &before, const QString &commit, bool handled,
                 std::optional<double> latencyMs);
    void observeUsage(const ObservedInput &input, const Presentation &before, const Presentation &after,
                      const QString &commit, bool handled, std::optional<double> latencyMs);
    void syncRecorder();
};
