#include "CorrectionLog.h"

#include <QDate>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QProcess>
#include <QSaveFile>
#include <QUuid>
#include <cmath>

namespace {
using K = KeyInput;

QList<uint> scalars(const QString &text) { return text.toUcs4(); }
QString fromScalars(const QList<uint> &values, qsizetype from = 0, qsizetype to = -1) {
    if (to < 0) to = values.size();
    QString result;
    for (qsizetype i = from; i < to; ++i) result.append(QString::fromUcs4(reinterpret_cast<const char32_t *>(&values[i]), 1));
    return result;
}
int scalarCount(const QString &text) { return int(text.toUcs4().size()); }
bool isZhuyin(uint scalar) {
    return (scalar >= 0x3105 && scalar <= 0x3129) || scalar == 0x02CA || scalar == 0x02C7 || scalar == 0x02CB || scalar == 0x02D9;
}
int milliseconds(double seconds) { return std::max(0, int(std::lround(seconds * 1000))); }
bool atEnd(const Presentation &p) { return p.isEmpty() || p.cursor == p.text.size(); }
int scalarOffset(const QString &text, int utf16) {
    return scalarCount(text.left(std::clamp(utf16, 0, int(text.size()))));
}
bool isContent(const KeyInput &key) {
    switch (key.kind) {
    case K::Char: case K::ShiftedChar: case K::Printed: case K::ControlChar: case K::Space: case K::ShiftSpace:
    case K::Backspace: case K::OptionBackspace: case K::ForwardDelete: case K::Escape: case K::Enter: case K::ShiftEnter:
        return true;
    default: return false;
    }
}
void setOptional(QJsonObject &object, const char *key, const std::optional<QString> &value) {
    if (value) object.insert(key, *value); else object.remove(key);
}
QJsonObject event(const QString &type, int t, const QString &shown) {
    return {{"type", type}, {"t", t}, {"shown", shown}};
}
}

QJsonObject correctionSettings(const QVariantMap &v) {
    QJsonArray fuzzy;
    for (const auto &[key, name] : {std::pair{"fuzzy.zZh", "zh"}, {"fuzzy.cCh", "ch"}, {"fuzzy.sSh", "sh"},
                                    {"fuzzy.enEng", "en"}, {"fuzzy.anAng", "an"}})
        if (v.value(key).toBool()) fuzzy.append(QString::fromLatin1(name));
    return {{"typo_tolerance", v.value("typoTolerance", "standard").toString()}, {"fuzzy_sounds", fuzzy},
            {"punctuation_keys", v.value("punctuationKeys", "windows").toString()},
            // The engine's names, as the Mac records them.
            {"space_action", v.value("spaceAction", "commit").toString() == "candidates" ? QStringLiteral("openCandidates")
                 : v.value("spaceAction", "commit").toString() == "insert" ? QStringLiteral("insertSpace")
                                                                           : QStringLiteral("commitAndSpace")},
            {"character_forms", v.value("characterForms", "common").toString()},
            {"candidate_anchor", v.value("candidateAnchor", "after").toString()},
            {"selection_keys", v.value("selectionKeys", "123456789").toString()},
            {"vertical_list", v.value("candidateLayout", "vertical").toString() != "horizontal"},
            {"learning", v.value("learningEnabled", true).toBool()},
            {"preceding_text", v.value("usesPrecedingText", true).toBool()},
            {"reading_channel", v.value("readingChannel", true).toBool()}};
}

// MARK: - Recorder

CorrectionRecorder::Shape::Shape(const QString &text) {
    const auto all = scalars(text);
    qsizetype count = 0;
    while (count < all.size() && isZhuyin(all[all.size() - 1 - count])) ++count;
    converted = int(all.size() - count);
    pending = fromScalars(all, all.size() - count);
}

void CorrectionRecorder::breakContext() {
    lastCommit_.reset();
    suspended_.reset();
}

