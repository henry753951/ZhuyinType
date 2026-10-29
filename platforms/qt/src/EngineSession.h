#pragma once

#include <QObject>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <memory>

// Use on the QObject's owning thread. rows/selected use absolute candidate indices;
// cursor and clause ranges are UTF-16 offsets. No system input method is installed here.
class EngineSession final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool ready READ ready NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QString composition READ composition NOTIFY changed)
    Q_PROPERTY(QString reading READ reading NOTIFY changed)
    Q_PROPERTY(QVariantList rows READ rows NOTIFY changed)
    Q_PROPERTY(int selected READ selected NOTIFY changed)
    Q_PROPERTY(int cursor READ cursor NOTIFY changed)
    Q_PROPERTY(QVariantList clauses READ clauses NOTIFY changed)
    Q_PROPERTY(int focusedClause READ focusedClause NOTIFY changed)
    Q_PROPERTY(int targetClause READ targetClause NOTIFY changed)
    Q_PROPERTY(QString notice READ notice NOTIFY changed)
    Q_PROPERTY(QString committed READ committed NOTIFY changed)
    Q_PROPERTY(bool composing READ composing NOTIFY changed)
    Q_PROPERTY(int pageSize READ pageSize NOTIFY changed)
    Q_PROPERTY(int page READ page NOTIFY changed)
    Q_PROPERTY(int pageCount READ pageCount NOTIFY changed)
    Q_PROPERTY(bool single READ single NOTIFY changed)
    Q_PROPERTY(bool corrections READ corrections NOTIFY changed)
    Q_PROPERTY(bool readingChannelActive READ readingChannelActive NOTIFY changed)
    Q_PROPERTY(QVariantList userPhrases READ userPhrases NOTIFY personalDataChanged)
    Q_PROPERTY(QVariantList learnedEntries READ learnedEntries NOTIFY personalDataChanged)
    Q_PROPERTY(QVariantList excludedEntries READ excludedEntries NOTIFY personalDataChanged)
    // How long the last open took, in ms: {lexicon, reranker, character}; null for a model this client does not load.
    Q_PROPERTY(QVariantMap loadTimes READ loadTimes NOTIFY changed)
public:
    explicit EngineSession(QObject *parent = nullptr);
    ~EngineSession() override;
    bool ready() const;
    QString error() const { return error_; }
    QString composition() const { return composition_; }
    QString reading() const { return reading_; }
    QVariantList rows() const { return rows_; }
    int selected() const { return selected_; }
    int cursor() const { return cursor_; }
    QVariantList clauses() const { return clauses_; }
    int focusedClause() const { return focusedClause_; }
    int targetClause() const { return targetClause_; }
    QString notice() const { return notice_; }
    QString committed() const { return committed_; }
    bool composing() const { return !composition_.isEmpty(); }
    int pageSize() const { return pageSize_; }
    int page() const { return selected_ < 0 ? 0 : selected_ / pageSize_; }
    int pageCount() const;
    bool single() const { return single_; }
    bool corrections() const { return corrections_; }
    // The list's corrections mode: 1 on, 0 off, -1 the list cannot have corrections (or no list).
    int tolerant() const { return tolerant_; }
    bool readingChannelActive() const { return readingChannelActive_; }
    QVariantList userPhrases() const { return userPhrases_; }
    QVariantList learnedEntries() const { return learnedEntries_; }
    QVariantList excludedEntries() const { return excludedEntries_; }
    QVariantMap loadTimes() const { return loadTimes_; }

    // Requires the current public ABI (not the v0.4.4 void-returning flush ABI).
    Q_INVOKABLE bool open(const QString &libraryPath, const QString &lexiconPath, const QString &userDataDirectory);
    // False means pass the key through, even if committedText was emitted first.
    Q_INVOKABLE bool key(int qtKey, int modifiers = 0);
    Q_INVOKABLE bool choose(int absoluteIndex);
    Q_INVOKABLE bool reset();
    Q_INVOKABLE bool commitAll();
    // Partial updates merge with the last valid options; unsupported UI-only keys are ignored.
    Q_INVOKABLE bool applySettings(const QVariantMap &settings);
    Q_INVOKABLE bool setPrecedingText(const QString &text);
    Q_INVOKABLE bool setLearningSuspended(bool suspended);
    Q_INVOKABLE bool flush();
    Q_INVOKABLE bool addPhrase(const QString &word, const QString &reading);
    Q_INVOKABLE bool removePhrase(const QString &word, const QString &reading);
    // Replaces a phrase: adds the new one, then removes the old one when it differs (macOS's edit).
    Q_INVOKABLE bool editPhrase(const QString &oldWord, const QString &oldReading, const QString &word, const QString &reading);
    // The readings of `word` the lexicon knows (for the add-phrase sheet); empty when not ready.
    Q_INVOKABLE QStringList readings(const QString &word) const;
    // Forgets everything learned; user phrases and excluded words stay.
    Q_INVOKABLE bool forgetAllLearned();
    Q_INVOKABLE bool forgetLearned(const QString &word, const QString &reading);
    Q_INVOKABLE bool excludeLearned(const QString &word, const QString &reading);
    Q_INVOKABLE bool includeLearned(const QString &word, const QString &reading);
    Q_INVOKABLE qint64 importPersonal(const QUrl &fileUrl);
    Q_INVOKABLE bool exportPersonal(const QUrl &fileUrl);
signals:
    void changed();
    void errorChanged();
    void personalDataChanged();
    void committedText(const QString &text);
    // Includes presentation/persistence work; also emitted for picks and commits that pass the key through.
    void keyHandled(double elapsedMs);
private:
    struct State;
    std::unique_ptr<State> state_;
    QVariantMap settings_;
    QString error_, composition_, reading_, notice_, committed_;
    QVariantList rows_, clauses_, userPhrases_, learnedEntries_, excludedEntries_;
    QVariantMap loadTimes_{{"lexicon", QVariant()}, {"reranker", QVariant()}, {"character", QVariant()}};
    int selected_ = -1, cursor_ = 0, focusedClause_ = -1, targetClause_ = -1, pageSize_ = 9, tolerant_ = -1;
    bool single_ = false, corrections_ = false, readingChannelActive_ = false;
    bool fail(const QString &message);
    bool requireReady();
    void clearError();
    bool refresh();
    bool refreshPersonal();
    bool finishPersonal();
    bool validatePhrase(const QString &word, const QString &reading);
    bool finishKey(bool handled, const QString &commit);
};
