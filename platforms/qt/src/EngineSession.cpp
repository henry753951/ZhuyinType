#include "EngineSession.h"
#include "../../../engine/crates/zhuyin-ffi/include/zhuyin.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QLibrary>
#include <QSaveFile>
#include <QThread>
#include <QDebug>
#include <cmath>
#include <limits>
#include <optional>

#define ZT_FUNCTIONS(X) \
    X(zt_engine_open) X(zt_engine_free) X(zt_engine_config_default) \
    X(zt_composer_options_default) X(zt_composer_new) X(zt_composer_free) \
    X(zt_composer_set_options) X(zt_composer_handle) X(zt_composer_select_candidate) \
    X(zt_composer_reset) X(zt_composer_commit_all) X(zt_composer_presentation) X(zt_presentation_free) X(zt_string_free) \
    X(zt_composer_set_preceding_text) X(zt_composer_set_learning_suspended) \
    X(zt_engine_set_fuzzy_sounds) X(zt_engine_set_reading_channel_enabled) X(zt_engine_reading_channel_active) \
    X(zt_engine_flush_personal_data) X(zt_engine_user_phrases) X(zt_engine_learned_entries) \
    X(zt_engine_excluded_entries) X(zt_phrase_list_free) X(zt_engine_add_user_phrase) \
    X(zt_engine_remove_user_phrase) X(zt_engine_forget_learned) X(zt_engine_exclude_from_learning) \
    X(zt_engine_include_in_learning) X(zt_engine_import_personal_data) X(zt_engine_export_personal_data) \
    X(zt_engine_readings) X(zt_string_list_free) X(zt_engine_forget_all_learned)

namespace {
bool validString(const QString &value) {
    return !value.contains(QChar::Null) && value.isValidUtf16();
}

bool validateSettings(const QVariantMap &values, QString &error) {
    const QMap<QString, QStringList> choices{
        {"spaceAction", {"commit", "insert", "candidates"}}, {"escape", {"composition", "reading"}},
        {"typoTolerance", {"off", "standard", "aggressive"}}, {"candidateAnchor", {"after", "before"}},
        {"candidateLayout", {"vertical", "horizontal"}}, {"characterForms", {"common", "standard"}},
        {"punctuationKeys", {"windows", "mac"}}, {"selectionKeys", {"123456789", "asdfghjkl"}}
    };
    const QStringList toggles{"learningEnabled", "pageAtListEdge", "readingChannel", "usesPrecedingText",
        "fuzzy.zZh", "fuzzy.cCh", "fuzzy.sSh", "fuzzy.enEng", "fuzzy.anAng"};
    for (auto it = values.cbegin(); it != values.cend(); ++it) {
        bool valid = true;
        if (choices.contains(it.key()))
            valid = it.value().metaType().id() == QMetaType::QString && choices.value(it.key()).contains(it.value().toString());
        else if (toggles.contains(it.key()) || it.key().startsWith("feature."))
            valid = it.value().metaType().id() == QMetaType::Bool;
        else if (it.key() == "candidatesPerPage") {
            const int type = it.value().metaType().id();
            const double number = it.value().toDouble();
            valid = (type == QMetaType::Int || type == QMetaType::LongLong || type == QMetaType::Double)
                && std::isfinite(number) && std::floor(number) == number && number >= 3 && number <= 9;
        }
        if (!valid) {
            error = QStringLiteral("設定值無效：%1").arg(it.key());
            return false;
        }
    }
    return true;
}

std::optional<ZtKey> translatedKey(int code, int modifiers) {
    // Qt swaps Control and Meta on macOS; native Control arrives as MetaModifier.
#ifdef Q_OS_MACOS
    constexpr int controlModifier = Qt::MetaModifier;
    constexpr int commandModifier = Qt::ControlModifier;
#else
    constexpr int controlModifier = Qt::ControlModifier;
    constexpr int commandModifier = Qt::MetaModifier;
#endif
    if (modifiers & commandModifier) return std::nullopt;
    const int mods = modifiers & ~int(Qt::KeypadModifier);
    const bool shift = mods == Qt::ShiftModifier;
    const bool alt = mods == Qt::AltModifier;
    const bool control = mods == controlModifier;
    if (mods != 0 && !shift && !alt && !control) return std::nullopt;
    const auto key = [](uint32_t kind, uint32_t c = 0) { return ZtKey{kind, c}; };
    switch (code) {
    case Qt::Key_Space: if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_SPACE : ZT_KEY_SPACE); break;
    case Qt::Key_Return: case Qt::Key_Enter: if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_ENTER : ZT_KEY_ENTER); break;
    case Qt::Key_Backspace: if (mods == 0 || alt) return key(alt ? ZT_KEY_OPTION_BACKSPACE : ZT_KEY_BACKSPACE); break;
    case Qt::Key_Delete: if (mods == 0) return key(ZT_KEY_FORWARD_DELETE); break;
    case Qt::Key_Escape: if (mods == 0) return key(ZT_KEY_ESCAPE); break;
    case Qt::Key_Tab: case Qt::Key_Backtab:
        if (mods == 0 || shift) return key(shift || code == Qt::Key_Backtab ? ZT_KEY_SHIFT_TAB : ZT_KEY_TAB);
        break;
    case Qt::Key_Left: if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_LEFT : ZT_KEY_LEFT); break;
    case Qt::Key_Right: if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_RIGHT : ZT_KEY_RIGHT); break;
    case Qt::Key_Up:
        if (alt) return key(ZT_KEY_PAGE_UP);
        if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_UP : ZT_KEY_UP);
        break;
    case Qt::Key_Down:
        if (alt) return key(ZT_KEY_PAGE_DOWN);
        if (mods == 0 || shift) return key(shift ? ZT_KEY_SHIFT_DOWN : ZT_KEY_DOWN);
        break;
    case Qt::Key_Home: if (mods == 0) return key(ZT_KEY_HOME); break;
    case Qt::Key_End: if (mods == 0) return key(ZT_KEY_END); break;
    case Qt::Key_PageUp: if (mods == 0) return key(ZT_KEY_PAGE_UP); break;
    case Qt::Key_PageDown: if (mods == 0) return key(ZT_KEY_PAGE_DOWN); break;
    default:
        if (code < 0x21 || code > 0x7e) return std::nullopt;
        if (code >= 'A' && code <= 'Z') code += 'a' - 'A';
        if (shift) {
            const QByteArray shifted("~!@#$%^&*()_+{}|:\"<>?");
            const QByteArray plain("`1234567890-=[]\\;',./");
            const auto index = shifted.indexOf(char(code));
            if (index >= 0) code = plain[index];
        }
        if ((alt || control) && code >= 'a' && code <= 'z') return std::nullopt;
        return key(shift ? ZT_KEY_SHIFTED_CHAR : alt ? ZT_KEY_PRINTED : control ? ZT_KEY_CONTROL_CHAR : ZT_KEY_CHAR,
                   static_cast<uint32_t>(code));
    }
    return std::nullopt;
}

