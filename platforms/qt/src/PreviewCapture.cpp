#include "PreviewCapture.h"
#include <QCoreApplication>
#include <QDir>
#include <QImage>
#include <QJSValue>
#include <QQuickItem>
#include <QTextStream>
#include <cmath>

PreviewCapture::PreviewCapture(QQuickWindow &window, Settings &settings, QString directory, QObject *parent)
    : QObject(parent), window_(window), settings_(settings), directory_(std::move(directory)) {
    const auto pages = window.property("renderPages").value<QJSValue>().toVariant().toStringList();
    for (const auto &appearance : {QString("light"), QString("dark")}) {
        for (const auto &page : pages) {
            frames_.append({page, appearance, "vertical", 900, page + "-" + appearance});
            frames_.append({page, appearance, "vertical", 640, page + "-narrow-" + appearance});
        }
        frames_.append({"appearance", appearance, "horizontal", 900, "horizontal-" + appearance});
        frames_.append({"appearance", appearance, "horizontal", 640, "narrow-" + appearance});
    }
    // Long pages (the licence, the Markdown sources) need more than one frame to lay out before they are grabbed.
    timer_.setInterval(450);
    connect(&timer_, &QTimer::timeout, this, &PreviewCapture::capture);
}

bool PreviewCapture::start() {
    if (frames_.isEmpty() || !QDir().mkpath(directory_)) return false;
    prepare();
    timer_.start();
    return true;
}

void PreviewCapture::prepare() {
    const auto &frame = frames_.at(index_);
    settings_.setValue("panelAppearance", frame.appearance);
    settings_.setValue("candidateLayout", frame.layout);
    window_.setWidth(frame.width);
    window_.setProperty("pageId", frame.page);
}

void PreviewCapture::capture() {
    const auto &frame = frames_.at(index_);
    const auto image = window_.grabWindow();
    if (image.isNull() || !image.save(QDir(directory_).filePath(frame.name + ".png"))) {
        timer_.stop();
        QCoreApplication::exit(1);
        return;
    }
    checkRows(frame.name);
    if (++index_ == frames_.size()) {
        timer_.stop();
        if (!misaligned_.isEmpty()) {
            QTextStream(stderr) << "Misaligned settings rows:\n" << misaligned_.join('\n') << '\n';
            QCoreApplication::exit(1);
            return;
        }
        QCoreApplication::quit();
        return;
    }
    prepare();
}

void PreviewCapture::checkRows(const QString &frame) {
    QList<QQuickItem *> pending{window_.contentItem()};
    while (!pending.isEmpty()) {
        auto *item = pending.takeLast();
        if (!item->isVisible() || item->width() <= 0) continue;
        pending.append(item->childItems());
        if (!item->property("isSettingsRow").toBool() || item->property("stacked").toBool()) continue;
        auto *leading = item->property("leadingItem").value<QQuickItem *>();
        auto *trailing = item->property("trailingItem").value<QQuickItem *>();
        if (!leading || !trailing || !trailing->isVisible() || trailing->width() <= 0) continue;
        const auto lead = leading->mapRectToItem(item, leading->boundingRect());
        const auto trail = trailing->mapRectToItem(item, trailing->boundingRect());
        const auto title = item->property("title").toString();
        if (std::abs(lead.center().y() - trail.center().y()) > 1.0)
            misaligned_.append(QStringLiteral("%1: 「%2」 is not centred (%3 vs %4)").arg(frame, title).arg(lead.center().y()).arg(trail.center().y()));
        if (std::abs(trail.right() - item->width()) > 1.0)
            misaligned_.append(QStringLiteral("%1: 「%2」 ends %3 px before the right edge").arg(frame, title).arg(item->width() - trail.right()));
    }
}
