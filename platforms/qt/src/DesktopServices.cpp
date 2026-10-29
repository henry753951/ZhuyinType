#include "DesktopServices.h"

#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QRegularExpression>
#include <QSaveFile>
#include <QTimer>
#include <QUrl>
#include <QVersionNumber>
#include <cmath>
#include <memory>
#include <utility>

#if defined(Q_OS_WIN)
#define NOMINMAX
#include <windows.h>
#include <psapi.h>
#else
#include <sys/resource.h>
#endif
#if defined(Q_OS_MACOS)
#include <libproc.h>
#include <unistd.h>
#endif

namespace {
constexpr qsizetype MaxDocumentBytes = 256 * 1024;
constexpr qsizetype MaxStatsBytes = 4 * 1024 * 1024;
constexpr int SchemaVersion = 2;
/// A gap longer than this is time away, not thinking: it counts as neither (UsageStatsObserver.awayAfter).
constexpr double AwayAfterSeconds = 60;
const QString ReleasesUrl = QStringLiteral("https://github.com/henry753951/ZhuyinType/releases");
const QList<double> LatencyBounds = {1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 25, 30, 40, 50, 75, 100, 150, 250, 500, 1000};
const QList<double> IntervalBounds = {0.04, 0.06, 0.08, 0.1, 0.12, 0.15, 0.18, 0.22, 0.26, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8,
                                      1.0, 1.3, 1.6, 2.0, 3.0, 4.0, 6.0};
const QList<double> PauseBounds = {2, 3, 4, 5, 6, 8, 10, 13, 16, 20, 25, 30, 40, 50, 60};

QVersionNumber versionNumber(const QString &text) {
    static const QRegularExpression pattern(QStringLiteral("^v?(\\d+\\.\\d+\\.\\d+)$"));
    const auto match = pattern.match(text);
    return match.hasMatch() ? QVersionNumber::fromString(match.captured(1)) : QVersionNumber();
}

// The counters by their stats.json names, in one place for reading, writing and QML.
using Field = qint64 DesktopServices::Counters::*;
const QList<QPair<QString, Field>> &fields() {
    using C = DesktopServices::Counters;
    static const QList<QPair<QString, Field>> list = {
        {"characters", &C::characters}, {"compositions", &C::compositions}, {"cleanCompositions", &C::cleanCompositions},
        {"keys", &C::keys}, {"picksNormal", &C::picksNormal}, {"picksCorrections", &C::picksCorrections},
        {"picksSingle", &C::picksSingle}, {"picksPunctuation", &C::picksPunctuation}, {"backspaces", &C::backspaces},
        {"escapes", &C::escapes}, {"deleteAfterCommit", &C::deleteAfterCommit}, {"literalZhuyin", &C::literalZhuyin},
        {"undoLiteral", &C::undoLiteral}, {"correctedShown", &C::correctedShown},
        {"correctedCommitted", &C::correctedCommitted}, {"phrasesAdded", &C::phrasesAdded},
        {"typingMilliseconds", &C::typingMilliseconds}, {"thinkingMilliseconds", &C::thinkingMilliseconds}};
    return list;
}

bool count(const QJsonValue &value, qint64 &result) {
    if (value.isUndefined() || value.isNull()) { result = 0; return true; } // Fields added later start at zero.
    if (!value.isDouble()) return false;
    const double number = value.toDouble();
    if (!std::isfinite(number) || number < 0 || number > 9007199254740991.0 || std::floor(number) != number) return false;
    result = qint64(number);
    return true;
}
bool decode(const QJsonValue &value, DesktopServices::Counters &counters) {
    if (!value.isObject()) return false;
    const auto object = value.toObject();
    for (const auto &[name, field] : fields())
        if (!count(object.value(name), counters.*field)) return false;
    return true;
}
QJsonObject encode(const DesktopServices::Counters &counters) {
    QJsonObject object;
    for (const auto &[name, field] : fields()) object.insert(name, double(counters.*field));
    return object;
}
bool decode(const QJsonValue &value, DesktopServices::Buckets &buckets) {
    if (value.isUndefined()) return true;
    const auto object = value.toObject();
    const auto counts = object.value("counts").toArray();
    if (!value.isObject() || counts.size() != buckets.bounds.size() + 1) return false;
    for (int i = 0; i < counts.size(); ++i)
        if (!count(counts.at(i), buckets.counts[i])) return false;
    const double maximum = object.value("maximum").toDouble(0);
    buckets.maximum = std::isfinite(maximum) && maximum >= 0 ? maximum : 0;
    return true;
}
QJsonObject encode(const DesktopServices::Buckets &buckets, bool withBounds) {
    QJsonArray counts;
    for (const auto value : buckets.counts) counts.append(double(value));
    QJsonObject object{{"counts", counts}, {"maximum", buckets.maximum}};
    if (withBounds) {
        QJsonArray bounds;
        for (const auto value : buckets.bounds) bounds.append(value);
        object.insert("bounds", bounds);
    }
    return object;
}
QVariant optional(double value) { return value < 0 ? QVariant() : QVariant(value); }
}

