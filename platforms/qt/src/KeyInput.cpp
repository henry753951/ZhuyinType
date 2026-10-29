#include "KeyInput.h"
#include <Qt>
#include <QByteArray>

std::optional<KeyInput> KeyInput::fromQt(int code, int modifiers) {
    // Qt swaps Control and Meta on macOS; native Control arrives as MetaModifier.
#ifdef Q_OS_MACOS
    constexpr int controlModifier = Qt::MetaModifier;
    constexpr int commandModifier = Qt::ControlModifier;
#else
    constexpr int controlModifier = Qt::ControlModifier;
    constexpr int commandModifier = Qt::MetaModifier;
#endif
    if (modifiers & commandModifier) return std::nullopt;
    const int mods = modifiers & ~int(Qt::KeypadModifier);
    const bool shift = mods == Qt::ShiftModifier;
    const bool alt = mods == Qt::AltModifier;
    const bool control = mods == controlModifier;
    if (mods != 0 && !shift && !alt && !control) return std::nullopt;
    const auto key = [](Kind kind, char16_t c = 0) { return KeyInput{kind, c}; };
    switch (code) {
    case Qt::Key_Space: if (mods == 0 || shift) return key(shift ? ShiftSpace : Space); break;
    case Qt::Key_Return: case Qt::Key_Enter: if (mods == 0 || shift) return key(shift ? ShiftEnter : Enter); break;
    case Qt::Key_Backspace: if (mods == 0 || alt) return key(alt ? OptionBackspace : Backspace); break;
    case Qt::Key_Delete: if (mods == 0) return key(ForwardDelete); break;
    case Qt::Key_Escape: if (mods == 0) return key(Escape); break;
    case Qt::Key_Tab: case Qt::Key_Backtab:
        if (mods == 0 || shift) return key(shift || code == Qt::Key_Backtab ? ShiftTab : Tab);
        break;
    case Qt::Key_Left: if (mods == 0 || shift) return key(shift ? ShiftLeft : Left); break;
    case Qt::Key_Right: if (mods == 0 || shift) return key(shift ? ShiftRight : Right); break;
    case Qt::Key_Up:
        if (alt) return key(PageUp);
        if (mods == 0 || shift) return key(shift ? ShiftUp : Up);
        break;
    case Qt::Key_Down:
        if (alt) return key(PageDown);
        if (mods == 0 || shift) return key(shift ? ShiftDown : Down);
        break;
    case Qt::Key_Home: if (mods == 0) return key(Home); break;
    case Qt::Key_End: if (mods == 0) return key(End); break;
    case Qt::Key_PageUp: if (mods == 0) return key(PageUp); break;
    case Qt::Key_PageDown: if (mods == 0) return key(PageDown); break;
    default: {
        if (code < 0x21 || code > 0x7e) return std::nullopt;
        if (code >= 'A' && code <= 'Z') code += 'a' - 'A';
        if (shift) {
            const QByteArray shifted("~!@#$%^&*()_+{}|:\"<>?");
            const QByteArray plain("`1234567890-=[]\\;',./");
            const auto index = shifted.indexOf(char(code));
            if (index >= 0) code = plain[index];
        }
        if ((alt || control) && code >= 'a' && code <= 'z') return std::nullopt;
        return key(shift ? ShiftedChar : alt ? Printed : control ? ControlChar : Char, char16_t(code));
    }
    }
    return std::nullopt;
}

QString KeyInput::label() const {
    const QString c{QChar(character)};
    switch (kind) {
    case Char: return c == " " ? "<space>" : c == "<" || c == ">" ? QString() : c;
    case ShiftedChar: return c == ">" ? QString() : "<S-" + c + ">";
    case Printed: return c == ">" ? QString() : "<O-" + c + ">";
    case ControlChar: return c == ">" ? QString() : "<C-" + c + ">";
    case Space: return "<space>";
    case ShiftSpace: return "<sspace>";
    case Enter: return "<enter>";
    case ShiftEnter: return "<senter>";
    case Backspace: return "<bs>";
    case OptionBackspace: return "<obs>";
    case ForwardDelete: return "<del>";
    case Escape: return "<esc>";
    case Tab: return "<tab>";
    case ShiftTab: return "<stab>";
    case Left: return "<left>";
    case Right: return "<right>";
    case Up: return "<up>";
    case Down: return "<down>";
    case Home: return "<home>";
    case End: return "<end>";
    case PageUp: return "<pgup>";
    case PageDown: return "<pgdown>";
    case ShiftLeft: return "<sleft>";
    case ShiftRight: return "<sright>";
    case ShiftDown: return "<sdown>";
    case ShiftUp: return "<sup>";
    }
    return {};
}