QString featureFor(ZtKey key) {
    switch (key.kind) {
    case ZT_KEY_OPTION_BACKSPACE: return "feature.deleteWord";
    case ZT_KEY_SHIFT_UP: return "feature.correctionsList";
    case ZT_KEY_SHIFT_DOWN: return "feature.singleCharacterList";
    case ZT_KEY_SHIFT_LEFT: case ZT_KEY_SHIFT_RIGHT: return "feature.selectPhrase";
    case ZT_KEY_TAB: case ZT_KEY_SHIFT_TAB: case ZT_KEY_PAGE_UP: case ZT_KEY_PAGE_DOWN: return "feature.pageKeys";
    case ZT_KEY_CONTROL_CHAR: return "feature.controlPunctuation";
    case ZT_KEY_PRINTED: return "feature.optionSymbols";
    case ZT_KEY_CHAR: if (key.character == '`') return "feature.typeZhuyin"; break;
    default: break;
    }
    return {};
}
}

struct EngineSession::State {
    QLibrary library;
    ZtEngine *engine = nullptr;
    ZtComposer *composer = nullptr;
    bool learningSuspended = false;
#define DECLARE(name) decltype(&::name) name = nullptr;
    ZT_FUNCTIONS(DECLARE)
#undef DECLARE
    ~State() {
        if (composer) zt_composer_free(composer);
        if (engine) zt_engine_free(engine);
    }
    bool load(const QString &path, QString &error) {
        library.setFileName(path);
        // Rust may still have a delayed personal-data writer running after a handle is released.
        library.setLoadHints(QLibrary::PreventUnloadHint);
        if (!library.load()) { error = library.errorString(); return false; }
#define RESOLVE(name) name = reinterpret_cast<decltype(name)>(library.resolve(#name)); \
        if (!name) { error = QStringLiteral("引擎缺少必要介面：") + QStringLiteral(#name); return false; }
        ZT_FUNCTIONS(RESOLVE)
#undef RESOLVE
        return true;
    }
    QString take(char *value) const {
        if (!value) return {};
        const QString text = QString::fromUtf8(value);
        zt_string_free(value);
        return text;
    }
    bool configure(const QVariantMap &values) {
        auto options = zt_composer_options_default();
        const QByteArray keys = values.value("selectionKeys", "123456789").toString()
            .left(values.value("candidatesPerPage", 9).toInt()).toUtf8();
        options.selection_keys = keys.constData();
        const QString space = values.value("spaceAction", "commit").toString();
        options.space_action = space == "candidates" ? ZT_SPACE_OPEN_CANDIDATES : space == "insert" ? ZT_SPACE_INSERT_SPACE : ZT_SPACE_COMMIT_AND_SPACE;
        options.escape_clears_composition = values.value("escape", "composition") == "composition";
        options.learning_enabled = values.value("learningEnabled", true).toBool();
        const QString typo = values.value("typoTolerance", "standard").toString();
        options.typo_tolerance = typo == "off" ? ZT_TYPO_OFF : typo == "aggressive" ? ZT_TYPO_AGGRESSIVE : ZT_TYPO_STANDARD;
        options.candidate_anchor = values.value("candidateAnchor", "after") == "before" ? ZT_ANCHOR_BEFORE_CURSOR : ZT_ANCHOR_AFTER_CURSOR;
        options.vertical_list = values.value("candidateLayout", "vertical") == "vertical";
        options.page_at_list_edge = values.value("pageAtListEdge", true).toBool();
        options.character_forms = values.value("characterForms", "common") == "standard" ? ZT_FORMS_STANDARD : ZT_FORMS_COMMON;
        options.punctuation_keys = values.value("punctuationKeys", "windows") == "mac" ? ZT_PUNCTUATION_MAC : ZT_PUNCTUATION_WINDOWS;
        if (!zt_composer_set_options(composer, &options)) return false;
        uint32_t fuzzy = 0;
        const QStringList pairs{"zZh", "cCh", "sSh", "enEng", "anAng"};
        for (int i = 0; i < pairs.size(); ++i)
            if (values.value("fuzzy." + pairs[i], false).toBool()) fuzzy |= 1u << i;
        zt_engine_set_fuzzy_sounds(engine, fuzzy);
        zt_engine_set_reading_channel_enabled(engine, values.value("readingChannel", true).toBool());
        if (!values.value("usesPrecedingText", true).toBool()) zt_composer_set_preceding_text(composer, "");
        return true;
    }
    QVariantList phrases(ZtPhraseList *raw) const {
        const std::unique_ptr<ZtPhraseList, decltype(zt_phrase_list_free)> list(raw, zt_phrase_list_free);
        QVariantList result;
        if (!list) return result;
        for (size_t i = 0; i < list->count; ++i) {
            const auto &item = list->items[i];
            result.append(QVariantMap{{"word", QString::fromUtf8(item.word)},
                {"reading", QString::fromUtf8(item.reading)}, {"count", item.count}});
        }
        return result;
    }
};
#undef ZT_FUNCTIONS

EngineSession::EngineSession(QObject *parent) : QObject(parent) {}
EngineSession::~EngineSession() {
    if (state_ && !state_->zt_engine_flush_personal_data(state_->engine))
        qWarning("EngineSession: personal data could not be flushed before closing");
}
bool EngineSession::ready() const { return state_ && state_->composer; }
int EngineSession::pageCount() const { return rows_.isEmpty() ? 0 : int((rows_.size() + pageSize_ - 1) / pageSize_); }
bool EngineSession::fail(const QString &message) { error_ = message; emit errorChanged(); return false; }
void EngineSession::clearError() { if (!error_.isEmpty()) { error_.clear(); emit errorChanged(); } }
bool EngineSession::requireReady() {
    Q_ASSERT(QThread::currentThread() == thread());
    return ready() || fail(QStringLiteral("引擎尚未開啟。"));
}

bool EngineSession::open(const QString &libraryPath, const QString &lexiconPath, const QString &userDataDirectory) {
    Q_ASSERT(QThread::currentThread() == thread());
    if (!validString(libraryPath) || !validString(lexiconPath) || !validString(userDataDirectory)
        || libraryPath.isEmpty() || lexiconPath.isEmpty() || userDataDirectory.isEmpty())
        return fail(QStringLiteral("請提供有效的引擎、詞庫與個人資料路徑。"));
    if (!QFileInfo(libraryPath).isAbsolute() || !QFileInfo(lexiconPath).isAbsolute() || !QFileInfo(userDataDirectory).isAbsolute())
        return fail(QStringLiteral("引擎與資料路徑須為絕對路徑。"));
    auto next = std::make_unique<State>();
    QString message;
    if (!next->load(libraryPath, message)) return fail(message);
    if (!QDir().mkpath(userDataDirectory)) return fail(QStringLiteral("無法建立個人資料目錄。"));
    auto config = next->zt_engine_config_default();
    const QDir assets = QFileInfo(lexiconPath).dir();
    const QByteArray channel = assets.filePath("channel.bin").toUtf8();
    const QByteArray chat = assets.filePath("lm_chat").toUtf8();
    if (QFileInfo::exists(QString::fromUtf8(channel))) config.reading_channel = channel.constData();
    if (QFileInfo::exists(assets.filePath("lm_chat/lm.bin")) || QFileInfo::exists(assets.filePath("lm_chat/bigram.tsv")))
        config.chat_model_directory = chat.constData();
    char *rawError = nullptr;
    const QByteArray lexicon = lexiconPath.toUtf8(), user = userDataDirectory.toUtf8();
    if (state_ && !flush()) return false;
    QElapsedTimer loading; loading.start();
    next->engine = next->zt_engine_open(lexicon.constData(), user.constData(), &config, &rawError);
    const double lexiconMs = double(loading.nsecsElapsed()) / 1e6;
    message = next->take(rawError);
    if (!next->engine) return fail(message.isEmpty() ? QStringLiteral("無法開啟詞庫。") : message);
    next->composer = next->zt_composer_new(next->engine, nullptr);
    if (!next->composer || !next->configure(settings_)) return fail(QStringLiteral("無法建立組字狀態。"));
    state_ = std::move(next);
    // The Qt client loads the lexicon (with the chat model and reading channel) only; no reranker or character model.
    loadTimes_ = {{"lexicon", lexiconMs}, {"reranker", QVariant()}, {"character", QVariant()}};
    committed_.clear();
    clearError();
    return refresh() && refreshPersonal();
}

bool EngineSession::refresh() {
    const auto free = state_->zt_presentation_free;
    const std::unique_ptr<ZtPresentation, decltype(free)> p(state_->zt_composer_presentation(state_->composer), free);
    if (!p) return fail(QStringLiteral("無法取得組字狀態。"));
    composition_ = QString::fromUtf8(p->text);
    reading_ = p->zhuyin_hint ? QString::fromUtf8(p->zhuyin_hint) : QString();
    notice_ = p->notice ? QString::fromUtf8(p->notice) : QString();
    cursor_ = int(p->cursor);
    focusedClause_ = int(p->focused_clause);
    targetClause_ = int(p->target_clause);
    rows_.clear(); clauses_.clear();
    for (size_t i = 0; i < p->clause_count; ++i)
        clauses_.append(QVariantMap{{"start", qulonglong(p->clauses[i].start)}, {"end", qulonglong(p->clauses[i].end)}});
    selected_ = -1;
    single_ = false; corrections_ = false; tolerant_ = -1;
    if (const auto *list = p->candidates) {
        pageSize_ = qMax(1, int(list->page_size));
        selected_ = list->count ? int(list->selection) : -1;
        single_ = list->single_character;
        corrections_ = list->tolerant == 1;
        tolerant_ = list->tolerant;
        for (size_t i = 0; i < list->count; ++i)
            rows_.append(QVariantMap{{"text", QString::fromUtf8(list->items[i].text)},
                {"annotation", list->items[i].annotation ? QString::fromUtf8(list->items[i].annotation) : QString()}});
    } else pageSize_ = settings_.value("candidatesPerPage", 9).toInt();
    readingChannelActive_ = state_->zt_engine_reading_channel_active(state_->engine);
    emit changed();
    return true;
}

bool EngineSession::refreshPersonal() {
    userPhrases_ = state_->phrases(state_->zt_engine_user_phrases(state_->engine));
    learnedEntries_ = state_->phrases(state_->zt_engine_learned_entries(state_->engine));
    excludedEntries_ = state_->phrases(state_->zt_engine_excluded_entries(state_->engine));
    emit personalDataChanged();
    return true;
}

bool EngineSession::finishKey(bool handled, const QString &commit) {
    committed_ = commit;
    const bool refreshed = refresh();
    if (!commit.isEmpty()) {
        if (refreshed) finishPersonal();
        emit committedText(commit);
    }
    return handled;
}

bool EngineSession::key(int qtKey, int modifiers) {
    if (!requireReady()) return false;
    clearError();
    const auto input = translatedKey(qtKey, modifiers);
    if (!input || !settings_.value(featureFor(*input), true).toBool()) {
        committed_.clear(); emit changed(); return false;
    }
    QElapsedTimer timer; timer.start();
    const auto result = state_->zt_composer_handle(state_->composer, *input);
    const QString commit = state_->take(result.commit);
    const bool handled = finishKey(result.handled, commit);
    if (handled || !commit.isEmpty()) emit keyHandled(double(timer.nsecsElapsed()) / 1e6);
    // Return may add a selected phrase without committing text.
    if (handled && qtKey == Qt::Key_Return && commit.isEmpty()) finishPersonal();
    return handled;
}

bool EngineSession::choose(int absoluteIndex) {
    if (!requireReady()) return false;
    if (absoluteIndex < 0 || absoluteIndex >= rows_.size()) return fail(QStringLiteral("候選字索引超出範圍。"));
    clearError();
    QElapsedTimer timer; timer.start();
    const auto result = state_->zt_composer_select_candidate(state_->composer, size_t(absoluteIndex));
    const QString commit = state_->take(result.commit);
    const bool handled = finishKey(result.handled, commit);
    if (handled && commit.isEmpty()) finishPersonal();
    if (handled) emit keyHandled(double(timer.nsecsElapsed()) / 1e6);
    return handled;
}

bool EngineSession::reset() {
    if (!requireReady()) return false;
    clearError();
    state_->zt_composer_reset(state_->composer);
    committed_.clear();
    return refresh();
}

bool EngineSession::commitAll() {
    if (!requireReady()) return false;
    clearError();
    QElapsedTimer timer; timer.start();
    const QString commit = state_->take(state_->zt_composer_commit_all(state_->composer));
    finishKey(true, commit);
    if (!commit.isEmpty()) emit keyHandled(double(timer.nsecsElapsed()) / 1e6);
    return error_.isEmpty();
}

bool EngineSession::applySettings(const QVariantMap &settings) {
    Q_ASSERT(QThread::currentThread() == thread());
    QVariantMap next = settings_;
    for (auto it = settings.cbegin(); it != settings.cend(); ++it) next.insert(it.key(), it.value());
    QString message;
    if (!validateSettings(next, message)) return fail(message);
    if (ready() && !state_->configure(next)) return fail(QStringLiteral("引擎拒絕設定值。"));
    settings_ = next;
    clearError();
    return !ready() || refresh();
}

bool EngineSession::setPrecedingText(const QString &text) {
    if (!requireReady()) return false;
    if (!validString(text)) return fail(QStringLiteral("前文不是有效文字。"));
    const auto utf8 = !state_->learningSuspended && settings_.value("usesPrecedingText", true).toBool() ? text.toUtf8() : QByteArray();
    state_->zt_composer_set_preceding_text(state_->composer, utf8.constData());
    clearError();
    return refresh();
}

bool EngineSession::setLearningSuspended(bool suspended) {
    if (!requireReady()) return false;
    state_->learningSuspended = suspended;
    state_->zt_composer_set_learning_suspended(state_->composer, suspended);
    if (suspended) state_->zt_composer_set_preceding_text(state_->composer, "");
    clearError();
    return refresh();
}

bool EngineSession::flush() {
    if (!requireReady()) return false;
    if (!state_->zt_engine_flush_personal_data(state_->engine))
        return fail(QStringLiteral("個人資料尚未寫入磁碟；請確認目錄權限後重試。"));
    clearError();
    return true;
}

bool EngineSession::finishPersonal() {
    refreshPersonal();
    return flush();
}

bool EngineSession::validatePhrase(const QString &word, const QString &reading) {
    return (validString(word) && validString(reading) && !word.trimmed().isEmpty() && !reading.trimmed().isEmpty())
        || fail(QStringLiteral("請提供有效的字詞與注音。"));
}

bool EngineSession::addPhrase(const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(word, reading)) return false;
    char *rawError = nullptr;
    const QByteArray w = word.toUtf8(), r = reading.toUtf8();
    const bool ok = state_->zt_engine_add_user_phrase(state_->engine, w.constData(), r.constData(), &rawError);
    const QString message = state_->take(rawError);
    if (!ok) return fail(message.isEmpty() ? QStringLiteral("無法加入自訂詞。") : message);
    return finishPersonal();
}

bool EngineSession::removePhrase(const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(word, reading)) return false;
    const auto w = word.toUtf8(), r = reading.toUtf8();
    state_->zt_engine_remove_user_phrase(state_->engine, w.constData(), r.constData());
    return finishPersonal();
}
bool EngineSession::editPhrase(const QString &oldWord, const QString &oldReading, const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(oldWord, oldReading) || !validatePhrase(word, reading)) return false;
    char *rawError = nullptr;
    const QByteArray w = word.toUtf8(), r = reading.toUtf8();
    const bool ok = state_->zt_engine_add_user_phrase(state_->engine, w.constData(), r.constData(), &rawError);
    const QString message = state_->take(rawError);
    if (!ok) return fail(message.isEmpty() ? QStringLiteral("無法加入自訂詞。") : message);
    if (oldWord != word || oldReading != reading) {
        const auto ow = oldWord.toUtf8(), orr = oldReading.toUtf8();
        state_->zt_engine_remove_user_phrase(state_->engine, ow.constData(), orr.constData());
    }
    return finishPersonal();
}

