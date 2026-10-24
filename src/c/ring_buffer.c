#include "ring_buffer.h"

#include <stdlib.h>
#include <string.h>

int lv_ring_init(lv_ring_t *r, uint32_t slots) {
    if (r == NULL || slots < 2u || (slots & (slots - 1u)) != 0u) {
        return LV_ERR_INVALID_ARG;
    }
    r->slots = (lv_slot_t *)calloc(slots, sizeof(lv_slot_t));
    if (r->slots == NULL) {
        return LV_ERR_NOMEM;
    }
    r->mask = slots - 1u;
    atomic_init(&r->head, 0);
    atomic_init(&r->tail, 0);
    atomic_init(&r->dropped, 0);
    return LV_OK;
}

void lv_ring_destroy(lv_ring_t *r) {
    if (r != NULL) {
        free(r->slots);
        r->slots = NULL;
    }
}

lv_slot_t *lv_ring_acquire(lv_ring_t *r) {
    uint_fast32_t tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
    uint_fast32_t head = atomic_load_explicit(&r->head, memory_order_acquire);
    if (tail - head > r->mask) {
        atomic_fetch_add_explicit(&r->dropped, 1, memory_order_relaxed);
        return NULL;
    }
    return &r->slots[tail & r->mask];
}

void lv_ring_commit(lv_ring_t *r, uint32_t len) {
    uint_fast32_t tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
    r->slots[tail & r->mask].len = len;
    atomic_store_explicit(&r->tail, tail + 1, memory_order_release);
}

int32_t lv_ring_pop(lv_ring_t *r, uint8_t *buf, size_t cap) {
    uint_fast32_t head = atomic_load_explicit(&r->head, memory_order_relaxed);
    uint_fast32_t tail = atomic_load_explicit(&r->tail, memory_order_acquire);
    if (head == tail) {
        return 0;
    }
    const lv_slot_t *s = &r->slots[head & r->mask];
    if (s->len > cap) {
        return LV_ERR_TOO_BIG;
    }
    memcpy(buf, s->data, s->len);
    atomic_store_explicit(&r->head, head + 1, memory_order_release);
    return (int32_t)s->len;
}