void CorrectionRecorder::observe(const ObservedInput &input, const Presentation &before, const Presentation &after,
                                 const QString &commit, bool handled, bool secure, const std::function<QString()> &context) {
    const double time = input.kind == ObservedInput::Key && input.time > 0 ? input.time : clock();
    const bool hasCommit = !commit.isEmpty();
    if (secure) {
        session_.reset(); suspended_.reset(); lastCommit_.reset();
        return;
    }
    if (before.isEmpty() && after.isEmpty() && !hasCommit) {
        // A key for the app with nothing composed.
        if (input.kind != ObservedInput::Key || handled) return;
        if (input.key.is(K::Backspace)) followDeletion(time); else lastCommit_.reset();
        suspended_.reset();
        return;
    }
    if (!session_) {
        // A composition that started before recording was turned on is not recorded.
        if (!before.isEmpty()) return;
        if (suspended_ && time - suspended_->second < window) {
            session_ = suspended_->first;
        } else {
            auto fresh = std::make_shared<Session>();
            fresh->start = time;
            if (recordsContext) fresh->context = context();
            if (lastCommit_ && lastCommit_->deleted > lastCommit_->passed) {
                const auto committed = scalars(lastCommit_->committed);
                const int take = std::min(int(committed.size()), lastCommit_->deleted - lastCommit_->passed);
                const QString deleted = fromScalars(committed, committed.size() - take);
                auto e = event("delete_after_commit", 0, before.text);
                e.insert("deleted", deleted);
                e.insert("previous_committed", lastCommit_->committed);
                e.insert("previous_typing", lastCommit_->typing);
                e.insert("previous_replayable", lastCommit_->replayable);
                setOptional(e, "previous_context", lastCommit_->context);
                fresh->events.append(e);
                fresh->afterCommit = qMakePair(deleted, lastCommit_->committed);
            }
            session_ = fresh;
        }
        suspended_.reset();
        lastCommit_.reset();
    }
    const auto keep = session_;
    Session &s = *keep;
    const QString label = keyLabel(input, before.candidates);
    if (s.lastKey) s.keys += QStringLiteral("<dt:%1>").arg(milliseconds(time - *s.lastKey));
    s.keys += label.isEmpty() ? QStringLiteral("<?>") : label;
    s.lastKey = time;
    const int t = milliseconds(time - s.start);
    // A typing key that made the composition send its head first: the key is typing, and the record goes on.
    const bool carried = hasCommit && !after.isEmpty();
    if (carried) {
        Presentation whole = after;
        whole.text = commit + after.text;
        endRetyping(s, whole);
    }
    switch (input.kind) {
    case ObservedInput::Commit: break;
    case ObservedInput::Click: pick(s, input.index, "click", before, after, t); break;
    case ObservedInput::ToggleCorrections:
        if (after.candidates && after.candidates != before.candidates) {
            auto e = event("switch_list", t, after.text);
            e.insert("list", kind(*after.candidates));
            s.events.append(e);
        }
        break;
    case ObservedInput::Key:
        if (before.candidates && isListKey(input.key)) {
            listKey(s, input.key, *before.candidates, before, after, t);
        } else {
            if (before.candidates) s.events.append(event("close_list", t, after.text));
            composingKey(s, input.key, label, before, after, !carried && hasCommit, t);
        }
        break;
    }
    if (carried) {
        carry(s, commit, after, t);
    } else if (hasCommit) {
        finish(s, commit, input, handled, time, t);
    } else if (after.isEmpty() && session_ == keep && !s.sent.isEmpty()) {
        // Part of it was sent already: the record ends here, with what was sent.
        finish(s, QString(), input, handled, time, t);
    } else if (after.isEmpty() && session_ == keep) {
        // Emptied without a commit (Escape or Backspace): typing it again continues this record.
        s.literal.reset();
        s.selecting = false;
        session_.reset();
        suspended_ = qMakePair(keep, time);
    }
}