QStringList EngineSession::readings(const QString &word) const {
    QStringList result;
    if (!ready() || !validString(word) || word.trimmed().isEmpty()) return result;
    const QByteArray w = word.trimmed().toUtf8();
    const std::unique_ptr<ZtStringList, decltype(state_->zt_string_list_free)> list(
        state_->zt_engine_readings(state_->engine, w.constData()), state_->zt_string_list_free);
    if (!list) return result;
    for (size_t i = 0; i < list->count; ++i) result.append(QString::fromUtf8(list->items[i]));
    return result;
}

bool EngineSession::forgetAllLearned() {
    if (!requireReady()) return false;
    state_->zt_engine_forget_all_learned(state_->engine);
    return finishPersonal();
}

bool EngineSession::forgetLearned(const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(word, reading)) return false;
    const auto w = word.toUtf8(), r = reading.toUtf8();
    state_->zt_engine_forget_learned(state_->engine, w.constData(), r.constData());
    return finishPersonal();
}
bool EngineSession::excludeLearned(const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(word, reading)) return false;
    const auto w = word.toUtf8(), r = reading.toUtf8();
    state_->zt_engine_exclude_from_learning(state_->engine, w.constData(), r.constData());
    return finishPersonal();
}
bool EngineSession::includeLearned(const QString &word, const QString &reading) {
    if (!requireReady() || !validatePhrase(word, reading)) return false;
    const auto w = word.toUtf8(), r = reading.toUtf8();
    state_->zt_engine_include_in_learning(state_->engine, w.constData(), r.constData());
    return finishPersonal();
}

