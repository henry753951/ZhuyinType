#pragma once

#include "EngineSession.h"
#include <QList>
#include <QPair>
#include <optional>

// What the composer shows at one moment, as macOS's Presentation: the observers compare the one before a key with
// the one after it.  Offsets are UTF-16, as EngineSession reports them.
struct CandidateListState {
    int count = 0;
    int page = 0;
    int pageSize = 9;
    int selection = 0;
    /// 1 corrections, 0 words as typed, -1 the list cannot have corrections.
    int tolerant = -1;
    bool single = false;
    bool operator==(const CandidateListState &) const = default;
};

struct Presentation {
    QString text;
    int cursor = 0;
    QList<QPair<int, int>> clauses;
    int focusedClause = -1;
    std::optional<CandidateListState> candidates;
    QString zhuyinHint;
    QString notice;
    bool operator==(const Presentation &) const = default;
    bool isEmpty() const { return text.isEmpty(); }

    static Presentation of(const EngineSession &session) {
        Presentation p;
        if (!session.ready()) return p;
        p.text = session.composition();
        p.cursor = session.cursor();
        for (const auto &clause : session.clauses()) {
            const auto map = clause.toMap();
            p.clauses.append({map.value("start").toInt(), map.value("end").toInt()});
        }
        p.focusedClause = session.focusedClause();
        if (!session.rows().isEmpty()) {
            CandidateListState list;
            list.count = int(session.rows().size());
            list.pageSize = session.pageSize();
            list.selection = qMax(0, session.selected());
            list.page = session.page();
            list.tolerant = session.tolerant();
            list.single = session.single();
            p.candidates = list;
        }
        p.zhuyinHint = session.reading();
        p.notice = session.notice();
        return p;
    }
};
