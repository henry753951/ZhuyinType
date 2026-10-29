#pragma once
#include <windows.h>
inline bool ActivateTestWindow(HWND window,HWND control) {
    const auto thread=GetCurrentThreadId();
    const auto foreground=GetForegroundWindow();
    const auto owner=foreground?GetWindowThreadProcessId(foreground,nullptr):0;
    const bool attached=owner && owner!=thread && AttachThreadInput(thread,owner,TRUE);
    ShowWindow(window,SW_RESTORE);
    BringWindowToTop(window);
    SetForegroundWindow(window);
    SetActiveWindow(window);
    SetFocus(control);
    if (attached) AttachThreadInput(thread,owner,FALSE);
    return GetForegroundWindow()==window && GetFocus()==control;
}