void CorrectionRecorder::listKey(Session &s, const KeyInput &key, const CandidateListState &list,
                                 const Presentation &before, const Presentation &after, int t) {
    endRetyping(s, after);
    s.literal.reset();
    if (key.is(K::Char, u'`')) {
        // The list's span as literal Zhuyin: depends on the list, so the typing no longer replays.
        if (!(before == after)) {
            auto e = event("literal_zhuyin", t, after.text);
            e.insert("shown_before", before.text);
            s.events.append(e);
            correction(s, "literal_zhuyin", before.text);
            s.literal = int(s.events.size()) - 1;
            s.replayable = false;
            s.originalReplayable = false;
            s.tracking = false;
        }
        return;
    }
    if (!after.candidates) {
        const int number = key.kind == K::Char ? int(selectionKeys.indexOf(QChar(key.character))) : -1;
        if (number >= 0) pick(s, list.page * list.pageSize + number, "key", before, after, t);
        else if (key.is(K::Enter)) pick(s, list.selection, "key", before, after, t);
        else s.events.append(event("close_list", t, after.text));
    } else if (kind(*after.candidates) != kind(list)) {
        auto e = event("switch_list", t, after.text);
        e.insert("list", kind(*after.candidates));
        s.events.append(e);
    }
}

void CorrectionRecorder::pick(Session &s, int index, const QString &via, const Presentation &before,
                              const Presentation &after, int t) {
    if (!before.candidates || after.candidates) return;
    endRetyping(s, after);
    s.literal.reset();
    auto e = event("pick", t, after.text);
    const auto [start, removed, inserted] = difference(before.text, after.text);
    e.insert("before", before.text);
    e.insert("after", after.text);
    e.insert("start", start);
    e.insert("removed", removed);
    e.insert("inserted", inserted);
    e.insert("rank", index + 1);
    e.insert("list", kind(*before.candidates));
    e.insert("via", via);
    s.events.append(e);
    if (before.text != after.text) correction(s, "pick", before.text);
}

void CorrectionRecorder::composingKey(Session &s, const KeyInput &key, const QString &label, const Presentation &before,
                                      const Presentation &after, bool committed, int t) {
    const auto literal = s.literal;
    s.literal.reset();
    if (s.selecting) {
        const bool lastSelect = !s.events.isEmpty() && s.events.last().value("type").toString() == "select";
        if ((key.is(K::ShiftLeft) || key.is(K::ShiftRight)) && lastSelect) {
            setOptional(s.events.last(), "selected", focused(after));
            s.events.last().insert("shown", after.text);
            return;
        }
        if (key.is(K::Escape)) {
            s.selecting = false;
            s.events.append(event("cancel_select", t, after.text));
            return;
        }
        if (key.is(K::Enter) && !committed) {
            s.selecting = false;
            auto e = event("add_phrase", t, after.text);
            if (!s.events.isEmpty() && s.events.last().contains("selected")) e.insert("phrase", s.events.last().value("selected"));
            const bool added = after.notice.startsWith(QStringLiteral("已加入"));
            e.insert("added", added);
            s.events.append(e);
            if (added) correction(s, "add_phrase", before.text);
            return;
        }
        s.selecting = false;
    }
    if (committed) {
        // The key that sent the composition is not typing; a replay ends with <enter>.
        endRetyping(s, after);
        return;
    }
    switch (key.kind) {
    case K::ShiftLeft: case K::ShiftRight: {
        endRetyping(s, after);
        s.selecting = true;
        auto e = event("select", t, after.text);
        e.insert("shown_before", before.text);
        setOptional(e, "selected", focused(after));
        s.events.append(e);
        correction(s, "select", before.text);
        return;
    }
    case K::Left: case K::Right: case K::Home: case K::End: {
        if (before.cursor == after.cursor) break;
        endRetyping(s, after);
        const int position = scalarOffset(after.text, after.cursor);
        if (!s.events.isEmpty() && s.events.last().value("type").toString() == "cursor") {
            s.events.last().insert("position", position);
            s.events.last().insert("t", t);
        } else {
            auto e = event("cursor", t, after.text);
            e.insert("position", position);
            s.events.append(e);
        }
        return;
    }
    default: break;
    }
    if (!before.candidates && after.candidates) {
        // Opening the list may first finish the unfinished syllable: keep the key, closed again, as typing.
        endRetyping(s, after);
        if (before.text != after.text && !label.isEmpty())
            appendTyping(s, Key{label + "<esc>", s.lastKey.value_or(0)}, after);
        auto e = event("open_list", t, after.text);
        e.insert("list", kind(*after.candidates));
        setOptional(e, "target", focused(after));
        s.events.append(e);
        return;
    }
    const bool mid = !atEnd(before) || !atEnd(after);
    const bool content = isContent(key);
    if (!content && before.text == after.text) return;
    if (mid) {
        s.tracking = false;
        s.replayable = false;
        s.originalReplayable = false;
        if (content) correction(s, "cursor_edit", before.text);
    }
    // Esc or ` right after ` restores the Zhuyin as it was.
    if (literal && (key.is(K::Char, u'`') || key.is(K::Escape)) && !(before == after) && !after.isEmpty()
        && after.text == s.events[*literal].value("shown_before").toString()) {
        auto e = event("undo_literal", t, after.text);
        e.insert("shown_before", before.text);
        s.events.append(e);
        correction(s, "undo_literal", before.text);
        appendTyping(s, Key{label.isEmpty() ? QStringLiteral("<?>") : label, s.lastKey.value_or(0)}, after);
        return;
    }
    switch (key.kind) {
    case K::Char:
        if (key.character == u'`') {
            if (before == after) return;
            endRetyping(s, after);
            auto e = event("literal_zhuyin", t, after.text);
            e.insert("shown_before", before.text);
            if (mid) e.insert("mid", true);
            s.events.append(e);
            correction(s, "literal_zhuyin", before.text);
            appendTyping(s, Key{"`", s.lastKey.value_or(0)}, after);
            s.literal = int(s.events.size()) - 1;
            return;
        }
        break;
    case K::Backspace: case K::OptionBackspace: case K::ForwardDelete: case K::Escape:
        deletion(s, key, label, before, after, mid, t);
        return;
    default: break;
    }
    typed(s, Key{label.isEmpty() ? QStringLiteral("<?>") : label, s.lastKey.value_or(0)}, after, mid, t);
    if (label.isEmpty()) {
        s.replayable = false;
        s.originalReplayable = false;
    }
}

