#ifndef LAVINIA_PLATFORM_H
#define LAVINIA_PLATFORM_H

#include <stddef.h>
#include <stdint.h>

#include "../ring_buffer.h"

struct lv_tun {
    lv_ring_t ring;
    void *plat;                      /* platform-private state */
    char name[LV_TUN_NAME_MAX + 1u];
};

/* Implemented by exactly one of platform/linux or platform/windows. */
int32_t lv_platform_open(struct lv_tun *tun, const char *name); /* starts RX pump */
int32_t lv_platform_write(struct lv_tun *tun, const uint8_t *pkt, size_t len);
void lv_platform_close(struct lv_tun *tun);                     /* stops pump, frees plat */

#endif
