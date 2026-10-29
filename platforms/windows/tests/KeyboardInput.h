#pragma once
#include <windows.h>
#include <array>
// A failed GUI test can leave a modifier down. Synthetic fixtures start from a
// neutral keyboard while their own test window is foreground.
inline bool ReleaseTestModifiers() {
    const WORD modifiers[]={VK_LSHIFT,VK_RSHIFT,VK_LCONTROL,VK_RCONTROL,VK_LMENU,VK_RMENU,VK_LWIN,VK_RWIN};
    std::array<INPUT,8> events{};
    for (size_t i=0;i<events.size();++i) { events[i].type=INPUT_KEYBOARD; events[i].ki.wVk=modifiers[i]; events[i].ki.dwFlags=KEYEVENTF_KEYUP; }
    return SendInput(static_cast<UINT>(events.size()),events.data(),sizeof(INPUT))==events.size();
}
