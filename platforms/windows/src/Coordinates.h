#pragma once
#include <windows.h>
#include <array>
#include <cmath>
namespace zt {
inline std::array<double,4> QtRect(RECT physical,POINT monitorOrigin,double scale) noexcept {
    if (!std::isfinite(scale) || scale<=0) scale=1;
    return {monitorOrigin.x+(static_cast<double>(physical.left)-monitorOrigin.x)/scale,
        monitorOrigin.y+(static_cast<double>(physical.top)-monitorOrigin.y)/scale,
        static_cast<double>(physical.right-physical.left)/scale,
        static_cast<double>(physical.bottom-physical.top)/scale};
}
}