// MARK: Counters and buckets

DesktopServices::Counters &DesktopServices::Counters::operator+=(const Counters &other) {
    for (const auto &[name, field] : fields()) this->*field += other.*field;
    return *this;
}
bool DesktopServices::Counters::isZero() const {
    for (const auto &[name, field] : fields())
        if (this->*field != 0) return false;
    return true;
}

DesktopServices::Buckets::Buckets(QList<double> upper) : bounds(std::move(upper)), counts(bounds.size() + 1, 0) {}
qint64 DesktopServices::Buckets::total() const {
    qint64 sum = 0;
    for (const auto value : counts) sum += value;
    return sum;
}
void DesktopServices::Buckets::add(double value) {
    qsizetype index = 0;
    while (index < bounds.size() && value > bounds.at(index)) ++index;
    counts[index] += 1;
    if (value > maximum) maximum = value;
}
double DesktopServices::Buckets::percentile(double fraction) const {
    const auto all = total();
    if (all <= 0) return -1;
    const double target = fraction * double(all);
    double seen = 0;
    for (qsizetype index = 0; index < counts.size(); ++index) {
        const auto value = counts.at(index);
        if (value <= 0) continue;
        if (seen + double(value) >= target) {
            const double low = index == 0 ? 0 : bounds.at(index - 1);
            const double high = index < bounds.size() ? bounds.at(index) : qMax(maximum, low);
            return qMin(maximum, low + (high - low) * (target - seen) / double(value));
        }
        seen += double(value);
    }
    return maximum;
}

DesktopServices::Rhythm::Rhythm() : intervals(IntervalBounds), pauses(PauseBounds) {}
double DesktopServices::Rhythm::pauseThreshold() const {
    const double median = intervals.percentile(0.5);
    return qMax(2.0, 4 * (median < 0 ? 0.25 : median));
}

// MARK: The store

DesktopServices::DesktopServices(QString statsPath, QString currentVersion, QObject *parent, QNetworkAccessManager *network,
                                 std::function<QDateTime()> now)
    : QObject(parent), path_(std::move(statsPath)), currentVersion_(std::move(currentVersion)),
      network_(network ? network : new QNetworkAccessManager(this)), now_(std::move(now)), latency_(LatencyBounds) {
    clock_.start();
    notify_.setSingleShot(true);
    notify_.setInterval(2000);
    connect(&notify_, &QTimer::timeout, this, &DesktopServices::usageChanged);
    uptime_ = [this] { return clock_.elapsed(); };
    reset();
    load();
    sessionThreshold_ = rhythm_.pauseThreshold();
}

DesktopServices::~DesktopServices() { saveIfNeeded(); }

void DesktopServices::reset() {
    since_ = now_();
    totals_ = {};
    days_.clear();
    latency_ = Buckets(LatencyBounds);
    rhythm_ = Rhythm();
    lastKey_ = -1;
    burst_ = cleanRun_ = 0;
    corrected_ = sawCorrected_ = committedCorrected_ = false;
}

