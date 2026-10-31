#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/if.h>
#include <linux/if_tun.h>
#include <poll.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "../platform.h"

#define TUN_DEV_PATH "/dev/net/tun"
#define PUMP_POLL_TIMEOUT_MS 100
#define LV_LINUX_MAX_PACKET 65536u
#define LV_WRITE_MAX_RETRIES 5

struct linux_plat {
    int fd;
    pthread_t pump;
    atomic_bool running;
};

static void *pump_main(void *arg) {
    struct lv_tun *tun = (struct lv_tun *)arg;
    struct linux_plat *p = (struct linux_plat *)tun->plat;
    struct pollfd pfd = {.fd = p->fd, .events = POLLIN};
    uint8_t rx_buf[LV_LINUX_MAX_PACKET];

    while (atomic_load_explicit(&p->running, memory_order_acquire)) {
        int pr = poll(&pfd, 1, PUMP_POLL_TIMEOUT_MS);
        if (pr < 0) {
            if (errno == EINTR) {
                continue;
            }
            break;
        }
        if (pfd.revents & (POLLERR | POLLHUP | POLLNVAL)) {
            break;
        }
        if (pr == 0 || !(pfd.revents & POLLIN)) {
            continue;
        }

        ssize_t n = read(p->fd, rx_buf, sizeof(rx_buf));
        if (n > 0) {
            if ((size_t)n > LV_TUN_MAX_PACKET) {
                atomic_fetch_add_explicit(&tun->ring.dropped, 1u, memory_order_relaxed);
            } else {
                lv_slot_t *slot = lv_ring_acquire(&tun->ring);
                if (slot != NULL) {
                    memcpy(slot->data, rx_buf, (size_t)n);
                    lv_ring_commit(&tun->ring, (uint32_t)n);
                }
                /* If slot == NULL, lv_ring_acquire already incremented dropped */
            }
        } else if (n == 0) {
            /* EOF on tun fd */
            break;
        } else if (errno != EAGAIN && errno != EINTR) {
            break;
        }
    }
    return NULL;
}

int32_t lv_platform_open(struct lv_tun *tun, const char *name) {
    if (tun == NULL || name == NULL) {
        return LV_ERR_INVALID_ARG;
    }
    size_t name_len = strlen(name);
    if (name_len == 0 || name_len >= IFNAMSIZ || name_len > LV_TUN_NAME_MAX) {
        return LV_ERR_INVALID_ARG;
    }

    struct linux_plat *p = (struct linux_plat *)calloc(1, sizeof(*p));
    if (p == NULL) {
        return LV_ERR_NOMEM;
    }
    p->fd = open(TUN_DEV_PATH, O_RDWR | O_CLOEXEC | O_NONBLOCK);
    if (p->fd < 0) {
        int e = errno;
        free(p);
        return (e == EACCES || e == EPERM) ? LV_ERR_PRIVILEGE : LV_ERR_OPEN;
    }

    struct ifreq ifr;
    memset(&ifr, 0, sizeof(ifr));
    ifr.ifr_flags = IFF_TUN | IFF_NO_PI;
    memcpy(ifr.ifr_name, name, name_len);
    ifr.ifr_name[name_len] = '\0';

    if (ioctl(p->fd, TUNSETIFF, &ifr) < 0) {
        int e = errno;
        close(p->fd);
        free(p);
        return (e == EACCES || e == EPERM) ? LV_ERR_PRIVILEGE : LV_ERR_OPEN;
    }

    size_t assigned_len = strnlen(ifr.ifr_name, IFNAMSIZ);
    if (assigned_len >= IFNAMSIZ || assigned_len > LV_TUN_NAME_MAX) {
        close(p->fd);
        free(p);
        return LV_ERR_INVALID_ARG;
    }
    memcpy(tun->name, ifr.ifr_name, assigned_len);
    tun->name[assigned_len] = '\0';
    if (strlen(tun->name) >= IFNAMSIZ) {
        close(p->fd);
        free(p);
        return LV_ERR_INVALID_ARG;
    }

    tun->plat = p;
    atomic_init(&p->running, 1);
    if (pthread_create(&p->pump, NULL, pump_main, tun) != 0) {
        close(p->fd);
        free(p);
        tun->plat = NULL;
        return LV_ERR_IO;
    }
    return LV_OK;
}

int32_t lv_platform_write(struct lv_tun *tun, const uint8_t *pkt, size_t len) {
    if (tun == NULL || pkt == NULL || len == 0) {
        return LV_ERR_INVALID_ARG;
    }
    struct linux_plat *p = (struct linux_plat *)tun->plat;
    if (!p || !atomic_load_explicit(&p->running, memory_order_acquire)) {
        return LV_ERR_IO;
    }

    int retries = 0;
    while (atomic_load_explicit(&p->running, memory_order_acquire)) {
        ssize_t n = write(p->fd, pkt, len);
        if (n == (ssize_t)len) {
            return LV_OK;
        }
        if (n < 0 && errno == EINTR) {
            continue;
        }
        if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            if (++retries > LV_WRITE_MAX_RETRIES) {
                return LV_ERR_IO;
            }
            struct pollfd pfd = {.fd = p->fd, .events = POLLOUT};
            (void)poll(&pfd, 1, PUMP_POLL_TIMEOUT_MS);
            continue;
        }
        return LV_ERR_IO;
    }
    return LV_ERR_IO;
}

void lv_platform_close(struct lv_tun *tun) {
    if (tun == NULL) {
        return;
    }
    struct linux_plat *p = (struct linux_plat *)tun->plat;
    if (p == NULL) {
        return;
    }
    atomic_store_explicit(&p->running, 0, memory_order_release);
    pthread_join(p->pump, NULL);
    close(p->fd);
    free(p);
    tun->plat = NULL;
}
