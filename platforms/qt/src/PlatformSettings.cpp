#include "PlatformSettings.h"
#include <QCoreApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QLocale>
#include <QStandardPaths>
#include <QTextStream>
#include <algorithm>
#ifdef Q_OS_WIN
#define NOMINMAX
#include <windows.h>
#endif

namespace {
QString appId(const QString &id) {
    const auto trimmed = id.trimmed();
    if (trimmed.isEmpty() || trimmed.size() > 256) return {};
    for (const auto character : id) {
        const auto category = character.category();
        if (category == QChar::Other_Control || category == QChar::Separator_Line || category == QChar::Separator_Paragraph)
            return {};
    }
    return trimmed;
}

// The program an Exec line starts, as a bare executable name (env, flatpak wrappers and paths dropped).
QString execProgram(const QString &exec) {
    const auto words = exec.split(QLatin1Char(' '), Qt::SkipEmptyParts);
    int index = 0;
    if (index < words.size() && words[index] == QLatin1String("env")) {
        ++index;
        while (index < words.size() && words[index].contains(QLatin1Char('='))) ++index;
    }
    if (index >= words.size()) return {};
    auto program = words[index];
    program.remove(QLatin1Char('"'));
    return QFileInfo(program).fileName();
}

#ifdef Q_OS_LINUX
QStringList applicationDirectories() {
    QStringList directories = QStandardPaths::standardLocations(QStandardPaths::ApplicationsLocation);
    directories.removeDuplicates();
    return directories;
}
#endif
}

PlatformSettings::PlatformSettings(QSettings &storage, QObject *parent) : QObject(parent), storage_(storage) {
    for (const auto &id : storage.value("compatibility/apps").toStringList()) {
        const auto normalized = appId(id);
        if (!normalized.isEmpty() && !apps_.contains(normalized)) apps_.append(normalized);
    }
}
bool PlatformSettings::fail(const QString &message) { error_ = message; emit changed(); return false; }
bool PlatformSettings::saveApps(const QStringList &apps) {
    const auto previous = storage_.value("compatibility/apps");
    storage_.setValue("compatibility/apps", apps);
    storage_.sync();
    if (storage_.status() != QSettings::NoError) {
        if (previous.isValid()) storage_.setValue("compatibility/apps", previous);
        else storage_.remove("compatibility/apps");
        return fail(QStringLiteral("無法儲存 App 清單。"));
    }
    apps_ = apps; error_.clear(); emit changed(); return true;
}
bool PlatformSettings::addApp(const QString &id) {
    const auto normalized = appId(id);
    if (normalized.isEmpty()) return fail(QStringLiteral("請輸入有效的 App 名稱。"));
    if (apps_.contains(normalized)) { error_.clear(); emit changed(); return true; }
    auto next = apps_; next.append(normalized); return saveApps(next);
}
bool PlatformSettings::removeApp(const QString &id) {
    auto next = apps_; next.removeAll(id); return saveApps(next);
}

QMap<QString, QString> PlatformSettings::desktopEntries(const QStringList &directories) {
    QMap<QString, QString> entries;
    const auto locale = QLocale::system().name();          // zh_TW
    const auto language = locale.section(QLatin1Char('_'), 0, 0);
    for (const auto &directory : directories) {
        QDirIterator files(directory, {QStringLiteral("*.desktop")}, QDir::Files, QDirIterator::Subdirectories);
        while (files.hasNext()) {
            QFile file(files.next());
            if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) continue;
            QTextStream stream(&file);
            bool main = false, hidden = false, application = true;
            QString name, localized, exec;
            while (!stream.atEnd()) {
                const auto line = stream.readLine().trimmed();
                if (line.startsWith(QLatin1Char('['))) { main = line == QLatin1String("[Desktop Entry]"); continue; }
                if (!main) continue;
                const auto equals = line.indexOf(QLatin1Char('='));
                if (equals <= 0) continue;
                const auto key = line.left(equals).trimmed(), value = line.mid(equals + 1).trimmed();
                if (key == QLatin1String("Name")) name = value;
                else if (key == QStringLiteral("Name[%1]").arg(locale) || (localized.isEmpty() && key == QStringLiteral("Name[%1]").arg(language)))
                    localized = value;
                else if (key == QLatin1String("Exec")) exec = value;
                else if ((key == QLatin1String("NoDisplay") || key == QLatin1String("Hidden")) && value == QLatin1String("true")) hidden = true;
                else if (key == QLatin1String("Type")) application = value == QLatin1String("Application");
            }
            const auto program = execProgram(exec);
            // The first directory wins, as XDG lookup does (the user's own entries come first).
            if (!hidden && application && !program.isEmpty() && !entries.contains(program))
                entries.insert(program, localized.isEmpty() ? name : localized);
        }
    }
    return entries;
}

