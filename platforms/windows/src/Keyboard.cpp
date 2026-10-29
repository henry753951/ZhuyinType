#include "Keyboard.h"
#include <array>
namespace zt {
bool PotentialKey(WPARAM vk) noexcept {
    return vk != VK_LWIN && vk != VK_RWIN && vk != VK_PROCESSKEY && vk != VK_PACKET;
}
bool HostWantsKey(const Key& key, bool composing, bool english) noexcept {
    if (!key.key) return false;
    if (composing) return true;
    if (key.key == 0x01000020) return true;  // Shift: a tap switches 中/英.
    if (english || key.release) return false;
    if (key.key >= 0x01000000 || key.key <= 0x20) return false;  // Space, Return, arrows and editing keys when idle.
    const int chord = key.modifiers & (0x04000000 | 0x08000000 | 0x10000000);
    if (!chord) return true;  // Zhuyin keys, digits for tones, punctuation.
    if (chord & 0x10000000) return false;  // Win: the system's.
    const bool letter = (key.key >= 'A' && key.key <= 'Z') || (key.key >= 'a' && key.key <= 'z');
    // Ctrl or Alt with a symbol key types 中文標點 / the key's symbol; with a letter it is the app's shortcut.
    return !letter && chord != (0x04000000 | 0x08000000);
}
wchar_t TypedCharacter(WPARAM vk, LPARAM flags) noexcept {
    std::array<BYTE,256> state{};
    std::array<wchar_t,8> chars{};
    if (!GetKeyboardState(state.data())) return 0;
    if ((state[VK_CONTROL] | state[VK_MENU]) & 0x80) return 0;
    const int n=ToUnicodeEx(static_cast<UINT>(vk), (static_cast<UINT>(flags)>>16)&0xff, state.data(), chars.data(),
        static_cast<int>(chars.size()), 4, GetKeyboardLayout(0));
    return n==1 && chars[0]>=32 && !(chars[0]>=0xd800 && chars[0]<=0xdfff) ? chars[0] : 0;
}
Key TranslateKey(WPARAM vk, LPARAM flags, bool release) noexcept {
    Key r;
    r.release = release;
    r.repeat = !release && (flags & (1LL << 30)) != 0;
    if (GetKeyState(VK_SHIFT) & 0x8000) r.modifiers |= 0x02000000;
    if (GetKeyState(VK_CONTROL) & 0x8000) r.modifiers |= 0x04000000;
    if (GetKeyState(VK_MENU) & 0x8000) r.modifiers |= 0x08000000;
    if ((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000) r.modifiers |= 0x10000000;
    if (vk >= VK_NUMPAD0 && vk <= VK_DIVIDE) r.modifiers |= 0x20000000;
    if (vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT) {
        const auto physical = vk == VK_SHIFT ? MapVirtualKeyW((static_cast<UINT>(flags) >> 16) & 0xff, MAPVK_VSC_TO_VK_EX) : vk;
        r.shiftSide = physical == VK_LSHIFT ? 1 : physical == VK_RSHIFT ? 2 : 0;
        r.key = 0x01000020;
    } else if (vk >= 'A' && vk <= 'Z') r.key = static_cast<int>(vk);
    else if (vk >= VK_F1 && vk <= VK_F24) r.key = 0x01000030 + static_cast<int>(vk - VK_F1);
    else {
        switch (vk) {
        case VK_ESCAPE: r.key=0x01000000; break;
        case VK_TAB: r.key=0x01000001; break;
        case VK_BACK: r.key=0x01000003; break;
        case VK_RETURN: r.key=(flags & (1LL<<24)) ? 0x01000005 : 0x01000004; break;
        case VK_INSERT: r.key=0x01000006; break;
        case VK_DELETE: r.key=0x01000007; break;
        case VK_HOME: r.key=0x01000010; break;
        case VK_END: r.key=0x01000011; break;
        case VK_LEFT: r.key=0x01000012; break;
        case VK_UP: r.key=0x01000013; break;
        case VK_RIGHT: r.key=0x01000014; break;
        case VK_DOWN: r.key=0x01000015; break;
        case VK_PRIOR: r.key=0x01000016; break;
        case VK_NEXT: r.key=0x01000017; break;
        case VK_CONTROL: case VK_LCONTROL: case VK_RCONTROL: r.key=0x01000021; break;
        case VK_MENU: case VK_LMENU: case VK_RMENU: r.key=0x01000023; break;
        case VK_CAPITAL: r.key=0x01000024; break;
        case VK_NUMLOCK: r.key=0x01000025; break;
        case VK_SCROLL: r.key=0x01000026; break;
        default: {
            std::array<BYTE,256> state{};
            std::array<wchar_t,8> chars{};
            if (!GetKeyboardState(state.data())) break;
            // Ignore Ctrl/Alt for the key identity; they travel in modifiers.
            state[VK_CONTROL]=state[VK_LCONTROL]=state[VK_RCONTROL]=0;
            state[VK_MENU]=state[VK_LMENU]=state[VK_RMENU]=0;
            const int n=ToUnicodeEx(static_cast<UINT>(vk), (static_cast<UINT>(flags)>>16)&0xff,
                state.data(), chars.data(), static_cast<int>(chars.size()), 4, GetKeyboardLayout(0));
            if (n==1 && chars[0]>=32 && !(chars[0]>=0xd800 && chars[0]<=0xdfff)) r.key=chars[0];
        }
        }
    }
    return r;
}
}
