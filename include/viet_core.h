#ifndef VIET_CORE_H
#define VIET_CORE_H
#include <stdbool.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct Engine VietEngine;
/* flags: 1 free marking, 2 hòa/ủy style; default 7. Clear bit 2 for hoà/uỷ. */
VietEngine *viet_engine_new(uint32_t flags);
void viet_engine_free(VietEngine *engine);
bool viet_engine_key(VietEngine *engine, uint32_t unicode_scalar);
bool viet_engine_process(VietEngine *engine, const char *utf8);
char *viet_engine_text(const VietEngine *engine);
void viet_string_free(char *text);
void viet_engine_reset(VietEngine *engine);
void viet_engine_backspace(VietEngine *engine);
/* Serialize calls per handle. Free each handle/string exactly once. */
#ifdef __cplusplus
}
#endif
#endif