void CorrectionRecorder::typed(Session &s, const Key &key, const Presentation &after, bool mid, int t) {
    if (!s.events.isEmpty() && s.events.last().value("type").toString() == "type"
        && s.events.last().value("mid").toBool(false) == mid) {
        auto &last = s.events.last();
        last.insert("keys", last.value("keys").toString() + key.label);
        last.insert("shown", after.text);
        last.insert("t", t);
    } else {
        auto e = event("type", t, after.text);
        e.insert("keys", key.label);
        if (mid) e.insert("mid", true);
        s.events.append(e);
    }
    appendTyping(s, key, after);
    if (s.retyping && Shape(after.text).converted >= s.retyping->target) endRetyping(s, after);
}

void CorrectionRecorder::appendTyping(Session &s, const Key &key, const Presentation &after) {
    s.typing.append(key);
    if (s.retyping) s.retyping->keys.append(key); else s.original.append(key);
    if (s.tracking) s.history.append({int(s.typing.size()), Shape(after.text)});
}

void CorrectionRecorder::deletion(Session &s, const KeyInput &key, const QString &label, const Presentation &before,
                                  const Presentation &after, bool mid, int t) {
    if (before == after) return;
    const QString type = key.is(K::Escape) ? "escape" : "backspace";
    const std::optional<QString> cleared = key.is(K::Escape)
        ? std::optional<QString>(after.isEmpty() ? "composition" : "symbols") : std::nullopt;
    const QString removed = std::get<1>(difference(before.text, after.text));
    const Shape shape(after.text);
    // The typing keys behind what was removed: back to the last time the screen had the shape it has now.
    qsizetype index = -1;
    for (qsizetype i = s.history.size() - 1; i >= 0; --i)
        if (s.history[i].second == shape) { index = i; break; }
    if (!s.tracking || mid || index < 0 || s.history[index].first >= s.typing.size()) {
        endRetyping(s, after);
        const Key typedKey{label.isEmpty() ? QStringLiteral("<?>") : label, s.lastKey.value_or(0)};
        s.typing.append(typedKey);
        s.original.append(typedKey);
        if (s.tracking) s.history.append({int(s.typing.size()), shape});
        auto e = event(type, t, after.text);
        e.insert("shown_before", before.text);
        e.insert("removed", removed);
        setOptional(e, "cleared", cleared);
        if (mid) e.insert("mid", true);
        s.events.append(e);
        if (mid) correction(s, "cursor_edit", before.text);
        return;
    }
    const int markTyped = s.history[index].first;
    const QList<Key> deleted = s.typing.mid(markTyped);
    s.typing.resize(markTyped);
    s.history.resize(index + 1);
    QString deletedKeys;
    for (const auto &k : deleted) deletedKeys += k.label;
    if (s.retyping) {
        auto &retyping = *s.retyping;
        const int last = int(s.events.size()) - 1;
        if (retyping.keys.isEmpty() && retyping.event == last && s.events[last].value("type").toString() == type) {
            // Backspace held or pressed again: one deletion.
            auto &e = s.events[last];
            e.insert("deleted_keys", deletedKeys + e.value("deleted_keys").toString());
            e.insert("removed", std::get<1>(difference(e.value("shown_before").toString(), after.text)));
            e.insert("shown", after.text);
            e.insert("t", t);
            setOptional(e, "cleared", cleared);
            retyping.position = shape.converted;
            return;
        }
        if (deleted.size() <= retyping.keys.size()) {
            // A slip while retyping: the retyped keys lose it and the correction goes on.
            retyping.keys.resize(retyping.keys.size() - deleted.size());
            auto e = event(type, t, after.text);
            e.insert("shown_before", before.text);
            e.insert("removed", removed);
            e.insert("deleted_keys", deletedKeys);
            setOptional(e, "cleared", cleared);
            s.events.append(e);
            return;
        }
        // Deleting past the retyped keys: the keys as first typed can no longer be told apart.
        s.originalReplayable = false;
    }
    endRetyping(s, after);
    auto e = event(type, t, after.text);
    e.insert("shown_before", before.text);
    e.insert("removed", removed);
    e.insert("deleted_keys", deletedKeys);
    setOptional(e, "cleared", cleared);
    s.events.append(e);
    const Shape was(before.text);
    s.retyping = Retyping{int(s.events.size()) - 1, was.converted + (was.pending.isEmpty() ? 0 : 1), shape.converted, {}};
}

