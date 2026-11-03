// SPDX-License-Identifier: MIT -- transport adapter, no Vietnamese/config logic.
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/text.h>
#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <cstring>
#include "viet_core.h"

namespace {
struct State : fcitx::InputContextProperty {
    VietEngine *core = viet_engine_new();
    ~State() override { viet_engine_free(core); }
};
void apply(fcitx::InputContext *client, const CActionResult &result) {
    const char *preedit = result.preedit_str ? result.preedit_str : "";
    fcitx::Text text(preedit, fcitx::TextFormatFlag::Underline);
    text.setCursor(static_cast<int>(std::strlen(preedit))); // Fcitx cursor uses bytes.
    client->inputPanel().setClientPreedit(text);
    client->updatePreedit();
    if (result.action_type == VIET_DELETE_SURROUNDING) {
        client->deleteSurroundingText(-static_cast<int>(result.backspace_count), result.backspace_count);
    } else if (result.action_type == VIET_FAKE_BACKSPACE) {
        for (uint32_t i = 0; i < result.backspace_count; ++i) {
            client->forwardKey(fcitx::Key(FcitxKey_BackSpace), false);
            client->forwardKey(fcitx::Key(FcitxKey_BackSpace), true);
        }
    }
    if (result.commit_str && *result.commit_str) client->commitString(result.commit_str);
}
uint32_t modifiers(fcitx::InputContext *client, const fcitx::KeyEvent &event) {
    uint32_t mods = 0;
    auto states = event.rawKey().states();
    if (states.test(fcitx::KeyState::Shift)) mods |= VIET_SHIFT;
    if (states.test(fcitx::KeyState::Ctrl)) mods |= VIET_CTRL;
    if (states.test(fcitx::KeyState::Alt) || states.test(fcitx::KeyState::Mod5)) mods |= VIET_ALT;
    if (states.test(fcitx::KeyState::Super) || states.test(fcitx::KeyState::Super2)
        || states.test(fcitx::KeyState::Meta) || states.test(fcitx::KeyState::Hyper)
        || states.test(fcitx::KeyState::Hyper2)) mods |= VIET_SUPER;
    if (states.test(fcitx::KeyState::IgnoredMask)) mods |= VIET_FORWARDED;
    if (event.isRelease()) mods |= VIET_RELEASE;
    auto caps = client->capabilityFlags();
    if (caps.test(fcitx::CapabilityFlag::SurroundingText)) mods |= VIET_CAP_SURROUNDING;
    if (caps.test(fcitx::CapabilityFlag::Password) || caps.test(fcitx::CapabilityFlag::Sensitive))
        mods |= VIET_SENSITIVE;
    return mods;
}
class VietPlugin final : public fcitx::InputMethodEngineV2 {
    fcitx::FactoryFor<State> states_{[](fcitx::InputContext &) { return new State; }};
public:
    explicit VietPlugin(fcitx::Instance *instance) {
        instance->inputContextManager().registerProperty("VietState", &states_);
    }
    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
        auto *client = event.inputContext();
        viet_engine_set_app_id(client->propertyFor(&states_)->core, client->program().c_str());
    }
    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override {
        auto *client = event.inputContext();
        auto *state = client->propertyFor(&states_);
        if (!state->core) return;
        auto result = viet_engine_process_key(state->core, event.rawKey().sym(),
                                              event.rawKey().code(), modifiers(client, event));
        if (!event.isRelease() && !(modifiers(client, event) & VIET_FORWARDED)) apply(client, result);
        if (result.action_type != VIET_IGNORE) event.filterAndAccept();
    }
    void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
        auto *client = event.inputContext();
        auto *state = client->propertyFor(&states_);
        apply(client, viet_engine_process_key(state->core, VIET_KEY_FLUSH, 0, 0));
        viet_engine_reset(state->core);
    }
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
        auto *client = event.inputContext();
        viet_engine_reset(client->propertyFor(&states_)->core);
        client->inputPanel().setClientPreedit(fcitx::Text());
        client->updatePreedit();
    }
};
class Factory final : public fcitx::AddonFactory {
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new VietPlugin(manager->instance());
    }
};
} // namespace
FCITX_ADDON_FACTORY(Factory)
