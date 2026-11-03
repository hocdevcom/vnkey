#ifndef VIET_CORE_H
#define VIET_CORE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct VietEngine VietEngine;
enum VietActionType {
    VIET_IGNORE = 0, VIET_PREEDIT = 1, VIET_COMMIT = 2,
    VIET_DELETE_SURROUNDING = 3, VIET_FAKE_BACKSPACE = 4
};
enum VietModifiers {
    VIET_SHIFT = 1, VIET_CTRL = 2, VIET_ALT = 4, VIET_SUPER = 8,
    VIET_RELEASE = 1u << 16, VIET_FORWARDED = 1u << 17,
    VIET_CAP_SURROUNDING = 1u << 18, VIET_SENSITIVE = 1u << 19
};
#define VIET_KEY_FLUSH UINT32_MAX
typedef struct CActionResult {
    uint32_t action_type;
    uint32_t backspace_count; /* Unicode scalar count, NOT UTF-8 bytes */
    const char *preedit_str;
    const char *commit_str;
} CActionResult;
/* Engine reads ~/.config/viet-ime/config.toml (XDG_CONFIG_HOME supported).
 * Returns NULL on config/IPC initialization failure. One handle per input context.
 * No concurrent/reentrant calls on one handle. Different handles are independent.
 * Strings are engine-owned UTF-8, NULL means empty; valid until next mutating call
 * or free on this handle. Apply/copy before calling core again. NEVER free them.
 * keyval: XKB/IBus/Fcitx keysym (Latin-1 or 0x01000000 | Unicode scalar).
 * keycode: framework keycode, opaque to core. Translate modifiers using enum above.
 * Tell core surrounding-text support on each event; otherwise it uses backspaces.
 * Apply result: update/clear preedit, delete backspace_count scalars immediately
 * before cursor for DELETE_SURROUNDING, or forward that many Backspace press/release
 * pairs for FAKE_BACKSPACE; then commit commit_str if nonempty.
 * IGNORE means pass original event through AFTER applying any commit/preedit;
 * every other action consumes it. Releases/synthetic forwards bypass output updates.
 * VIET_KEY_FLUSH commits pending preedit before focus-out/navigation; do not forward
 * this sentinel. reset discards composition; call after external cursor/text changes.
 * Clear stale preedit on reset/focus-out. In password fields pass VIET_SENSITIVE.
 * Frontend owns event ordering; no XTest/uinput or async injection inside core. */
VietEngine *viet_engine_new(void);
void viet_engine_free(VietEngine *engine);
void viet_engine_set_app_id(VietEngine *engine, const char *app_id);
CActionResult viet_engine_process_key(VietEngine *engine, uint32_t keyval,
                                     uint32_t keycode, uint32_t modifiers);
void viet_engine_reset(VietEngine *engine);
#ifdef __cplusplus
}
#endif
#endif