void CorrectionRecorder::endRetyping(Session &s, const Presentation &after) {
    if (!s.retyping) return;
    const Retyping retyping = *s.retyping;
    s.retyping.reset();
    const QJsonObject e = s.events[retyping.event];
    if (retyping.keys.isEmpty()) {
        const QString deleted = e.value("deleted_keys").toString();
        QString labels;
        for (const auto &k : s.original) labels += k.label;
        if (labels.endsWith(deleted)) {
            qsizetype remaining = deleted.size();
            while (remaining > 0 && !s.original.isEmpty()) remaining -= s.original.takeLast().label.size();
        } else {
            s.originalReplayable = false;
        }
        return;
    }
    QString retyped;
    for (const auto &k : retyping.keys) retyped += k.label;
    s.events[retyping.event].insert("retyped_keys", retyped);
    const auto all = scalars(after.text);
    const int end = int(all.size()) - scalarCount(Shape(after.text).pending);
    if (retyping.position <= end)
        s.events[retyping.event].insert("replaced_by", fromScalars(all, retyping.position, end));
    correction(s, e.value("type").toString(), e.contains("shown_before") ? e.value("shown_before").toString() : e.value("shown").toString());
}

void CorrectionRecorder::correction(Session &s, const QString &kind, const QString &shown) {
    if (!s.shown) s.shown = s.sent + shown;
    if (!s.corrections.contains(kind)) s.corrections.append(kind);
}

void CorrectionRecorder::carry(Session &s, const QString &committed, const Presentation &after, int t) {
    auto e = event("commit", t, after.text);
    e.insert("text", committed);
    s.events.append(e);
    s.sent += committed;
    // A deletion can no longer go back past the head: from here, deleting keys are recorded as typed.
    if (s.tracking) s.history = {{int(s.typing.size()), Shape(after.text)}};
}

