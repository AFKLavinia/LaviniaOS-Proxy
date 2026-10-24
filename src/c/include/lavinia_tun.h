#ifndef LAVINIA_TUN_H
#define LAVINIA_TUN_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LV_TUN_MAX_PACKET 2048u
#define LV_TUN_NAME_MAX 15u

typedef enum lv_status {
    LV_OK = 0,
    LV_ERR_INVALID_ARG = -1,
    LV_ERR_OPEN = -2,       /* device/driver could not be opened */
    LV_ERR_PRIVILEGE = -3,  /* insufficient privileges */
    LV_ERR_NOMEM = -4,
    LV_ERR_IO = -5,
    LV_ERR_TOO_BIG = -6,    /* packet exceeds LV_TUN_MAX_PACKET or caller buffer */
    LV_ERR_DRIVER = -7      /* wintun.dll missing or incompatible */
} lv_status_t;

typedef struct lv_tun lv_tun_t;

/* name: NUL-terminated adapter name (<= LV_TUN_NAME_MAX chars).
 * ring_slots: power of two, >= 2. Opaque handle returned via out. */
int32_t lv_tun_open(const char *name, uint32_t ring_slots, lv_tun_t **out);

/* Non-blocking pop of one raw L3 IP packet from RX ring.
 * Returns packet length (>0), 0 when empty, or negative lv_status_t. */
int32_t lv_tun_read(lv_tun_t *tun, uint8_t *buf, size_t cap);

/* Injects one raw L3 IP packet into the interface. Returns LV_OK or error. */
int32_t lv_tun_write(lv_tun_t *tun, const uint8_t *pkt, size_t len);

/* Number of packets dropped because the RX ring was full. */
uint64_t lv_tun_dropped(const lv_tun_t *tun);

/* Copies the OS-assigned interface name into buf (NUL-terminated). */
int32_t lv_tun_name(const lv_tun_t *tun, char *buf, size_t cap);

void lv_tun_close(lv_tun_t *tun);

#ifdef __cplusplus
}
#endif

#endif
