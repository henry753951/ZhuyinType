#pragma once
#include <QObject>
#include <QTimer>
#include <QQuickWindow>
#include "Settings.h"

// Captures each registered page with isolated settings. All state belongs to this run. Like the macOS renderer, it
// also checks every settings row: the trailing block centred on the leading block, and ending on the row's right edge.
class PreviewCapture final : public QObject {
    Q_OBJECT
public:
    PreviewCapture(QQuickWindow &window, Settings &settings, QString directory, QObject *parent = nullptr);
    bool start();
private:
    struct Frame { QString page; QString appearance; QString layout; int width; QString name; };
    void prepare();
    void capture();
    void checkRows(const QString &frame);
    QQuickWindow &window_;
    Settings &settings_;
    QString directory_;
    QList<Frame> frames_;
    qsizetype index_ = 0;
    QTimer timer_;
    QStringList misaligned_;
};
