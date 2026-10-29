#include "ImeEndpoint.h"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>
#ifdef Q_OS_WIN
#include <windows.h>
#include <sddl.h>
#include <vector>
#else
#include <sys/stat.h>
#include <unistd.h>
#endif

QString imeEndpoint() {
    const auto override = qEnvironmentVariable("ZT_IME_SOCKET");
    if (!override.isEmpty()) return override;
#ifdef Q_OS_WIN
    HANDLE token = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return {};
    DWORD size = 0;
    GetTokenInformation(token, TokenUser, nullptr, 0, &size);
    std::vector<unsigned char> bytes(size);
    const bool valid = GetTokenInformation(token, TokenUser, bytes.data(), size, &size);
    CloseHandle(token);
    if (!valid) return {};
    LPWSTR sid = nullptr;
    if (!ConvertSidToStringSidW(reinterpret_cast<TOKEN_USER *>(bytes.data())->User.Sid, &sid)) return {};
    const QString name = QStringLiteral("ZhuyinType-ime-v1-") + QString::fromWCharArray(sid);
    LocalFree(sid);
    return QStringLiteral("\\\\.\\pipe\\") + name;
#else
    auto base = qEnvironmentVariable("XDG_RUNTIME_DIR");
    base = !base.startsWith('/') ? QStringLiteral("/tmp/zhuyintype-%1").arg(getuid()) : QDir(base).filePath("zhuyintype");
    return QDir(base).filePath("ime-v1");
#endif
}

bool prepareImeEndpoint(const QString &endpoint, QString &error) {
    if (endpoint.isEmpty()) { error = QStringLiteral("無法取得使用者通訊端。"); return false; }
#ifndef Q_OS_WIN
    const QString directory = QFileInfo(endpoint).absolutePath();
    const QByteArray path = QFile::encodeName(directory);
    if (::mkdir(path.constData(), 0700) < 0 && errno != EEXIST) {
        error = QStringLiteral("無法建立輸入法通訊目錄。"); return false;
    }
    struct stat status {};
    if (::lstat(path.constData(), &status) || !S_ISDIR(status.st_mode)
        || status.st_uid != getuid() || (status.st_mode & 0077)) {
        error = QStringLiteral("輸入法通訊目錄必須由目前使用者擁有，且權限為 0700。"); return false;
    }
#endif
    return true;
}