bool DesktopServices::load() {
    if (path_.isEmpty() || !QFileInfo::exists(path_)) return true;
    QFile file(path_);
    QByteArray bytes;
    if (file.open(QIODevice::ReadOnly) && file.size() <= MaxStatsBytes) bytes = file.readAll();
    QJsonParseError error;
    const auto document = QJsonDocument::fromJson(bytes, &error);
    const auto object = document.object();
    const auto since = QDateTime::fromString(object.value("since").toString(), Qt::ISODate);
    Counters totals;
    QMap<QDate, Counters> days;
    Buckets latency(LatencyBounds);
    Rhythm rhythm;
    bool ok = error.error == QJsonParseError::NoError && document.isObject() && since.isValid()
        && decode(object.value("totals"), totals) && object.value("days").isObject()
        && (object.value("latency").isUndefined() || decode(object.value("latency"), latency));
    const auto storedDays = object.value("days").toObject();
    for (auto it = storedDays.constBegin(); ok && it != storedDays.constEnd(); ++it) {
        const auto date = QDate::fromString(it.key(), Qt::ISODate);
        Counters day;
        ok = date.isValid() && decode(it.value(), day);
        days.insert(date, day);
    }
    const auto storedRhythm = object.value("rhythm").toObject();
    if (ok && object.value("rhythm").isObject()) {
        // As on macOS, a rhythm of an older shape starts empty rather than failing the whole file.
        if (!decode(storedRhythm.value("intervals"), rhythm.intervals)) rhythm.intervals = Buckets(IntervalBounds);
        if (!decode(storedRhythm.value("pauses"), rhythm.pauses)) rhythm.pauses = Buckets(PauseBounds);
        count(storedRhythm.value("longestRun"), rhythm.longestRun);
        count(storedRhythm.value("longestCleanRun"), rhythm.longestCleanRun);
        const auto hours = storedRhythm.value("hours").toArray();
        if (hours.size() == int(rhythm.hours.size()))
            for (int i = 0; i < hours.size(); ++i) count(hours.at(i), rhythm.hours[size_t(i)]);
    }
    if (!ok) {
        // Never overwrite what cannot be read: counting waits until 清除統計.
        readable_ = false;
        usageError_ = QStringLiteral("使用統計資料無法讀取，請先清除統計再開始記錄。");
        return false;
    }
    since_ = since;
    totals_ = totals;
    days_ = days;
    latency_ = latency;
    rhythm_ = rhythm;
    return true;
}

