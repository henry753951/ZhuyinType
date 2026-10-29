#pragma once

#include <QString>
#include <optional>

// What the composer receives for a Qt key, as macOS's KeyInput: the same mapping EngineSession uses to call the
// engine, kept here so the observers (usage events, the correction log) can name the key without the engine.
struct KeyInput {
    enum Kind {
        Char, ShiftedChar, Printed, ControlChar, Space, ShiftSpace, Enter, ShiftEnter, Backspace, OptionBackspace,
        ForwardDelete, Escape, Tab, ShiftTab, Left, Right, Up, Down, Home, End, PageUp, PageDown, ShiftLeft,
        ShiftRight, ShiftDown, ShiftUp
    };
    Kind kind = Char;
    char16_t character = 0;
    bool operator==(const KeyInput &) const = default;
    bool is(Kind k, char16_t c = 0) const { return kind == k && character == c; }

    static std::optional<KeyInput> fromQt(int key, int modifiers);
    /// The key in zhuyin-cli batch syntax; empty for the few keys it cannot write (> with Option or Control).
    QString label() const;
};
