#include "ImeHost.h"
#include "ImeEndpoint.h"
#include <QGuiApplication>
#include <QScreen>
#include <QJsonArray>
#include <QJsonDocument>
#include <QTimer>
#include <QDir>
#include <QStandardPaths>
#include <QCryptographicHash>
#include <cmath>
#include <algorithm>
#include <utility>
#ifdef Q_OS_WIN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <QCoreApplication>
#include <QEventLoop>

// The native TIP cannot link Qt; only this host exposes its safe nested event pump.
extern "C" __declspec(dllexport) void WINAPI ZtProcessImeEvents() {
    QCoreApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
}
#endif
#ifndef Q_OS_WIN
#include <sys/stat.h>
#include <unistd.h>
#include <QFile>
#endif

namespace {
constexpr qsizetype maximumFrame = 256 * 1024;
bool integer(const QJsonValue &value, double minimum, double maximum) {
    const double number = value.toDouble(std::nan(""));
    return std::isfinite(number) && number == std::floor(number) && number >= minimum && number <= maximum;
}
}

ImeHost::ImeHost(EngineSession &session, InputController &input, QObject *parent)
    : QObject(parent), session_(session), input_(input) {
    connect(&server_, &QLocalServer::newConnection, this, &ImeHost::accept);
    connect(&session_, &EngineSession::changed, this, &ImeHost::changed);
    connect(&session_, &EngineSession::committedText, this, &ImeHost::appendCommit);
    connect(&input_, &InputController::insertTextRequested, this, &ImeHost::appendCommit);
    connect(&input_, &InputController::candidateBoundaryReached, this, &ImeHost::boundaryReached);
    connect(&input_, &InputController::modeSwitched, this, [this](bool english) {
        if (active_ && !secure_) emit modeSwitched(english);
    });
}

ImeHost::~ImeHost() {
    // Socket destruction emits disconnected; stop callbacks before member state is destroyed.
    for (auto *peer : server_.findChildren<QLocalSocket *>()) {
        disconnect(peer, nullptr, this, nullptr);
        peer->abort();
    }
    server_.close();
}

bool ImeHost::reserve(const QString &endpoint) {
    if (!prepareImeEndpoint(endpoint, error_)) return false;
#ifdef Q_OS_WIN
    const auto lockDirectory = QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
    if (!QDir().mkpath(lockDirectory)) { error_ = QStringLiteral("無法建立服務鎖定目錄。"); return false; }
    const auto lockPath = QDir(lockDirectory).filePath(QString::fromLatin1(QCryptographicHash::hash(endpoint.toUtf8(), QCryptographicHash::Sha256).toHex()) + ".lock");
#else
    const auto lockPath = endpoint + ".lock";
#endif
    lock_ = std::make_unique<QLockFile>(lockPath);
    if (!lock_->tryLock()) { error_ = QStringLiteral("輸入法服務已在執行。"); return false; }
#ifndef Q_OS_WIN
    struct stat endpointStatus {};
    const auto endpointBytes = QFile::encodeName(endpoint);
    if (::lstat(endpointBytes.constData(), &endpointStatus) == 0
        && (!S_ISSOCK(endpointStatus.st_mode) || endpointStatus.st_uid != getuid())) {
        error_ = QStringLiteral("通訊端路徑已被其他檔案佔用。"); return false;
    }
#endif
    // UserAccessOption can replace an existing Unix path; inspect it before listen.
    QLocalSocket probe;
    probe.connectToServer(endpoint);
    if (probe.waitForConnected(150)) { error_ = QStringLiteral("輸入法服務已在執行。"); return false; }
    if (probe.error() != QLocalSocket::ConnectionRefusedError
        && probe.error() != QLocalSocket::ServerNotFoundError) { error_ = probe.errorString(); return false; }
    endpoint_ = endpoint;
    return true;
}

bool ImeHost::listen(const QString &endpoint) {
    if (endpoint_ != endpoint && !reserve(endpoint)) return false;
#ifndef Q_OS_WIN
    struct stat status {};
    if (::lstat(QFile::encodeName(endpoint).constData(), &status) == 0
        && (!S_ISSOCK(status.st_mode) || status.st_uid != getuid())) {
        error_ = QStringLiteral("通訊端路徑已被其他檔案佔用。"); return false;
    }
#endif
    server_.setSocketOptions(QLocalServer::UserAccessOption);
    QLocalServer::removeServer(endpoint);
    if (server_.listen(endpoint)) return true;
    error_ = server_.errorString();
    return false;
}

