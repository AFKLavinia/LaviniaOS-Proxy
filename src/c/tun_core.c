#include <stdlib.h>
#include <string.h>

#include "lavinia_tun.h"
#include "platform/platform.h"

int32_t lv_tun_open(const char *name, uint32_t ring_slots, lv_tun_t **out) {
    if (name == NULL || out == NULL || strlen(name) > LV_TUN_NAME_MAX) {
        return LV_ERR_INVALID_ARG;
    }
    *out = NULL;
    struct lv_tun *tun = (struct lv_tun *)calloc(1, sizeof(*tun));
    if (tun == NULL) {
        return LV_ERR_NOMEM;
    }
    int32_t rc = lv_ring_init(&tun->ring, ring_slots);
    if (rc != LV_OK) {
        free(tun);
        return rc;
    }
    rc = lv_platform_open(tun, name);
    if (rc != LV_OK) {
        lv_ring_destroy(&tun->ring);
        free(tun);
        return rc;
    }
    *out = tun;
    return LV_OK;
}

int32_t lv_tun_read(lv_tun_t *tun, uint8_t *buf, size_t cap) {
    if (tun == NULL || buf == NULL) {
        return LV_ERR_INVALID_ARG;
    }
    return lv_ring_pop(&tun->ring, buf, cap);
}

int32_t lv_tun_write(lv_tun_t *tun, const uint8_t *pkt, size_t len) {
    if (tun == NULL || pkt == NULL || len == 0) {
        return LV_ERR_INVALID_ARG;
    }
    if (len > LV_TUN_MAX_PACKET) {
        return LV_ERR_TOO_BIG;
    }
    return lv_platform_write(tun, pkt, len);
}

uint64_t lv_tun_dropped(const lv_tun_t *tun) {
    if (tun == NULL) {
        return 0;
    }
    return atomic_load_explicit(&((struct lv_tun *)tun)->ring.dropped, memory_order_relaxed);
}

int32_t lv_tun_name(const lv_tun_t *tun, char *buf, size_t cap) {
    if (tun == NULL || buf == NULL || cap == 0) {
        return LV_ERR_INVALID_ARG;
    }
    size_t n = strlen(tun->name);
    if (n + 1 > cap) {
        return LV_ERR_TOO_BIG;
    }
    memcpy(buf, tun->name, n + 1);
    return LV_OK;
}

void lv_tun_close(lv_tun_t *tun) {
    if (tun == NULL) {
        return;
    }
    lv_platform_close(tun);
    lv_ring_destroy(&tun->ring);
    free(tun);
}
