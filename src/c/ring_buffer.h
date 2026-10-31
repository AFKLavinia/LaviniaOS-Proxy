#ifndef LAVINIA_RING_BUFFER_H
#define LAVINIA_RING_BUFFER_H

#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

#include "lavinia_tun.h"

/* Lock-free SPSC ring of fixed-size packet slots (single allocation). */
typedef struct lv_slot {
    uint32_t len;
    uint8_t data[LV_TUN_MAX_PACKET];
} lv_slot_t;

typedef struct lv_ring {
    /* Read-mostly configuration fields */
    lv_slot_t *slots;
    uint32_t mask;
    uint8_t _pad0[64 - sizeof(lv_slot_t *) - sizeof(uint32_t)];

    /* Consumer index: isolated on its own 64-byte cache line to eliminate false sharing */
    _Atomic uint32_t head;
    uint8_t _pad1[64 - sizeof(_Atomic uint32_t)];

    /* Producer index: isolated on its own 64-byte cache line to eliminate false sharing */
    _Atomic uint32_t tail;
    uint8_t _pad2[64 - sizeof(_Atomic uint32_t)];

    /* Dropped packet counter: isolated on its own 64-byte cache line */
    _Atomic uint64_t dropped;
    uint8_t _pad3[64 - sizeof(_Atomic uint64_t)];
} lv_ring_t;

int lv_ring_init(lv_ring_t *r, uint32_t slots);
void lv_ring_destroy(lv_ring_t *r);

/* Producer: zero-copy slot to fill, or NULL when full (counts a drop). */
lv_slot_t *lv_ring_acquire(lv_ring_t *r);
void lv_ring_commit(lv_ring_t *r, uint32_t len);

/* Consumer: returns len, 0 if empty, LV_ERR_TOO_BIG if cap too small (bad slot dropped). */
int32_t lv_ring_pop(lv_ring_t *r, uint8_t *buf, size_t cap);

#endif
