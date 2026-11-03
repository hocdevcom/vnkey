/* SPDX-License-Identifier: MIT -- transport adapter, no Vietnamese/config logic. */
#include <ibus.h>
#include "viet_core.h"

typedef struct { IBusEngine parent; VietEngine *core; gboolean sensitive; } IBusViet;
typedef struct { IBusEngineClass parent; } IBusVietClass;
G_DEFINE_TYPE(IBusViet, ibus_viet, IBUS_TYPE_ENGINE)

static void apply(IBusEngine *engine, CActionResult result) {
    const char *preedit = result.preedit_str ? result.preedit_str : "";
    IBusText *text = ibus_text_new_from_string(preedit);
    ibus_text_append_attribute(text, IBUS_ATTR_TYPE_UNDERLINE, IBUS_ATTR_UNDERLINE_SINGLE, 0, -1);
    ibus_engine_update_preedit_text(engine, text,
                                   (guint)g_utf8_strlen(preedit, -1), *preedit != '\0');
    if (result.action_type == VIET_DELETE_SURROUNDING) {
        ibus_engine_delete_surrounding_text(engine, -(gint)result.backspace_count,
                                           result.backspace_count);
    } else if (result.action_type == VIET_FAKE_BACKSPACE) {
        for (uint32_t i = 0; i < result.backspace_count; ++i) {
            ibus_engine_forward_key_event(engine, IBUS_KEY_BackSpace, 0, 0);
            ibus_engine_forward_key_event(engine, IBUS_KEY_BackSpace, 0, IBUS_RELEASE_MASK);
        }
    }
    if (result.commit_str && *result.commit_str)
        ibus_engine_commit_text(engine, ibus_text_new_from_string(result.commit_str));
}
static guint modifiers(IBusEngine *engine, guint state) {
    IBusViet *self = (IBusViet *)engine;
    guint mods = 0;
    if (state & IBUS_SHIFT_MASK) mods |= VIET_SHIFT;
    if (state & IBUS_CONTROL_MASK) mods |= VIET_CTRL;
    if (state & (IBUS_MOD1_MASK | IBUS_MOD5_MASK)) mods |= VIET_ALT;
    if (state & (IBUS_MOD4_MASK | IBUS_SUPER_MASK | IBUS_META_MASK | IBUS_HYPER_MASK)) mods |= VIET_SUPER;
    if (state & IBUS_RELEASE_MASK) mods |= VIET_RELEASE;
    if (state & IBUS_FORWARD_MASK) mods |= VIET_FORWARDED;
    if (engine->client_capabilities & IBUS_CAP_SURROUNDING_TEXT) mods |= VIET_CAP_SURROUNDING;
    if (self->sensitive) mods |= VIET_SENSITIVE;
    return mods;
}
static gboolean process_key(IBusEngine *engine, guint keyval, guint keycode, guint state) {
    IBusViet *self = (IBusViet *)engine;
    if (!self->core) return FALSE;
    CActionResult result = viet_engine_process_key(self->core, keyval, keycode,
                                                  modifiers(engine, state));
    /* Forwarded keys/releases must not clear an active preedit. */
    if (!(state & (IBUS_RELEASE_MASK | IBUS_FORWARD_MASK))) apply(engine, result);
    return result.action_type != VIET_IGNORE;
}
static void reset(IBusEngine *engine) {
    viet_engine_reset(((IBusViet *)engine)->core);
    ibus_engine_update_preedit_text(engine, ibus_text_new_from_string(""), 0, FALSE);
}
static void focus_in_id(IBusEngine *engine, const gchar *path, const gchar *client) {
    (void)path;
    IBusViet *self = (IBusViet *)engine;
    self->sensitive = FALSE;
    /* Core recognizes GTK client prefixes; generic Qt/gnome-shell stays unknown. */
    viet_engine_set_app_id(self->core, client);
    ibus_engine_get_surrounding_text(engine, NULL, NULL, NULL);
}
static void focus_out_id(IBusEngine *engine, const gchar *path) {
    (void)path;
    apply(engine, viet_engine_process_key(((IBusViet *)engine)->core, VIET_KEY_FLUSH, 0, 0));
    reset(engine);
}
static void disable(IBusEngine *engine) { focus_out_id(engine, NULL); }
static void content_type(IBusEngine *engine, guint purpose, guint hints) {
    IBusViet *self = (IBusViet *)engine;
    (void)hints;
    self->sensitive = purpose == IBUS_INPUT_PURPOSE_PASSWORD || purpose == IBUS_INPUT_PURPOSE_PIN;
    if (self->sensitive) reset(engine);
}
static void finalize(GObject *object) {
    viet_engine_free(((IBusViet *)object)->core);
    G_OBJECT_CLASS(ibus_viet_parent_class)->finalize(object);
}
static void ibus_viet_init(IBusViet *self) { self->core = viet_engine_new(); }
static void ibus_viet_class_init(IBusVietClass *klass) {
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    IBusEngineClass *engine_class = IBUS_ENGINE_CLASS(klass);
    object_class->finalize = finalize;
    engine_class->process_key_event = process_key;
    engine_class->reset = reset;
    engine_class->focus_in_id = focus_in_id;
    engine_class->focus_out_id = focus_out_id;
    engine_class->disable = disable;
    engine_class->set_content_type = content_type;
}
static IBusEngine *create_engine(IBusFactory *factory, const gchar *name, gpointer data) {
    (void)factory;
    static guint next_id = 0;
    if (g_strcmp0(name, "viet") != 0) return NULL;
    gchar *path = g_strdup_printf("/org/freedesktop/IBus/Viet/Engine/%u", ++next_id);
    IBusEngine *engine = g_object_new(ibus_viet_get_type(), "engine-name", name,
                                    "object-path", path, "connection", data,
                                    "has-focus-id", TRUE, NULL);
    g_free(path);
    return engine;
}
static void disconnected(IBusBus *bus, gpointer data) { (void)bus; (void)data; ibus_quit(); }
int main(int argc, char **argv) {
    (void)argc; (void)argv;
    ibus_init();
    IBusBus *bus = ibus_bus_new();
    if (!ibus_bus_is_connected(bus)) return 1;
    g_signal_connect(bus, "disconnected", G_CALLBACK(disconnected), NULL);
    IBusFactory *factory = ibus_factory_new(ibus_bus_get_connection(bus));
    g_signal_connect(factory, "create-engine", G_CALLBACK(create_engine), ibus_bus_get_connection(bus));
    ibus_bus_request_name(bus, "org.freedesktop.IBus.Viet", 0);
    ibus_main();
    g_object_unref(factory);
    g_object_unref(bus);
    return 0;
}
