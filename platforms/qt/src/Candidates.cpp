#include "Candidates.h"
#include <algorithm>

void Candidates::present(QVariantList entries, QString reading, int selected) {
    if (entries != entries_ || reading != reading_) ++revision_;
    entries_ = std::move(entries);
    reading_ = std::move(reading);
    const int index = std::clamp(selected, 0, std::max(0, int(entries_.size()) - 1));
    page_ = index / pageSize_;
    selected_ = entries_.isEmpty() ? -1 : index % pageSize_;
    emit changed();
}

QVariantList Candidates::rows() const { return entries_.mid(page_ * pageSize_, pageSize_); }
int Candidates::pageCount() const { return std::max(1, (int(entries_.size()) + pageSize_ - 1) / pageSize_); }

void Candidates::setPageSize(int size) {
    if (size < 3 || size > 9 || size == pageSize_)
        return;
    const int index = page_ * pageSize_ + std::max(0, selected_);
    pageSize_ = size;
    ++revision_;
    page_ = index / size;
    selected_ = entries_.isEmpty() ? -1 : index % size;
    emit changed();
}

void Candidates::turnPage(int delta) {
    const auto next = int(std::clamp(qint64(page_) + delta, qint64(0), qint64(pageCount() - 1)));
    if (next == page_) {
        if (delta != 0 && !entries_.isEmpty()) emit boundaryReached(delta > 0 ? 1 : -1);
        return;
    }
    page_ = next;
    selected_ = std::min(selected_, int(rows().size()) - 1);
    emit changed();
}

void Candidates::pick(int row) {
    if (row < 0 || row >= rows().size())
        return;
    selected_ = row;
    emit changed();
    emit picked(page_ * pageSize_ + row);
}

void Candidates::moveSelection(int delta) {
    if (entries_.isEmpty()) return;
    const auto current = qint64(page_) * pageSize_ + selected_;
    const auto next = std::clamp(current + delta, qint64(0), qint64(entries_.size() - 1));
    if (next == current) {
        if (delta != 0) emit boundaryReached(delta > 0 ? 1 : -1);
        return;
    }
    page_ = int(next / pageSize_);
    selected_ = int(next % pageSize_);
    emit changed();
}

void Candidates::toggleSingle() {
    single_ = !single_;
    ++revision_;
    emit changed();
    emit filterRequested(single_, corrections_);
}

void Candidates::toggleCorrections() {
    corrections_ = !corrections_;
    ++revision_;
    emit changed();
    emit filterRequested(single_, corrections_);
}
