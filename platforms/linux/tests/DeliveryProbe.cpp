#include <QApplication>
#include <QInputMethodEvent>
#include <QEventLoop>
#include <QLineEdit>
#include <QProcess>
#include <QSignalBlocker>
#include <QTimer>
#include <QScreen>
#include <QDir>
#include <QRegularExpression>
#include <iostream>

class Field final : public QLineEdit {
public:
    QString composition;
    int commits = 0;
    int focusTransitions = 0;
protected:
    void focusInEvent(QFocusEvent *event) override { ++focusTransitions; QLineEdit::focusInEvent(event); }
    void focusOutEvent(QFocusEvent *event) override { ++focusTransitions; QLineEdit::focusOutEvent(event); }
    void inputMethodEvent(QInputMethodEvent *event) override {
        composition = event->preeditString();
        if (!event->commitString().isEmpty()) ++commits;
        QLineEdit::inputMethodEvent(event);
    }
};

bool runCommand(const QString &program, const QStringList &args, QByteArray &output) {
        QProcess process;
        QEventLoop loop;
        QObject::connect(&process, &QProcess::finished, &loop, &QEventLoop::quit);
        QObject::connect(&process, &QProcess::errorOccurred, &loop, &QEventLoop::quit);
        process.start(program, args);
        loop.exec();
        output = process.readAllStandardOutput();
        return process.error() == QProcess::UnknownError && process.exitCode() == 0;
}

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    Field field;
    field.setWindowTitle("ZhuyinType delivery probe");
    field.resize(500, 80);
    field.move(200, 160);
    field.show();
    field.activateWindow();
    field.setFocus();
    QByteArray output;
    const auto run = [&output](const QString &program, const QStringList &args) { return runCommand(program, args, output); };
    const auto type = [&](const QString &text) { return run("xdotool", {"type", "--clearmodifiers", "--delay", "50", text}); };
    const auto key = [&](const QString &name) { return run("xdotool", {"key", "--clearmodifiers", name}); };
    int phase = 0, commitsBeforePassword = 0;
    QString chosenText = QStringLiteral("妳"), knownBeforeOutage;
    int commitsBeforeOutage = 0;
    const auto capture = [&](const QString &name) {
        const auto directory = qEnvironmentVariable("ZT_DELIVERY_CAPTURE_DIR");
        if (directory.isEmpty()) return;
        QDir().mkpath(directory);
        QGuiApplication::primaryScreen()->grabWindow(0).save(QDir(directory).filePath(name + ".png"));
    };
    const auto clickCandidate = [&] {
        if (!run("xdotool", {"search", "--onlyvisible", "--pid", qEnvironmentVariable("ZT_DELIVERY_HOST_PID")})) return false;
        const auto windows = QString::fromUtf8(output).trimmed().split('\n');
        for (const auto &window : windows) {
            if (!run("xdotool", {"getwindowgeometry", "--shell", window})) continue;
            const auto geometry = QString::fromUtf8(output);
            const auto number = [&](const QString &name) {
                return QRegularExpression("(?:^|\n)" + name + "=(-?[0-9]+)").match(geometry).captured(1).toInt();
            };
            int x = number("X"), y = number("Y");
            const int w = number("WIDTH"), h = number("HEIGHT");
            if (!run("xwininfo", {"-id", window})) return false;
            const auto info = QString::fromUtf8(output);
            const auto absoluteX = QRegularExpression("Absolute upper-left X:\\s+(-?[0-9]+)").match(info);
            const auto absoluteY = QRegularExpression("Absolute upper-left Y:\\s+(-?[0-9]+)").match(info);
            if (!absoluteX.hasMatch() || !absoluteY.hasMatch()) return false;
            x = absoluteX.captured(1).toInt(); y = absoluteY.captured(1).toInt();
            if (w < 100 || h < 50) continue;
            const auto caret = field.mapToGlobal(field.inputMethodQuery(Qt::ImCursorRectangle).toRect().bottomLeft());
            if (std::abs(x - caret.x()) > 40 || std::abs(y - caret.y()) > h + 40) {
                std::cerr << "FAIL popup anchor: window=" << x << ',' << y << " caret=" << caret.x() << ',' << caret.y() << '\n';
                return false;
            }
            std::cout << "Qt popup visible at " << x << ',' << y << " size " << w << 'x' << h
                << "; native caret " << caret.x() << ',' << caret.y() << '\n';
            if (!run("xdotool", {"sleep", "0.3"})) return false;
            std::cout << "Before pick: focus=" << field.hasFocus() << " composition units=" << field.composition.size()
                << " focus transitions=" << field.focusTransitions << '\n';
            capture("candidate-window");
            int offset = qEnvironmentVariableIntValue("ZT_PICK_Y_OFFSET");
            if (!offset) offset = h * 65 / 100; // Second row of the two-candidate fixture, above the footer.
            return run("xdotool", {"mousemove", QString::number(x + 70), QString::number(y + offset), "click", "1"});
        }
        return false;
    };
    QTimer tick;
    tick.setInterval(100);
    QObject::connect(&tick, &QTimer::timeout, &app, [&] {
        const QSignalBlocker guard(&tick);
        switch (phase) {
        case 0:
            if (!run("xdotool", {"windowfocus", "--sync", QString::number(field.winId())}) ||
                !run("fcitx5-remote", {"-s", "zhuyintype"}) || !run("fcitx5-remote", {"-o"})) { app.exit(2); return; }
            ++phase;
            break;
        case 1: if (!type("su3cl3")) { app.exit(2); return; } ++phase; break;
        case 2:
            if (field.composition != QStringLiteral("你好")) return;
            if (!key("Return")) { app.exit(2); return; }
            ++phase;
            break;
        case 3:
            if (field.text() != QStringLiteral("你好") || !field.composition.isEmpty()) return;
            field.clear();
            phase = 30;
            break;
        case 30:
            if (!type("su3") || !key("Down")) { app.exit(2); return; }
            phase = 4;
            break;
        case 4:
            if (field.composition != QStringLiteral("你")) return;
            if (!clickCandidate()) { app.exit(4); return; }
            ++phase;
            break;
        case 5:
            if (qEnvironmentVariableIsSet("ZT_DELIVERY_REAL_DATA")) {
                if (field.composition.isEmpty() || field.composition == QStringLiteral("你")) return;
                chosenText = field.composition;
            } else if (field.composition != chosenText) return;
            if (!field.hasFocus()) { std::cerr << "FAIL popup stole focus\n"; app.exit(4); return; }
            if (!key("Return")) { app.exit(2); return; }
            phase = 50;
            break;
        case 50:
            if (field.text() != chosenText || !field.composition.isEmpty()) return;
            capture("mouse-pick-delivered");
            field.clear();
            phase = 51;
            break;
        case 51:
            if (!type("su3")) { app.exit(2); return; }
            phase = 6;
            break;
        case 6:
            // The previous candidate pick may change the learned default.
            if (field.composition.isEmpty()) return;
            commitsBeforePassword = field.commits;
            field.setInputMethodHints(Qt::ImhHiddenText | Qt::ImhSensitiveData | Qt::ImhNoPredictiveText);
            field.setEchoMode(QLineEdit::Password);
            ++phase;
            break;
        case 7:
            if (field.commits != commitsBeforePassword) { std::cerr << "FAIL secure transition committed stale text\n"; app.exit(3); return; }
            if (!field.text().isEmpty()) { app.exit(3); return; }
            if (!type("su3")) { app.exit(2); return; }
            ++phase;
            break;
        case 8:
            if (field.text() != "su3") return;
            if (field.commits != commitsBeforePassword || !field.composition.isEmpty()) { app.exit(3); return; }
            capture("secure-pass-through");
            if (qEnvironmentVariableIsSet("ZT_DELIVERY_PRESERVE_HOST")) {
                std::cout << "PASS installed desktop service: real data, application commit, shared Qt popup, mouse pick and secure field\n";
                app.exit(0);
                return;
            }
            field.clear();
            field.setEchoMode(QLineEdit::Normal);
            field.setInputMethodHints(Qt::ImhNone);
            ++phase;
            break;
        case 9:
            if (!type("su3cl3")) { app.exit(2); return; }
            phase = 90;
            break;
        case 90:
            if (field.composition.isEmpty()) return;
            knownBeforeOutage = field.composition;
            commitsBeforeOutage = field.commits;
            if (!run("kill", {"-TERM", qEnvironmentVariable("ZT_DELIVERY_HOST_PID")})) { app.exit(2); return; }
            phase = 91;
            break;
        case 91:
            if (field.text() != knownBeforeOutage || !field.composition.isEmpty()) return;
            if (field.commits != commitsBeforeOutage + 1) { app.exit(3); return; }
            if (!type("abc123")) { app.exit(2); return; }
            phase = 10;
            break;
        case 10:
            if (field.text() != knownBeforeOutage + "abc123" || !field.composition.isEmpty()) return;
            if (field.commits != commitsBeforeOutage + 1) { app.exit(3); return; }
            std::cout << "PASS real Qt application: composition, commit, visible caret-anchored shared Qt popup, mouse pick via poll, password, outage composition preservation once and unavailable-host pass-through\n";
            app.exit(0);
            break;
        }
    });
    QTimer::singleShot(20000, &app, [&] {
        capture("failure");
        std::cerr << "FAIL application delivery timed out at phase " << phase
                  << " (focus=" << field.hasFocus() << ", preedit units=" << field.composition.size()
                  << ", delivered units=" << field.text().size() << ", commits=" << field.commits
                  << ", focus transitions=" << field.focusTransitions
                  << ", focus object=" << (QGuiApplication::focusObject() == &field)
                  << ", IM enabled=" << field.inputMethodQuery(Qt::ImEnabled).toBool() << ")\n";
        app.exit(1);
    });
    QTimer::singleShot(500, &tick, qOverload<>(&QTimer::start));
    return app.exec();
}