bool DesktopServices::write() {
    if (path_.isEmpty()) { dirty_ = false; return true; }
    QJsonObject days;
    for (auto it = days_.cbegin(); it != days_.cend(); ++it) days.insert(it.key().toString(Qt::ISODate), encode(it.value()));
    QJsonArray hours;
    for (const auto value : rhythm_.hours) hours.append(double(value));
    const QJsonObject object{
        {"schema", SchemaVersion}, {"since", since_.toUTC().toString(Qt::ISODate)}, {"totals", encode(totals_)},
        {"days", days}, {"latency", encode(latency_, false)},
        {"rhythm", QJsonObject{{"intervals", encode(rhythm_.intervals, true)}, {"pauses", encode(rhythm_.pauses, true)},
                               {"longestRun", double(rhythm_.longestRun)}, {"longestCleanRun", double(rhythm_.longestCleanRun)},
                               {"hours", hours}}}};
    QDir().mkpath(QFileInfo(path_).absolutePath());
    QSaveFile file(path_);
    const bool ok = file.open(QIODevice::WriteOnly) && file.write(QJsonDocument(object).toJson(QJsonDocument::Compact)) >= 0
        && file.commit();
    ++writes_;
    lastSave_ = uptime_();
    if (!ok) {
        usageError_ = QStringLiteral("無法儲存使用統計，請檢查資料夾的寫入權限。");
        emit usageChanged();
        return false;
    }
    QFile::setPermissions(path_, QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    dirty_ = false;
    return true;
}

bool DesktopServices::saveIfNeeded() {
    if (!dirty_ || !readable_) return true;
    return write();
}

void DesktopServices::changed() {
    dirty_ = true;
    const auto now = uptime_();
    if (lastSave_ < 0) lastSave_ = now;
    else if (now - lastSave_ >= SaveIntervalMs) write();
    // The 分析 page follows every two seconds at most (as macOS refreshes it); typing never waits for it.
    if (!notify_.isActive()) notify_.start();
}

void DesktopServices::add(const Counters &delta) {
    if (delta.isZero()) return;
    const auto now = now_();
    const auto day = now.date();
    days_[day] += delta;
    totals_ += delta;
    if (delta.characters > 0) {
        // Sunday first, as Calendar's weekday on macOS.
        const int weekday = day.dayOfWeek() % 7;
        rhythm_.hours[size_t(weekday * 24 + now.time().hour())] += delta.characters;
    }
    while (days_.size() > KeptDays) days_.erase(days_.begin());
}

void DesktopServices::noteKey(const QVariantMap &data) {
    Counters delta;
    if (!externalRhythm_) {
        const auto now = data.contains("atMs") ? data.value("atMs").toLongLong() : uptime_();
        if (lastKey_ >= 0) {
            const double gap = double(now - lastKey_) / 1000;
            if (gap >= 0 && gap <= sessionThreshold_) {
                delta.typingMilliseconds = qint64(gap * 1000);
                rhythm_.intervals.add(gap);
            } else if (gap > sessionThreshold_ && gap <= AwayAfterSeconds) {
                delta.thinkingMilliseconds = qint64(gap * 1000);
                rhythm_.pauses.add(gap);
                burst_ = cleanRun_ = 0;
            } else {
                burst_ = cleanRun_ = 0;
            }
        }
        lastKey_ = now;
    }
    if (data.value("composing", true).toBool()) {
        delta.keys = 1;
        bool ok = false;
        const double latency = data.value("latencyMs").toDouble(&ok);
        if (ok && std::isfinite(latency) && latency >= 0 && latency <= 600000) latency_.add(latency);
    }
    add(delta);
}

void DesktopServices::recordUsageEvent(const QString &kind, const QVariantMap &data) {
    if (!enabled_ || !readable_) return;
    Counters delta;
    if (kind == QLatin1String("key")) {
        noteKey(data);
        changed();
        return;
    } else if (kind == QLatin1String("pause") || kind == QLatin1String("interval")) {
        // The caller measures the rhythm itself: key gaps are no longer measured here.
        externalRhythm_ = true;
        bool ok = false;
        const double ms = data.value("ms").toDouble(&ok);
        if (!ok || !std::isfinite(ms) || ms < 0 || ms > AwayAfterSeconds * 1000) return;
        if (kind == QLatin1String("pause")) {
            delta.thinkingMilliseconds = qint64(ms);
            rhythm_.pauses.add(ms / 1000);
            burst_ = cleanRun_ = 0;
        } else {
            delta.typingMilliseconds = qint64(ms);
            rhythm_.intervals.add(ms / 1000);
        }
    } else if (kind == QLatin1String("commit")) {
        const auto characters = data.value("characters").toLongLong();
        if (characters <= 0 || characters > 1000000) return;
        delta.characters = characters;
        burst_ += characters;
        cleanRun_ += characters;
        rhythm_.longestRun = qMax(rhythm_.longestRun, burst_);
        rhythm_.longestCleanRun = qMax(rhythm_.longestCleanRun, cleanRun_);
        if (data.value("compositionEnded", true).toBool()) {
            delta.compositions = 1;
            const bool clean = data.contains("clean") ? data.value("clean").toBool() : !corrected_;
            if (clean) delta.cleanCompositions = 1;
            if (sawCorrected_ || data.value("correctedShown").toBool()) delta.correctedShown = 1;
            if (committedCorrected_ || data.value("correctedCommitted").toBool()) delta.correctedCommitted = 1;
            corrected_ = sawCorrected_ = committedCorrected_ = false;
        }
    } else if (kind == QLatin1String("pick")) {
        const auto list = data.value("list").toString();
        if (list == QLatin1String("corrections")) delta.picksCorrections = 1;
        else if (list == QLatin1String("single")) delta.picksSingle = 1;
        else if (list == QLatin1String("punctuation")) delta.picksPunctuation = 1;
        else delta.picksNormal = 1;
    } else if (kind == QLatin1String("backspace")) delta.backspaces = 1;
    else if (kind == QLatin1String("escape")) delta.escapes = 1;
    else if (kind == QLatin1String("deleteAfterCommit")) delta.deleteAfterCommit = 1;
    else if (kind == QLatin1String("literalZhuyin")) delta.literalZhuyin = 1;
    else if (kind == QLatin1String("undoLiteral")) delta.undoLiteral = 1;
    else if (kind == QLatin1String("phraseAdded")) delta.phrasesAdded = 1;
    else if (kind == QLatin1String("correctionShown")) { sawCorrected_ = true; return; }
    else if (kind == QLatin1String("correctionCommitted")) { committedCorrected_ = true; return; }
    else if (kind == QLatin1String("reset")) {
        lastKey_ = -1;
        burst_ = cleanRun_ = 0;
        corrected_ = sawCorrected_ = committedCorrected_ = false;
        return;
    } else {
        return;
    }
    if (delta.picks() > 0 || delta.backspaces > 0 || delta.escapes > 0 || delta.literalZhuyin > 0 || delta.deleteAfterCommit > 0) {
        corrected_ = true;
        cleanRun_ = 0;
    }
    add(delta);
    changed();
}

QVariantMap DesktopServices::usage() const {
    const auto today = this->today();
    Counters month;
    QVariantList days;
    for (int back = 29; back >= 0; --back) {
        const auto date = today.addDays(-back);
        const auto day = days_.value(date);
        month += day;
        days.append(QVariantMap{{"date", date.toString(Qt::ISODate)}, {"characters", day.characters}});
    }
    const auto &counters = period_ == QLatin1String("all") ? totals_ : month;
    QVariantMap result{{"valid", readable_}, {"since", since_.date().toString(Qt::ISODate)}, {"days", days}};
    for (const auto &[name, field] : fields()) result.insert(name, counters.*field);
    result.insert("picks", counters.picks());
    result.insert("corrections", counters.corrections());
    result.insert("activeSpeed", counters.typingMilliseconds >= 1000
        ? QVariant(double(counters.characters) * 60000 / double(counters.typingMilliseconds)) : QVariant());
    result.insert("firstTimeRight", counters.compositions > 0
        ? QVariant(double(counters.cleanCompositions) / double(counters.compositions)) : QVariant());
    // Consecutive days with typing up to today, or yesterday when today has none yet.
    int streak = 0;
    for (int back = 0; back < KeptDays; ++back) {
        const auto characters = days_.value(today.addDays(-back)).characters;
        if (back == 0 && characters == 0) continue;
        if (characters == 0) break;
        ++streak;
    }
    result.insert("streak", streak);
    QDate bestDate;
    qint64 best = 0;
    for (auto it = days_.cbegin(); it != days_.cend(); ++it)
        if (it.value().characters > best) { best = it.value().characters; bestDate = it.key(); }
    result.insert("bestDay", best > 0 ? QVariant(QVariantMap{{"date", bestDate.toString(Qt::ISODate)}, {"characters", best}}) : QVariant());
    QVariantList latencyCounts;
    for (const auto value : latency_.counts) latencyCounts.append(value);
    QVariantList latencyBounds;
    for (const auto value : latency_.bounds) latencyBounds.append(value);
    result.insert("latency", QVariantMap{{"total", latency_.total()}, {"p50", optional(latency_.percentile(0.5))},
                                         {"p95", optional(latency_.percentile(0.95))},
                                         {"maximum", latency_.total() > 0 ? QVariant(latency_.maximum) : QVariant()},
                                         {"counts", latencyCounts}, {"bounds", latencyBounds}});
    QVariantList hours;
    for (const auto value : rhythm_.hours) hours.append(value);
    result.insert("rhythm", QVariantMap{{"longestRun", rhythm_.longestRun}, {"longestCleanRun", rhythm_.longestCleanRun},
                                        {"pauseThreshold", rhythm_.pauseThreshold()},
                                        {"pauseMedian", optional(rhythm_.pauses.percentile(0.5))}, {"hours", hours}});
    return result;
}

bool DesktopServices::setUsageEnabled(bool enabled) {
    if (enabled == enabled_) return true;
    enabled_ = enabled;
    if (!enabled) {
        saveIfNeeded();
        lastKey_ = -1;
    }
    emit usageChanged();
    return true;
}

bool DesktopServices::setPeriod(const QString &period) {
    if (period != QLatin1String("month") && period != QLatin1String("all")) return false;
    if (period_ != period) { period_ = period; emit usageChanged(); }
    return true;
}

bool DesktopServices::clearUsage() {
    if (!path_.isEmpty() && QFileInfo::exists(path_) && !QFile::remove(path_)) {
        usageError_ = QStringLiteral("無法刪除使用統計檔案，請檢查資料夾的寫入權限。");
        emit usageChanged();
        return false;
    }
    reset();
    sessionThreshold_ = rhythm_.pauseThreshold();
    readable_ = true;
    usageError_.clear();
    dirty_ = false;
    emit usageChanged();
    return true;
}

// MARK: The process itself

QVariantMap DesktopServices::processMetrics() const {
    static const QDateTime launched = QDateTime::currentDateTime();
    QVariantMap result{{"launched", launched.toString(Qt::ISODate)},
                       {"running", double(launched.msecsTo(QDateTime::currentDateTime())) / 1000}};
#if defined(Q_OS_WIN)
    FILETIME created, exited, kernel, user;
    if (GetProcessTimes(GetCurrentProcess(), &created, &exited, &kernel, &user)) {
        auto seconds = [](const FILETIME &time) {
            return double((quint64(time.dwHighDateTime) << 32) | time.dwLowDateTime) / 1e7;
        };
        result.insert("cpuSeconds", seconds(kernel) + seconds(user));
    }
    PROCESS_MEMORY_COUNTERS_EX memory{};
    if (GetProcessMemoryInfo(GetCurrentProcess(), reinterpret_cast<PROCESS_MEMORY_COUNTERS *>(&memory), sizeof(memory))) {
        result.insert("memoryBytes", double(memory.PrivateUsage));
        result.insert("peakMemoryBytes", double(memory.PeakPagefileUsage));
    }
#else
    rusage usage{};
    if (getrusage(RUSAGE_SELF, &usage) == 0) {
        auto seconds = [](const timeval &time) { return double(time.tv_sec) + double(time.tv_usec) / 1e6; };
        result.insert("cpuSeconds", seconds(usage.ru_utime) + seconds(usage.ru_stime));
    }
#endif
#if defined(Q_OS_MACOS)
    rusage_info_v4 info{};
    if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, reinterpret_cast<rusage_info_t *>(&info)) == 0) {
        result.insert("memoryBytes", double(info.ri_phys_footprint));
        result.insert("peakMemoryBytes", double(info.ri_lifetime_max_phys_footprint));
    }
