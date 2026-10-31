#include "ring_buffer.h"

#include <stdlib.h>
#include <string.h>

int lv_ring_init(lv_ring_t *r, uint32_t slots) {
    if (r == NULL || slots < 2u || slots > 65536u || (slots & (slots - 1u)) != 0u) {
        return LV_ERR_INVALID_ARG;
    }
    if ((size_t)slots > SIZE_MAX / sizeof(lv_slot_t)) {
        return LV_ERR_INVALID_ARG;
    }
    memset(r, 0, sizeof(*r));
    r->slots = (lv_slot_t *)calloc(slots, sizeof(lv_slot_t));
    if (r->slots == NULL) {
        return LV_ERR_NOMEM;
    }
    r->mask = slots - 1u;
    atomic_init(&r->head, 0u);
    atomic_init(&r->tail, 0u);
    atomic_init(&r->dropped, 0u);
    return LV_OK;
}

void lv_ring_destroy(lv_ring_t *r) {
    if (r != NULL) {
        free(r->slots);
        r->slots = NULL;
        r->mask = 0u;
    }
}

lv_slot_t *lv_ring_acquire(lv_ring_t *r) {
    if (r == NULL || r->slots == NULL) {
        return NULL;
    }
    uint32_t tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
    uint32_t head = atomic_load_explicit(&r->head, memory_order_acquire);
    if ((tail - head) > r->mask) {
        atomic_fetch_add_explicit(&r->dropped, 1u, memory_order_relaxed);
        return NULL;
    }
    return &r->slots[tail & r->mask];
}

void lv_ring_commit(lv_ring_t *r, uint32_t len) {
    if (r == NULL || r->slots == NULL) {
        return;
    }
    uint32_t tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
    r->slots[tail & r->mask].len = len;
    atomic_store_explicit(&r->tail, tail + 1u, memory_order_release);
}

int32_t lv_ring_pop(lv_ring_t *r, uint8_t *buf, size_t cap) {
    if (r == NULL || r->slots == NULL || (buf == NULL && cap > 0u)) {
        return LV_ERR_INVALID_ARG;
    }
    uint32_t head = atomic_load_explicit(&r->head, memory_order_relaxed);
    uint32_t tail = atomic_load_explicit(&r->tail, memory_order_acquire);
    if (head == tail) {
        return 0;
    }
    const lv_slot_t *s = &r->slots[head & r->mask];
    if (s->len > cap) {
        /* Drop oversized packet and advance head to avoid infinite head-of-line blocking */
        atomic_store_explicit(&r->head, head + 1u, memory_order_release);
        atomic_fetch_add_explicit(&r->dropped, 1u, memory_order_relaxed);
        return LV_ERR_TOO_BIG;
    }
    memcpy(buf, s->data, s->len);
    atomic_store_explicit(&r->head, head + 1u, memory_order_release);
    return (int32_t)s->len;
}
