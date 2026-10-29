#pragma once
#include <QObject>

// The ZhuyinType logo in the Windows notification area while the input method service runs: a monochrome square with
// slightly rounded corners and the ㄓ cut out, drawn in the taskbar's text colour and redrawn when the taskbar
// switches between light and dark. Clicking it opens the settings. The 中/英 indicator itself is the TSF input-mode
// button (platforms/windows/src/InputModeButton). On other platforms this object does nothing.
class TrayIcon final : public QObject {
    Q_OBJECT
public:
    explicit TrayIcon(QObject *parent = nullptr);
    ~TrayIcon() override;
    TrayIcon(const TrayIcon &) = delete;
    TrayIcon &operator=(const TrayIcon &) = delete;
    bool shown() const { return shown_; }
signals:
    void activated();
private:
    bool shown_ = false;
#ifdef Q_OS_WIN
    void *window_ = nullptr;
    void *icon_ = nullptr;
    unsigned taskbarCreated_ = 0;
    bool add();
    void refresh();
    friend struct TrayWindow;
#endif
};