QVariantList PlatformSettings::runningApps() const {
    QMap<QString, QString> running;  // id → name
#if defined(Q_OS_WIN)
    struct Search { QMap<QString, QString> *found; DWORD self; } search{&running, GetCurrentProcessId()};
    EnumWindows([](HWND window, LPARAM data) -> BOOL {
        auto *search = reinterpret_cast<Search *>(data);
        if (!IsWindowVisible(window) || GetWindow(window, GW_OWNER) || GetWindowTextLengthW(window) == 0) return TRUE;
        DWORD process = 0;
        GetWindowThreadProcessId(window, &process);
        if (process == search->self) return TRUE;
        HANDLE handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process);
        if (!handle) return TRUE;
        wchar_t path[MAX_PATH * 2];
        DWORD size = DWORD(std::size(path));
        if (QueryFullProcessImageNameW(handle, 0, path, &size)) {
            const auto id = QFileInfo(QString::fromWCharArray(path, int(size))).fileName().toLower();
            if (!id.isEmpty()) search->found->insert(id, QFileInfo(id).completeBaseName());
        }
        CloseHandle(handle);
        return TRUE;
    }, reinterpret_cast<LPARAM>(&search));
#elif defined(Q_OS_LINUX)
    // Processes of this user whose program has a visible desktop entry: the apps on the desktop.
    const auto entries = desktopEntries(applicationDirectories());
    const auto self = QCoreApplication::applicationPid();
    const auto uid = QFileInfo(QStringLiteral("/proc/self")).ownerId();
    const auto processes = QDir(QStringLiteral("/proc")).entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot);
    for (const auto &process : processes) {
        bool number = false;
        const auto pid = process.fileName().toLongLong(&number);
        if (!number || pid == self || process.ownerId() != uid) continue;
        const auto program = QFileInfo(QFileInfo(process.filePath() + QStringLiteral("/exe")).symLinkTarget()).fileName();
        QFile comm(process.filePath() + QStringLiteral("/comm"));
        const auto command = comm.open(QIODevice::ReadOnly) ? QString::fromUtf8(comm.readAll()).trimmed() : QString();
        for (const auto &id : {program, command})
            if (!id.isEmpty() && entries.contains(id)) { running.insert(id, entries.value(id)); break; }
    }
#endif
    QVariantList result;
    for (auto it = running.cbegin(); it != running.cend(); ++it)
        if (!apps_.contains(it.key())) result.append(QVariantMap{{"id", it.key()}, {"name", it.value()}});
    std::sort(result.begin(), result.end(), [](const QVariant &a, const QVariant &b) {
        return QString::localeAwareCompare(a.toMap().value("name").toString(), b.toMap().value("name").toString()) < 0;
    });
    return result;
}

QString PlatformSettings::appName(const QString &id) const {
#if defined(Q_OS_LINUX)
    static const auto entries = desktopEntries(applicationDirectories());
    if (entries.contains(id)) return entries.value(id);
#elif defined(Q_OS_WIN)
    if (id.endsWith(QLatin1String(".exe"), Qt::CaseInsensitive)) return id.chopped(4);
#endif
    return id;
}
