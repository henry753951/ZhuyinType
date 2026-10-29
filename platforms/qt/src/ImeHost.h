#pragma once
#include "EngineSession.h"
#include "InputController.h"
#include <QLocalServer>
#include <QLocalSocket>
#include <QPointer>
#include <QJsonObject>
#include <QRectF>
#include <QLockFile>
#include <memory>

// One focused OS context owns the shared composer. Every reply consumes its commits once.
// Adapters blur before changing focus; stale contexts cannot operate or receive another's text.
class ImeHost final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantList rows READ rows NOTIFY changed)
    Q_PROPERTY(QString reading READ reading NOTIFY changed)
    Q_PROPERTY(QString composition READ composition NOTIFY changed)
    Q_PROPERTY(int page READ page NOTIFY changed)
    Q_PROPERTY(int pageCount READ pageCount NOTIFY changed)
    Q_PROPERTY(int pageSize READ pageSize NOTIFY changed)
    Q_PROPERTY(int selected READ selected NOTIFY changed)
    Q_PROPERTY(bool single READ single NOTIFY changed)
    Q_PROPERTY(bool corrections READ corrections NOTIFY changed)
    Q_PROPERTY(int tolerant READ tolerant NOTIFY changed)
    Q_PROPERTY(QString notice READ notice NOTIFY changed)
    Q_PROPERTY(bool visible READ visible NOTIFY changed)
    Q_PROPERTY(bool inlinePreedit READ inlinePreedit NOTIFY changed)
    Q_PROPERTY(QRectF anchor READ anchor NOTIFY changed)
public:
    ImeHost(EngineSession &session, InputController &input, QObject *parent = nullptr);
    ~ImeHost() override;
    bool reserve(const QString &endpoint);
    bool listen(const QString &endpoint);
    QString error() const { return error_; }
    // As macOS ControllerCore.render: the window shows a list, a notice, or the composition for apps that cannot.
    bool visible() const {
        return active_ && !secure_ && session_.composing()
            && (!session_.rows().isEmpty() || !session_.notice().isEmpty() || !inlinePreedit());
    }
    /// The app shows the composition itself: not when it cannot, or when it is on the 在候選字視窗顯示組字 list.
    bool inlinePreedit() const { return inlinePreedit_ && !popupApps_.contains(clientApp_, Qt::CaseInsensitive); }
    void setPopupCompositionApps(const QStringList &apps);
    QVariantList rows() const { return session_.rows().mid(page() * pageSize(), pageSize()); }
    QString reading() const { return session_.reading(); }
    QString composition() const { return session_.composition(); }
    int page() const { return session_.page(); }
    int pageCount() const { return session_.pageCount(); }
    int pageSize() const { return session_.pageSize(); }
    int selected() const { return session_.selected() < 0 ? -1 : session_.selected() % pageSize(); }
    bool single() const { return session_.single(); }
    bool corrections() const { return session_.corrections(); }
    int tolerant() const { return session_.tolerant(); }
    QString notice() const { return session_.notice(); }
    QRectF anchor() const { return anchor_; }
    Q_INVOKABLE void pick(int row);
    Q_INVOKABLE void turnPage(int delta);
    Q_INVOKABLE void moveSelection(int delta);
    Q_INVOKABLE void toggleSingle();
    Q_INVOKABLE void toggleCorrections();
    Q_INVOKABLE void typeReading();
    // Below the caret's line (or above it, `above`), the candidate text lined up with the composition (`textInset`);
    // on the other side when the screen has no room, always inside the visible area.  As macOS InputPanelPlacement.
    Q_INVOKABLE QPointF popupPosition(qreal width, qreal height, bool above = false, qreal textInset = 0) const;
signals:
    void changed();
    void boundaryReached(int direction);
    void showSettingsRequested();
    /// The user switched 中/英 in the focused field: the popup shows the mode toast.
    void modeSwitched(bool english);
private:
    void accept();
    QJsonObject dispatch(QLocalSocket *peer, const QJsonObject &request);
    QJsonObject snapshot(bool handled);
    void discard();
    void secure(bool value);
    void appendCommit(const QString &text);
    EngineSession &session_;
    InputController &input_;
    QLocalServer server_;
    std::unique_ptr<QLockFile> lock_;
    QPointer<QLocalSocket> active_;
    QString context_, commit_, error_, endpoint_, clientApp_;
    QStringList popupApps_;
    QRectF anchor_;
    bool secure_ = false, inlinePreedit_ = true;
};
