#pragma once
#include <fcitx-utils/key.h>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace zhuyintype {
struct BrokerKey { int key; int modifiers; int shiftSide; };
std::optional<BrokerKey> translateKey(fcitx::Key key);
// Broker offsets are UTF-16 units; Fcitx Text cursors are UTF-8 byte offsets.
std::optional<int> cursorBytes(std::string_view text, size_t utf16Offset);
// How the composition is marked (IPC.md): every word underlined, the word the candidate list replaces (`focus`)
// highlighted, the word ↓ would change (`target`) in bold.
enum class Mark { Word, Focus, Target };
struct PreeditPart { std::string text; Mark mark; };
// The preedit cut at the words' UTF-16 `clauses`; the whole of it as one word when they do not fit it.
// What of the host's `commit` is still to be sent when `delivered` (the composition shown in the field) was already
// committed there: the text committed before it (a mouse pick the field has not received yet), or nothing when the
// host's text ends differently (the field keeps the text the user saw).
std::string_view undelivered(std::string_view commit, std::string_view delivered);
std::vector<PreeditPart> preeditParts(std::string_view preedit, const std::vector<std::pair<size_t, size_t>> &clauses,
                                      std::optional<size_t> focus, std::optional<size_t> target);
}
