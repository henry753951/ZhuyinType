#include "Keys.h"
#include <fcitx-utils/utf8.h>
#include <limits>

namespace zhuyintype {
std::optional<BrokerKey> translateKey(fcitx::Key input) {
    using fcitx::KeyState;
    const auto states = input.states();
    int mods = 0;
    if (states.test(KeyState::Shift)) mods |= 0x02000000;
    if (states.test(KeyState::Ctrl)) mods |= 0x04000000;
    if (states.test(KeyState::Alt)) mods |= 0x08000000;
    if (states.test(KeyState::Super) || states.test(KeyState::Meta)) mods |= 0x10000000;
    uint32_t sym = input.sym();
    if (sym >= FcitxKey_KP_Space && sym <= FcitxKey_KP_Equal) mods |= 0x20000000;
    if (sym >= FcitxKey_KP_0 && sym <= FcitxKey_KP_9) sym = '0' + sym - FcitxKey_KP_0;
    switch (sym) {
    case FcitxKey_KP_Space: sym = FcitxKey_space; break;
    case FcitxKey_KP_Add: sym = '+'; break;
    case FcitxKey_KP_Subtract: sym = '-'; break;
    case FcitxKey_KP_Multiply: sym = '*'; break;
    case FcitxKey_KP_Divide: sym = '/'; break;
    case FcitxKey_KP_Decimal: sym = '.'; break;
    case FcitxKey_KP_Separator: sym = ','; break;
    case FcitxKey_KP_Equal: sym = '='; break;
    case FcitxKey_KP_Tab: sym = FcitxKey_Tab; break;
    case FcitxKey_KP_Home: sym = FcitxKey_Home; break;
    case FcitxKey_KP_End: sym = FcitxKey_End; break;
    case FcitxKey_KP_Left: sym = FcitxKey_Left; break;
    case FcitxKey_KP_Right: sym = FcitxKey_Right; break;
    case FcitxKey_KP_Up: sym = FcitxKey_Up; break;
    case FcitxKey_KP_Down: sym = FcitxKey_Down; break;
    case FcitxKey_KP_Page_Up: sym = FcitxKey_Page_Up; break;
    case FcitxKey_KP_Page_Down: sym = FcitxKey_Page_Down; break;
    case FcitxKey_KP_Insert: sym = FcitxKey_Insert; break;
    case FcitxKey_KP_Delete: sym = FcitxKey_Delete; break;
    default: break;
    }
    if (sym >= 'a' && sym <= 'z') sym -= 'a' - 'A';
    if (sym >= 0x20 && sym <= 0x7e) return BrokerKey{static_cast<int>(sym), mods, 0};
    // Qt public key enum values, without linking Qt into the Fcitx process.
    int key = 0, side = 0;
    switch (sym) {
    case FcitxKey_Escape: key = 0x01000000; break;
    case FcitxKey_Tab: key = 0x01000001; break;
    case FcitxKey_ISO_Left_Tab: key = 0x01000001; mods |= 0x02000000; break;
    case FcitxKey_BackSpace: key = 0x01000003; break;
    case FcitxKey_Return: key = 0x01000004; break;
    case FcitxKey_KP_Enter: key = 0x01000005; break;
    case FcitxKey_Insert: key = 0x01000006; break;
    case FcitxKey_Delete: key = 0x01000007; break;
    case FcitxKey_Pause: key = 0x01000008; break;
    case FcitxKey_Print: key = 0x01000009; break;
    case FcitxKey_Home: key = 0x01000010; break;
    case FcitxKey_End: key = 0x01000011; break;
    case FcitxKey_Left: key = 0x01000012; break;
    case FcitxKey_Up: key = 0x01000013; break;
    case FcitxKey_Right: key = 0x01000014; break;
    case FcitxKey_Down: key = 0x01000015; break;
    case FcitxKey_Page_Up: key = 0x01000016; break;
    case FcitxKey_Page_Down: key = 0x01000017; break;
    case FcitxKey_Shift_L: key = 0x01000020; side = 1; break;
    case FcitxKey_Shift_R: key = 0x01000020; side = 2; break;
    case FcitxKey_Control_L: case FcitxKey_Control_R: key = 0x01000021; break;
    case FcitxKey_Meta_L: case FcitxKey_Meta_R: case FcitxKey_Super_L: case FcitxKey_Super_R: key = 0x01000022; break;
    case FcitxKey_Alt_L: case FcitxKey_Alt_R: key = 0x01000023; break;
    case FcitxKey_Caps_Lock: key = 0x01000024; break;
    case FcitxKey_Num_Lock: key = 0x01000025; break;
    case FcitxKey_Scroll_Lock: key = 0x01000026; break;
    case FcitxKey_ISO_Level3_Shift: key = 0x01001103; break;
    default:
        if (sym >= FcitxKey_F1 && sym <= FcitxKey_F35) key = 0x01000030 + sym - FcitxKey_F1;
        else return std::nullopt;
    }
    return BrokerKey{key, mods, side};
}

std::optional<int> cursorBytes(std::string_view text, size_t utf16Offset) {
    if (text.empty()) return utf16Offset == 0 ? std::optional<int>(0) : std::nullopt;
    if (text.size() > static_cast<size_t>(std::numeric_limits<int>::max()) || !fcitx::utf8::validate(text))
        return std::nullopt;
    auto it = text.begin();
    size_t units = 0;
    while (it != text.end() && units < utf16Offset) {
        uint32_t scalar = 0;
        it = fcitx::utf8::getNextChar(it, text.end(), &scalar);
        units += scalar > 0xffff ? 2 : 1;
    }
    if (units != utf16Offset) return std::nullopt;
    return static_cast<int>(it - text.begin());
}
}
