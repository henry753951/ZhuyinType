#include <QApplication>
#include <QClipboard>
#include <QDir>
#include <QElapsedTimer>
#include <QProcess>
#include <QRegularExpression>
#include <QScreen>
#include <iostream>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    if (argc != 2) return 2;
    QByteArray output;
    const auto run = [&](const QString &program, const QStringList &args) {
        QProcess process;
        process.start(program,args);
        if (!process.waitForFinished(5000) || process.exitCode() != 0) return false;
        output=process.readAllStandardOutput();
        app.processEvents();
        return true;
    };
    const auto capture = [&](const QString &name) {
        const auto directory=qEnvironmentVariable("ZT_DELIVERY_CAPTURE_DIR");
        if (directory.isEmpty()) return;
        QDir().mkpath(directory);
        app.primaryScreen()->grabWindow(0).save(QDir(directory).filePath(name+".png"));
    };
    bool visible=false;
    for (int attempt=0;attempt<100;++attempt) {
        if (run("xdotool", {"search","--onlyvisible","--pid",argv[1]})) { visible=true; break; }
        run("xdotool",{"sleep","0.05"});
    }
    if (!visible) { std::cerr << "FAIL self-host settings window unavailable\n"; return 2; }
    const auto window=QString::fromUtf8(output).trimmed().split('\n').first();
    if (!run("xdotool",{"windowraise",window,"windowfocus","--sync",window,"sleep","0.3"})) return 2;
    if (!run("xdotool",{"getwindowgeometry","--shell",window})) return 2;
    const auto geometry=QString::fromUtf8(output);
    const auto number=[&](const QString &name) {
        return QRegularExpression("(?:^|\n)"+name+"=(-?[0-9]+)").match(geometry).captured(1).toInt();
    };
    int x=number("X"), y=number("Y");
    std::cout << "Settings geometry: " << x << ',' << y << '\n';
    if (!run("xwininfo",{"-id",window})) return 2;
    const auto info=QString::fromUtf8(output);
    const auto absoluteX=QRegularExpression("Absolute upper-left X:\\s+(-?[0-9]+)").match(info);
    const auto absoluteY=QRegularExpression("Absolute upper-left Y:\\s+(-?[0-9]+)").match(info);
    if (!absoluteX.hasMatch() || !absoluteY.hasMatch()) return 2;
    x=absoluteX.captured(1).toInt(); y=absoluteY.captured(1).toInt();
    std::cout << "Settings absolute geometry: " << x << ',' << y << '\n';
    capture("self-host-start");
    if (!run("xdotool",{"windowfocus","--sync",window})) return 2;
    int navigationY=qEnvironmentVariableIntValue("ZT_SELFHOST_NAV_Y");
    if (!navigationY) navigationY=443;
    if (!run("xdotool",{"mousemove",QString::number(x+80),QString::number(y+navigationY),"click","1","sleep","0.5"})) return 2;
    capture("self-host-search");
    int searchY=qEnvironmentVariableIntValue("ZT_SELFHOST_SEARCH_Y");
    if (!searchY) searchY=100;
    if (!run("xdotool",{"mousemove",QString::number(x+350),QString::number(y+searchY),"click","1"}) ||
        !run("fcitx5-remote",{"-s","zhuyintype"}) || !run("fcitx5-remote",{"-o"})) return 2;
    app.clipboard()->setText("zt-self-host-probe-unset");
    app.processEvents();
    QElapsedTimer elapsed; elapsed.start();
    if (!run("xdotool",{"type","--clearmodifiers","--delay","5","su3cl3"}) ||
        !run("xdotool",{"key","--clearmodifiers","Return","sleep","0.2","key","ctrl+a","ctrl+c","sleep","0.1"})) return 2;
    const auto delivered=app.clipboard()->text();
    capture("self-host-delivered");
    if (delivered != QStringLiteral("你好")) {
        std::cerr << "FAIL self-host settings TextField delivery: clipboard UTF-16 units=" << delivered.size() << '\n';
        return 1;
    }
    std::cout << "PASS self-host settings TextField: Qt Fcitx frontend -> Fcitx addon -> same Qt host; exact Chinese commit; pipeline "
        << elapsed.elapsed() << " ms including 300 ms test waits and 5 ms key spacing\n";
    return 0;
}
