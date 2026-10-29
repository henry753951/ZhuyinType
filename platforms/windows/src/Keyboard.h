#pragma once
#include <windows.h>
namespace zt {
struct Key { int key = 0, modifiers = 0, shiftSide = 0; bool release = false, repeat = false; };
Key TranslateKey(WPARAM vk, LPARAM flags, bool release) noexcept;
bool PotentialKey(WPARAM vk) noexcept;
}