#elif defined(Q_OS_LINUX)
    // Resident memory now and at its highest, from the kernel's own accounting; no energy figure exists per process.
    QFile status(QStringLiteral("/proc/self/status"));
    if (status.open(QIODevice::ReadOnly | QIODevice::Text)) {
        for (const auto &line : status.readAll().split('\n')) {
            const auto parts = line.simplified().split(' ');
            if (parts.size() >= 2 && (parts[0] == "VmRSS:" || parts[0] == "VmHWM:"))
                result.insert(parts[0] == "VmRSS:" ? "memoryBytes" : "peakMemoryBytes", parts[1].toDouble() * 1024);
        }
    }
#endif
    return result;
}

// MARK: Updates

void DesktopServices::checkForUpdates() {
    if (checking_) return;
    checking_ = true;
    updateError_.clear();
    update_.clear();
    emit updateChanged();
    QNetworkRequest request(QUrl(QStringLiteral("https://api.github.com/repos/henry753951/ZhuyinType/releases/latest")));
    request.setRawHeader("Accept", "application/vnd.github+json");
    request.setRawHeader("User-Agent", "ZhuyinType-Desktop");
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
    request.setTransferTimeout(10000);
    auto *reply = network_->get(request);
    reply->setReadBufferSize(MaxDocumentBytes + 1);
    QTimer::singleShot(10000, reply, [reply] { if (!reply->isFinished()) reply->abort(); });
    auto body = std::make_shared<QByteArray>();
    auto oversized = std::make_shared<bool>(false);
    auto read = [reply, body, oversized] {
        body->append(reply->read(MaxDocumentBytes + 1 - body->size()));
        if (body->size() > MaxDocumentBytes) { *oversized = true; if (!reply->isFinished()) reply->abort(); }
    };
    connect(reply, &QIODevice::readyRead, this, read);
    connect(reply, &QNetworkReply::finished, this, [this, reply, body, oversized, read] {
        if (!*oversized) read();
        if (*oversized) updateError_ = QStringLiteral("更新回應超過大小限制。");
        else if (reply->error() != QNetworkReply::NoError)
            updateError_ = QStringLiteral("無法檢查更新：%1").arg(reply->errorString().left(300));
        else if (reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt() != 200)
            updateError_ = QStringLiteral("更新服務回傳 HTTP %1。").arg(reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt());
        else parseRelease(*body);
        checking_ = false;
        if (!updateError_.isEmpty()) update_.insert("status", QStringLiteral("檢查失敗"));
        update_.insert("checkedAt", QDateTime::currentDateTimeUtc().toString(Qt::ISODate));
        emit updateChanged();
        reply->deleteLater();
    });
}

