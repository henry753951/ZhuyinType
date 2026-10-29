#pragma once
#include <windows.h>
namespace zt {
struct Key { int key = 0, modifiers = 0, shiftSide = 0; bool release = false, repeat = false; };
Key TranslateKey(WPARAM vk, LPARAM flags, bool release) noexcept;
bool PotentialKey(WPARAM vk) noexcept;
/// Whether the host would act on this key, decided locally because TSF drops a key whose test said "eaten" even when
/// OnKeyDown then declines it. While composing the host owns every key; otherwise only Shift (中/英), and in Chinese
/// mode the keys that type zhuyin or punctuation. Ctrl/Alt with a letter, Win, editing and navigation keys go to the app.
bool HostWantsKey(const Key& key, bool composing, bool english) noexcept;
/// The character the key types in the current keyboard state, for a key the host declined; 0 when none.
wchar_t TypedCharacter(WPARAM vk, LPARAM flags) noexcept;
}
