#include "FontSetup.h"
#include <QFontDatabase>
#include <QGuiApplication>
#include <algorithm>

void FontSetup::apply() {
    const auto available = QFontDatabase::families();
#if defined(Q_OS_WIN)
    const QStringList preferred = {"Microsoft JhengHei UI", "Microsoft JhengHei", "Noto Sans TC", "Noto Sans CJK TC"};
#elif defined(Q_OS_MACOS)
    const QStringList preferred = {"PingFang TC", "Noto Sans TC", "Noto Sans CJK TC"};
#else
    const QStringList preferred = {"Noto Sans CJK TC", "Noto Sans TC", "Source Han Sans TW", "Source Han Sans TC", "WenQuanYi Zen Hei"};
#endif
    QStringList traditionalSans;
    for (const auto &family : preferred) {
        const auto found = std::find_if(available.cbegin(), available.cend(), [&](const QString &name) {
            return name.compare(family, Qt::CaseInsensitive) == 0;
        });
        if (found != available.cend()) traditionalSans.append(*found);
    }
    auto font = QFontDatabase::systemFont(QFontDatabase::GeneralFont);
    font.setStyleHint(QFont::SansSerif);
#if !defined(Q_OS_MACOS)
    // Put the TC sans first: a Latin system font's automatic CJK fallback may be serif or use JP glyphs.
    if (!traditionalSans.isEmpty()) font.setFamilies(traditionalSans + font.families());
#endif
    if (font.family() == QStringLiteral("Sans Serif") && !traditionalSans.isEmpty())
        font.setFamilies(traditionalSans);
    QGuiApplication::setFont(font);
    if (!traditionalSans.isEmpty()) {
        QFontDatabase::setApplicationFallbackFontFamilies(QChar::Script_Han, traditionalSans);
        QFontDatabase::setApplicationFallbackFontFamilies(QChar::Script_Bopomofo, traditionalSans);
    }
}