void CorrectionRecorder::finish(Session &s, const QString &final, const ObservedInput &input, bool handled, double time,
                                int t) {
    endRetyping(s, Presentation());
    if (!final.isEmpty() || s.sent.isEmpty()) {
        auto e = event("commit", t, QString());
        e.insert("text", final);
        s.events.append(e);
    }
    const QString committed = s.sent + final;
    if (s.afterCommit) {
        const auto all = scalars(committed);
        const int length = scalarCount(s.afterCommit->first);
        if (fromScalars(all, 0, std::min<qsizetype>(length, all.size())) != s.afterCommit->first)
            correction(s, "delete_after_commit", s.afterCommit->second);
    }
    const QString typing = serialize(s.typing);
    if (!s.corrections.isEmpty() && sink_) {
        QJsonArray events;
        for (const auto &e : s.events) events.append(e);
        QJsonObject record{{"schema", 1}, {"id", QUuid::createUuid().toString(QUuid::WithoutBraces).toUpper()},
            {"date", today ? today() : QDate::currentDate().toString(Qt::ISODate)}, {"app_version", appVersion},
            {"settings", settings}, {"keys", s.keys}, {"typing", typing}, {"replayable", s.replayable},
            {"original_typing", serialize(s.original)}, {"original_replayable", s.replayable && s.originalReplayable},
            {"shown", s.shown.value_or(QString())}, {"committed", committed},
            {"corrections", QJsonArray::fromStringList(s.corrections)}, {"events", events}};
        if (s.context) record.insert("context", *s.context);
        sink_(record);
    }
    const auto context = s.context;
    const bool replayable = s.replayable;
    session_.reset();
    // What the app received after the commit: Backspace deletes it first.
    QString passed;
    if (!handled && input.kind == ObservedInput::Key) {
        if (input.key.is(K::Space)) passed = " ";
        else if (input.key.is(K::Enter) || input.key.is(K::ShiftEnter)) passed = "\n";
        else { lastCommit_.reset(); return; }
    }
    lastCommit_ = LastCommit{time, committed + passed, committed, scalarCount(passed), 0, typing, replayable, context};
}

void CorrectionRecorder::followDeletion(double time) {
    if (!lastCommit_ || time - lastCommit_->time >= window || lastCommit_->deleted >= scalarCount(lastCommit_->text)) {
        lastCommit_.reset();
        return;
    }
    lastCommit_->deleted += 1;
}

QString CorrectionRecorder::serialize(const QList<Key> &keys) {
    QString result;
    std::optional<double> previous;
    for (const auto &key : keys) {
        if (previous) result += QStringLiteral("<dt:%1>").arg(std::max(0, int(std::lround((key.time - *previous) * 1000))));
        result += key.label;
        previous = key.time;
    }
    return result;
}

QString CorrectionRecorder::keyLabel(const ObservedInput &input, const std::optional<CandidateListState> &list) const {
    switch (input.kind) {
    case ObservedInput::Key: return input.key.label();
    case ObservedInput::ToggleCorrections: return QStringLiteral("<sup>");
    case ObservedInput::Commit: return QStringLiteral("<enter>");
    case ObservedInput::Click: {
        // A click is written as the selection key at that place on the page, which picks the same item.
        if (!list) return {};
        const int number = input.index - list->page * list->pageSize;
        return number >= 0 && number < selectionKeys.size() ? QString(selectionKeys[number]) : QString();
    }
    }
    return {};
}

bool CorrectionRecorder::isListKey(const KeyInput &key) const {
    switch (key.kind) {
    case K::Char: return key.character == u'`' || selectionKeys.contains(QChar(key.character));
    case K::Enter: case K::ShiftDown: case K::ShiftUp: case K::Escape: case K::Backspace: case K::Left: case K::Right:
    case K::Up: case K::Down: case K::PageUp: case K::PageDown: case K::Tab: case K::ShiftTab: case K::Space:
        return true;
    default: return false;
    }
}

QString CorrectionRecorder::kind(const CandidateListState &list) {
    if (list.single) return list.tolerant == 1 ? "single_corrections" : "single";
    return list.tolerant == 1 ? "corrections" : "normal";
}

std::optional<QString> CorrectionRecorder::focused(const Presentation &p) {
    if (p.focusedClause < 0 || p.focusedClause >= p.clauses.size()) return std::nullopt;
    const auto range = p.clauses[p.focusedClause];
    if (range.first < 0 || range.second > p.text.size() || range.second < range.first) return std::nullopt;
    return p.text.mid(range.first, range.second - range.first);
}