void ImeHost::accept() {
    while (auto *peer = server_.nextPendingConnection()) {
        peer->setReadBufferSize(maximumFrame + 1);
        connect(peer, &QLocalSocket::disconnected, this, [this, peer] {
            if (active_ == peer) discard();
            peer->deleteLater();
        });
        connect(peer, &QLocalSocket::readyRead, this, [this, peer] {
            while (peer->canReadLine()) {
                const auto bytes = peer->readLine(maximumFrame + 1);
                QJsonParseError parse;
                const auto document = QJsonDocument::fromJson(bytes, &parse);
                if (bytes.size() > maximumFrame || parse.error != QJsonParseError::NoError || !document.isObject()) {
                    peer->abort(); return;
                }
                const auto request = document.object();
                if (!integer(request.value("id"), 1, 9007199254740991.0)
                    || request.value("id").toDouble() <= peer->property("lastRequest").toDouble()) {
                    peer->abort(); return;
                }
                peer->setProperty("lastRequest", request.value("id").toDouble());
                auto response = dispatch(peer, request);
                response.insert("v", 1); response.insert("id", request.value("id"));
                response.insert("context", request.value("context"));
                const auto output = QJsonDocument(response).toJson(QJsonDocument::Compact) + '\n';
                if (output.size() > maximumFrame || peer->bytesToWrite() > maximumFrame
                    || peer->write(output) != output.size()) { peer->abort(); return; }
            }
            if (peer->bytesAvailable() >= maximumFrame) peer->abort();
        });
    }
}

void ImeHost::appendCommit(const QString &text) {
    if (active_ && !secure_) commit_ += text;
}

void ImeHost::discard() {
    active_.clear(); context_.clear(); commit_.clear(); clientApp_.clear();
    input_.setSecure(true);
    secure_ = true;
    emit changed();
}

void ImeHost::secure(bool value) {
    if (value || value != secure_) {
        if (value) commit_.clear();
        input_.setSecure(value);
    }
    secure_ = value;
}

QJsonObject ImeHost::snapshot(bool handled) {
    const QString commit = std::exchange(commit_, QString());
    return {{"ok", true}, {"handled", handled}, {"commit", secure_ ? QString() : commit},
            // A listed app shows nothing itself: the composition is in the candidate window.
            {"preedit", secure_ || popupApps_.contains(clientApp_, Qt::CaseInsensitive) ? QString() : session_.composition()},
            {"cursor", secure_ ? 0 : session_.cursor()}, {"candidates", visible()},
            {"english", input_.english()}};
}

