#include "ZhuyinType.h"
#include <fcitx-utils/log.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>

class ZhuyinTypeFactory final : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        try {
            return new zhuyintype::ZhuyinType(*manager->instance());
        } catch (const std::exception &) {
            FCITX_ERROR() << "無法載入 ZhuyinType 原生前端。";
            return nullptr;
        }
    }
};
FCITX_ADDON_FACTORY_V2(zhuyintype, ZhuyinTypeFactory)
