// Focus moving away mid-composition (delivery.sh, ZT_DELIVERY_FOCUS=1): the composition reaches the field it was typed
// in exactly once, and nothing of it reaches the field that gets focus.  Two moves: to another field of the same
// window (Qt keeps one input context per window) and to another window.
#include <QApplication>
#include <QEventLoop>
#include <QInputMethodEvent>
#include <QLineEdit>
#include <QProcess>
#include <QSignalBlocker>
#include <QTimer>
#include <QVBoxLayout>
#include <QWidget>
#include <iostream>

namespace {
class Field final : public QLineEdit {
public:
    QString composition;
    int commits = 0;
protected:
    void inputMethodEvent(QInputMethodEvent *event) override {
        composition = event->preeditString();
        if (!event->commitString().isEmpty()) ++commits;
        QLineEdit::inputMethodEvent(event);
    }
};

bool run(const QString &program, const QStringList &args) {
    QProcess process;
    QEventLoop loop;
    QObject::connect(&process, &QProcess::finished, &loop, &QEventLoop::quit);
    QObject::connect(&process, &QProcess::errorOccurred, &loop, &QEventLoop::quit);
    process.start(program, args);
    loop.exec();
    return process.error() == QProcess::UnknownError && process.exitCode() == 0;
}
}

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QWidget window;
    window.setWindowTitle("ZhuyinType focus probe");
    auto *layout = new QVBoxLayout(&window);
    Field first, second, other;
    layout->addWidget(&first);
    layout->addWidget(&second);
    window.resize(500, 120);
    window.move(200, 160);
    window.show();
    other.setWindowTitle("ZhuyinType focus probe: other window");
    other.resize(500, 60);
    other.move(200, 400);
    other.show();
    window.activateWindow();
    first.setFocus();
    const auto type = [](const QString &text) {
        return run("xdotool", {"type", "--clearmodifiers", "--delay", "50", text});
    };
    int phase = 0, settle = 0;
    const auto report = [&](const char *what) {
        std::cerr << "FAIL " << what << " at phase " << phase << ": first=\"" << first.text().toStdString() << "\" ("
                  << first.commits << " commits) second=\"" << second.text().toStdString() << "\" (" << second.commits
                  << " commits) other=\"" << other.text().toStdString() << "\" (" << other.commits << " commits)\n";
    };
    QTimer tick;
    tick.setInterval(100);
    QObject::connect(&tick, &QTimer::timeout, &app, [&] {
        const QSignalBlocker guard(&tick);
        switch (phase) {
        case 0:
            if (!run("xdotool", {"windowfocus", "--sync", QString::number(window.winId())}) ||
                !run("fcitx5-remote", {"-s", "zhuyintype"}) || !run("fcitx5-remote", {"-o"}) || !type("su3cl3")) {
                report("xdotool or fcitx5-remote"); app.exit(2); return;
            }
            ++phase;
            break;
        case 1:
            if (first.composition != QStringLiteral("你好")) return;
            second.setFocus(); // Same window: Qt sends Fcitx a focus out and a focus in on the window's context.
            ++phase; settle = 0;
            break;
        case 2:
            // Late commits arrive over D-Bus: give them time before judging.
            if (++settle < 10) return;
            if (first.text() != QStringLiteral("你好") || first.commits != 1 || !second.text().isEmpty()) {
                report("moving to another field"); app.exit(3); return;
            }
            if (!type("su3")) { report("xdotool"); app.exit(2); return; }
            ++phase;
            break;
        case 3:
            if (second.composition != QStringLiteral("你")) return;
            other.activateWindow();
            // A window's first focus starts in the group's first input method (Fcitx shares no state by default).
            if (!run("xdotool", {"windowfocus", "--sync", QString::number(other.winId())})) {
                report("xdotool"); app.exit(2); return;
            }
            ++phase; settle = 0;
            break;
        case 4:
            if (++settle < 10) return;
            if (second.text() != QStringLiteral("你") || second.commits != 1 || !other.text().isEmpty() ||
                first.text() != QStringLiteral("你好")) {
                report("moving to another window"); app.exit(3); return;
            }
            // The host is ready for the next field.
            if (!run("fcitx5-remote", {"-o"}) || !type("cl3")) { report("xdotool or fcitx5-remote"); app.exit(2); return; }
            ++phase;
            break;
        case 5:
            if (other.composition != QStringLiteral("好")) return;
            if (!run("xdotool", {"key", "--clearmodifiers", "Return"})) { report("xdotool"); app.exit(2); return; }
            ++phase;
            break;
        case 6:
            if (other.text() != QStringLiteral("好") || !other.composition.isEmpty()) return;
            std::cout << "PASS focus moves mid-composition: the composition reaches its own field once, in the same "
                         "window and across windows\n";
            app.exit(0);
            break;
        }
    });
    QTimer::singleShot(20000, &app, [&] { report("timed out"); app.exit(1); });
    QTimer::singleShot(500, &tick, qOverload<>(&QTimer::start));
    return app.exec();
}