QJsonObject ImeHost::dispatch(QLocalSocket *peer, const QJsonObject &r) {
    const auto failure = [](const QString &message) { return QJsonObject{{"ok", false}, {"error", message}, {"handled", false}}; };
    const auto context = r.value("context").toString();
    const auto op = r.value("op").toString();
    if (r.value("v").toInt() != 1 || context.isEmpty() || context.size() > 128)
        return failure(QStringLiteral("通訊格式無效。"));
    if (op == "show-settings") { emit showSettingsRequested(); return {{"ok", true}}; }
    if (!QStringList{"focus", "key", "poll", "blur", "reset", "secure", "context"}.contains(op))
        return failure(QStringLiteral("不支援的操作。"));
    if ((op == "key" || op == "focus" || op == "secure") && !r.value("secure").isBool())
        return failure(QStringLiteral("缺少安全輸入狀態。"));
    for (const auto *field : {"secure", "repeat", "capsLock", "inlinePreedit"})
        if (r.contains(field) && !r.value(field).isBool())
            return failure(QStringLiteral("輸入狀態格式無效。"));
    if (op == "key" && (!integer(r.value("key"), 0, 0x1ffffff)
        || !integer(r.value("modifiers"), 0, 0x7fffffff)
        || !r.value("release").isBool()
        || (r.contains("shiftSide") && !integer(r.value("shiftSide"), 0, 2))))
        return failure(QStringLiteral("按鍵格式無效。"));
    // 前文: the app and the text before the insertion point, when the platform reports them (IPC.md §前文).
    if ((r.contains("app") && (!r.value("app").isString() || r.value("app").toString().size() > 256))
        || (r.contains("before") && (!r.value("before").isString() || r.value("before").toString().size() > 1024)))
        return failure(QStringLiteral("前文格式無效。"));
    QRectF rect = anchor_;
    if (r.contains("rect")) {
        const auto a = r.value("rect").toArray();
        const double scale = r.value("scale").toDouble(1);
        if (a.size() != 4 || (r.contains("scale") && !r.value("scale").isDouble())
            || !std::isfinite(scale) || scale < 0.25 || scale > 8)
            return failure(QStringLiteral("游標位置無效。"));
        for (const auto &v : a) if (!v.isDouble() || !std::isfinite(v.toDouble()) || std::abs(v.toDouble()) > 1000000)
            return failure(QStringLiteral("游標位置無效。"));
        if (a[2].toDouble() < 0 || a[3].toDouble() < 0) return failure(QStringLiteral("游標範圍無效。"));
        rect = QRectF(a[0].toDouble() / scale, a[1].toDouble() / scale, a[2].toDouble() / scale, a[3].toDouble() / scale);
    }
    if (!session_.ready()) return failure(QStringLiteral("輸入引擎尚未就緒。"));
    if (op == "focus") {
        if (active_ != peer || context_ != context) {
            discard(); active_ = peer; context_ = context;
        }
    } else if (active_ != peer || context_ != context) return failure(QStringLiteral("輸入欄位已失去焦點。"));
    const bool moved = anchor_ != rect;
    anchor_ = rect;
    if (op == "focus") inlinePreedit_ = r.value("inlinePreedit").toBool(true);
    if (r.contains("secure")) secure(r.value("secure").toBool(true));
    if (r.value("capsLock").isBool()) input_.synchronizeCapsLock(r.value("capsLock").toBool());
    if (!secure_ && (op == "focus" || r.contains("app") || r.contains("before"))) {
        if (op == "focus" || r.contains("app")) clientApp_ = r.value("app").toString();
        input_.setClient(clientApp_, r.contains("before") ? std::optional<QString>(r.value("before").toString()) : std::nullopt);
    }
    bool handled = false;
    if (op == "key" && !secure_) {
        const int key = r.value("key").toInt(), modifiers = r.value("modifiers").toInt();
        const int side = r.value("shiftSide").toInt();
        if (r.value("release").toBool()) input_.release(key, modifiers, side);
        else handled = input_.press(key, modifiers, r.value("repeat").toBool(), side);
    } else if (op == "reset") {
        commit_.clear(); session_.reset();
    } else if (op == "blur") input_.deactivate();
    auto response = snapshot(handled);
    if (op == "blur") { active_.clear(); context_.clear(); session_.flush(); }
    if (moved || op != "poll") emit changed();
    return response;
}

void ImeHost::setPopupCompositionApps(const QStringList &apps) {
    if (apps == popupApps_) return;
    popupApps_ = apps;
    emit changed();
}

void ImeHost::pick(int row) {
    if (visible() && row >= 0 && row < rows().size()) input_.choose(page() * pageSize() + row);
}
void ImeHost::turnPage(int delta) {
    if (!visible() || (delta != -1 && delta != 1)) return;
    const int oldPage = page();
    input_.feed(delta > 0 ? Qt::Key_PageDown : Qt::Key_PageUp);
    if (oldPage == page()) emit boundaryReached(delta);
}
void ImeHost::moveSelection(int delta) {
    if (visible() && (delta == -1 || delta == 1)) input_.feed(delta > 0 ? Qt::Key_Down : Qt::Key_Up);
}
void ImeHost::toggleSingle() { if (visible()) input_.feed(Qt::Key_Down, Qt::ShiftModifier); }
void ImeHost::toggleCorrections() { if (visible()) input_.toggleCorrections(); }
void ImeHost::typeReading() { if (visible() && !session_.reading().isEmpty()) input_.feed(Qt::Key_QuoteLeft); }

QPointF ImeHost::popupPosition(qreal width, qreal height, bool above, qreal textInset) const {
    constexpr qreal gap = 4;
    auto *screen = QGuiApplication::screenAt(anchor_.center().toPoint());
    if (!screen) screen = QGuiApplication::primaryScreen();
    if (!screen) return anchor_.bottomLeft();
    const auto bounds = screen->availableGeometry();
    const qreal x = std::clamp(anchor_.left() - textInset, qreal(bounds.left()),
                               std::max(qreal(bounds.left()), bounds.right() - width + 1));
    const qreal below = anchor_.bottom() + gap, over = anchor_.top() - height - gap;
    const bool fitsBelow = below + height <= bounds.bottom() + 1, fitsAbove = over >= bounds.top();
    qreal y = above ? (fitsAbove || !fitsBelow ? over : below) : (fitsBelow || !fitsAbove ? below : over);
    y = std::clamp(y, qreal(bounds.top()), std::max(qreal(bounds.top()), bounds.bottom() - height + 1));
    return {x, y};
}
