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

struct linux_plat {
    int fd;
    pthread_t pump;
    atomic_bool running;
};

static void *pump_main(void *arg) {
    struct lv_tun *tun = (struct lv_tun *)arg;
    struct linux_plat *p = (struct linux_plat *)tun->plat;
    struct pollfd pfd = {.fd = p->fd, .events = POLLIN};

    while (atomic_load_explicit(&p->running, memory_order_acquire)) {
        int pr = poll(&pfd, 1, PUMP_POLL_TIMEOUT_MS);
        if (pr < 0) {
            if (errno == EINTR) {
                continue;
            }
            break;
        }
        if (pr == 0 || !(pfd.revents & POLLIN)) {
            if (pfd.revents & (POLLERR | POLLHUP | POLLNVAL)) {
                break;
            }
            continue;
        }
        lv_slot_t *slot = lv_ring_acquire(&tun->ring);
        if (slot == NULL) {
            /* Ring full: drain and discard to apply backpressure by drop. */
            uint8_t sink[LV_TUN_MAX_PACKET];
            if (read(p->fd, sink, sizeof(sink)) < 0 && errno != EAGAIN && errno != EINTR) {
                break;
            }
            continue;
        }
        ssize_t n = read(p->fd, slot->data, LV_TUN_MAX_PACKET);
        if (n > 0) {
            lv_ring_commit(&tun->ring, (uint32_t)n);
        } else if (n < 0 && errno != EAGAIN && errno != EINTR) {
            break;
        }
    }
    return NULL;
}

int32_t lv_platform_open(struct lv_tun *tun, const char *name) {
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
    strncpy(ifr.ifr_name, name, IFNAMSIZ - 1);
    if (ioctl(p->fd, TUNSETIFF, &ifr) < 0) {
        int e = errno;
        close(p->fd);
        free(p);
        return (e == EACCES || e == EPERM) ? LV_ERR_PRIVILEGE : LV_ERR_OPEN;
    }
    strncpy(tun->name, ifr.ifr_name, LV_TUN_NAME_MAX);
    tun->name[LV_TUN_NAME_MAX] = '\0';

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
    struct linux_plat *p = (struct linux_plat *)tun->plat;
    for (;;) {
        ssize_t n = write(p->fd, pkt, len);
        if (n == (ssize_t)len) {
            return LV_OK;
        }
        if (n < 0 && errno == EINTR) {
            continue;
        }
        if (n < 0 && errno == EAGAIN) {
            struct pollfd pfd = {.fd = p->fd, .events = POLLOUT};
            (void)poll(&pfd, 1, PUMP_POLL_TIMEOUT_MS);
            continue;
        }
        return LV_ERR_IO;
    }
}

void lv_platform_close(struct lv_tun *tun) {
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