bool DesktopServices::parseRelease(const QByteArray &body) {
    QJsonParseError error;
    const auto document = QJsonDocument::fromJson(body, &error);
    const auto release = document.object();
    const auto tag = release.value("tag_name").toString();
    const auto version = versionNumber(tag);
    const auto current = versionNumber(currentVersion_);
    const QUrl url(release.value("html_url").toString());
    const auto assetsValue = release.value("assets");
    if (error.error != QJsonParseError::NoError || !document.isObject() || tag.size() > 64 || version.isNull() || current.isNull()
        || release.value("draft") != QJsonValue(false) || release.value("prerelease") != QJsonValue(false)
        || url.scheme() != "https" || url.host() != "github.com" || !url.userInfo().isEmpty() || url.port() != -1
        || !url.path().startsWith("/henry753951/ZhuyinType/releases/tag/") || !url.query().isEmpty() || !url.fragment().isEmpty()
        || !assetsValue.isArray() || assetsValue.toArray().size() > 128) {
        updateError_ = QStringLiteral("更新資料格式無法辨識，請至發布頁面查看。");
        return false;
    }
    QStringList names;
    for (const auto &asset : assetsValue.toArray()) {
        const auto name = asset.toObject().value("name");
        if (!asset.isObject() || !name.isString() || name.toString().isEmpty() || name.toString().size() > 240) {
            updateError_ = QStringLiteral("更新附件格式無法辨識。");
            return false;
        }
        names.append(name.toString());
    }
    update_ = {{"version", tag}, {"newer", QVersionNumber::compare(version, current) > 0},
               {"releaseUrl", url.toString()}, {"assets", names},
               {"status", QVersionNumber::compare(version, current) > 0 ? QStringLiteral("有較新版本") : QStringLiteral("目前沒有較新版本")},
               {"artifactStatus", QStringLiteral("以下為發布附件；尚未確認此 Qt 桌面平台的相容更新檔。請至發布頁面確認系統需求。")}};
    return true;
}

bool DesktopServices::openReleasePage() {
    if (QDesktopServices::openUrl(QUrl(update_.value("releaseUrl", ReleasesUrl).toString()))) return true;
    updateError_ = QStringLiteral("無法開啟瀏覽器，請前往 GitHub 的 ZhuyinType 發布頁面。");
    emit updateChanged();
    return false;
}