std::tuple<int, QString, QString> CorrectionRecorder::difference(const QString &a, const QString &b) {
    const auto x = scalars(a), y = scalars(b);
    qsizetype prefix = 0;
    while (prefix < std::min(x.size(), y.size()) && x[prefix] == y[prefix]) ++prefix;
    qsizetype suffix = 0;
    while (suffix < std::min(x.size(), y.size()) - prefix && x[x.size() - 1 - suffix] == y[y.size() - 1 - suffix]) ++suffix;
    return {int(prefix), fromScalars(x, prefix, x.size() - suffix), fromScalars(y, prefix, y.size() - suffix)};
}

// MARK: - The file

CorrectionLog::CorrectionLog(Settings &settings, const QString &directory, QObject *parent)
    : QObject(parent), settings_(settings), path_(QDir(directory).filePath(fileName)) {
    bool last = recordsContext();
    connect(&settings_, &Settings::changed, this, [this, last]() mutable {
        if (recordsContext() != last) { last = recordsContext(); emit recordsContextChanged(); }
    });
    recount();
}

QUrl CorrectionLog::location() const { return QUrl::fromLocalFile(path_); }

bool CorrectionLog::recordsContext() const { return settings_.values().value("recordsCorrectionContext").toBool(); }

void CorrectionLog::setRecordsContext(bool value) {
    if (value != recordsContext()) settings_.setValue("recordsCorrectionContext", value);
}

bool CorrectionLog::fail(const QString &message) {
    error_ = message;
    emit errorChanged();
    return false;
}

QByteArray CorrectionLog::encode(const QJsonObject &record) {
    // QJsonObject keeps its keys sorted, as the Mac's JSONEncoder with .sortedKeys; slashes are not escaped.
    return QJsonDocument(record).toJson(QJsonDocument::Compact);
}

bool CorrectionLog::append(const QJsonObject &record) {
    QDir().mkpath(QFileInfo(path_).absolutePath());
    QFile file(path_);
    const bool existed = file.exists();
    if (!file.open(QIODevice::WriteOnly | QIODevice::Append)) return fail(QStringLiteral("無法寫入改字記錄。"));
    if (!existed) file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    const QByteArray line = encode(record) + '\n';
    if (file.write(line) != line.size()) return fail(QStringLiteral("無法寫入改字記錄。"));
    file.close();
    ++count_;
    if (!error_.isEmpty()) { error_.clear(); emit errorChanged(); }
    emit changed();
    return true;
}

void CorrectionLog::recount() {
    QFile file(path_);
    int lines = 0;
    if (file.open(QIODevice::ReadOnly)) {
        while (!file.atEnd()) lines += int(file.read(1 << 16).count('\n'));
    }
    if (lines != count_) { count_ = lines; emit changed(); }
}

bool CorrectionLog::showLog() {
    const QFileInfo info(path_);
#if defined(Q_OS_WIN)
    if (info.exists()) return QProcess::startDetached("explorer.exe", {"/select,", QDir::toNativeSeparators(path_)});
#elif defined(Q_OS_MACOS)
    if (info.exists()) return QProcess::startDetached("open", {"-R", path_});
#endif
    QDir().mkpath(info.absolutePath());
    return QDesktopServices::openUrl(QUrl::fromLocalFile(info.absolutePath()));
}

bool CorrectionLog::exportLog(const QUrl &destination) {
    if (!destination.isLocalFile()) return fail(QStringLiteral("請選擇這台電腦上的位置。"));
    QFile source(path_);
    QByteArray bytes;
    if (source.exists()) {
        if (!source.open(QIODevice::ReadOnly)) return fail(QStringLiteral("無法讀取改字記錄。"));
        bytes = source.readAll();
    }
    QSaveFile target(destination.toLocalFile());
    if (!target.open(QIODevice::WriteOnly) || target.write(bytes) != bytes.size() || !target.commit())
        return fail(QStringLiteral("無法匯出改字記錄。"));
    return true;
}

bool CorrectionLog::clearLog() {
    if (QFile::exists(path_) && !QFile::remove(path_)) return fail(QStringLiteral("無法清除改字記錄。"));
    recount();
    if (count_ != 0) { count_ = 0; emit changed(); }
    return true;
}