qint64 EngineSession::importPersonal(const QUrl &fileUrl) {
    if (!requireReady()) return -1;
    if (!fileUrl.isLocalFile() || !validString(fileUrl.toLocalFile())) { fail(QStringLiteral("請選擇本機備份檔。")); return -1; }
    QFile file(fileUrl.toLocalFile());
    if (!file.open(QIODevice::ReadOnly)) { fail(file.errorString()); return -1; }
    const QByteArray data = file.readAll();
    if (file.error() != QFileDevice::NoError) { fail(file.errorString()); return -1; }
    char *rawError = nullptr;
    const auto count = state_->zt_engine_import_personal_data(state_->engine,
        reinterpret_cast<const uint8_t *>(data.constData()), size_t(data.size()), &rawError);
    const QString message = state_->take(rawError);
    if (count < 0) { fail(message.isEmpty() ? QStringLiteral("備份格式無效。") : message); return -1; }
    return finishPersonal() ? count : -1;
}

bool EngineSession::exportPersonal(const QUrl &fileUrl) {
    if (!requireReady()) return false;
    if (!fileUrl.isLocalFile() || !validString(fileUrl.toLocalFile())) return fail(QStringLiteral("請選擇本機備份位置。"));
    if (!flush()) return false;
    char *raw = state_->zt_engine_export_personal_data(state_->engine);
    if (!raw) return fail(QStringLiteral("無法匯出個人資料。"));
    const QByteArray data = state_->take(raw).toUtf8();
    QSaveFile file(fileUrl.toLocalFile());
    if (!file.open(QIODevice::WriteOnly) || file.write(data) != data.size() || !file.commit())
        return fail(file.errorString());
    clearError();
    return true;
}
