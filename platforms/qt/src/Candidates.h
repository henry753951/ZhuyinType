#pragma once

#include <QObject>
#include <QVariantList>

// Presentation state only. The host owns conversion and handles the emitted intents.
class Candidates final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantList rows READ rows NOTIFY changed)
    Q_PROPERTY(QString reading READ reading NOTIFY changed)
    Q_PROPERTY(int page READ page NOTIFY changed)
    Q_PROPERTY(int pageCount READ pageCount NOTIFY changed)
    Q_PROPERTY(int selected READ selected NOTIFY changed)
    Q_PROPERTY(int pageSize READ pageSize WRITE setPageSize NOTIFY changed)
    Q_PROPERTY(bool single READ single NOTIFY changed)
    Q_PROPERTY(bool corrections READ corrections NOTIFY changed)
    Q_PROPERTY(quint64 revision READ revision NOTIFY changed)
public:
    explicit Candidates(QObject *parent = nullptr) : QObject(parent) {}
    void present(QVariantList entries, QString reading, int selected = 0);
    QVariantList rows() const;
    QString reading() const { return reading_; }
    int page() const { return page_; }
    int pageCount() const;
    int selected() const { return selected_; }
    int pageSize() const { return pageSize_; }
    bool single() const { return single_; }
    bool corrections() const { return corrections_; }
    quint64 revision() const { return revision_; }
    void setPageSize(int size);
    Q_INVOKABLE void turnPage(int delta);
    Q_INVOKABLE void moveSelection(int delta);
    Q_INVOKABLE void pick(int row);
    Q_INVOKABLE void toggleSingle();
    Q_INVOKABLE void toggleCorrections();
signals:
    void changed();
    void picked(int index);
    void filterRequested(bool single, bool corrections);
    void boundaryReached(int direction);
private:
    QVariantList entries_;
    QString reading_;
    int page_ = 0;
    int selected_ = -1;
    int pageSize_ = 9;
    bool single_ = false;
    bool corrections_ = false;
    // A different list/style starts a new sizing and animation session.
    quint64 revision_ = 0;
};
