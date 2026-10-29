#pragma once
#include <fcitx-utils/key.h>
#include <optional>
#include <string_view>

namespace zhuyintype {
struct BrokerKey { int key; int modifiers; int shiftSide; };
std::optional<BrokerKey> translateKey(fcitx::Key key);
// Broker offsets are UTF-16 units; Fcitx Text cursors are UTF-8 byte offsets.
std::optional<int> cursorBytes(std::string_view text, size_t utf16Offset);
}
