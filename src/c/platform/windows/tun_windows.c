#include <windows.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "../platform.h"

#define WINTUN_DLL L"wintun.dll"
#define WINTUN_RING_CAPACITY 0x400000u /* 4 MiB */
#define PUMP_WAIT_MS 100u
#define TUNNEL_TYPE L"Lavinia"

typedef void *WINTUN_ADAPTER_HANDLE;
typedef void *WINTUN_SESSION_HANDLE;

typedef WINTUN_ADAPTER_HANDLE(WINAPI *WINTUN_CREATE_ADAPTER_FUNC)(LPCWSTR, LPCWSTR, const GUID *);
typedef WINTUN_ADAPTER_HANDLE(WINAPI *WINTUN_OPEN_ADAPTER_FUNC)(LPCWSTR);
typedef void(WINAPI *WINTUN_CLOSE_ADAPTER_FUNC)(WINTUN_ADAPTER_HANDLE);
typedef WINTUN_SESSION_HANDLE(WINAPI *WINTUN_START_SESSION_FUNC)(WINTUN_ADAPTER_HANDLE, DWORD);
typedef void(WINAPI *WINTUN_END_SESSION_FUNC)(WINTUN_SESSION_HANDLE);
typedef HANDLE(WINAPI *WINTUN_GET_READ_WAIT_EVENT_FUNC)(WINTUN_SESSION_HANDLE);
typedef BYTE *(WINAPI *WINTUN_RECEIVE_PACKET_FUNC)(WINTUN_SESSION_HANDLE, DWORD *);
typedef void(WINAPI *WINTUN_RELEASE_RECEIVE_PACKET_FUNC)(WINTUN_SESSION_HANDLE, const BYTE *);
typedef BYTE *(WINAPI *WINTUN_ALLOCATE_SEND_PACKET_FUNC)(WINTUN_SESSION_HANDLE, DWORD);
typedef void(WINAPI *WINTUN_SEND_PACKET_FUNC)(WINTUN_SESSION_HANDLE, const BYTE *);

struct win_plat {
    HMODULE dll;
    WINTUN_ADAPTER_HANDLE adapter;
    WINTUN_SESSION_HANDLE session;
    HANDLE pump;
    atomic_bool running;
    WINTUN_CREATE_ADAPTER_FUNC create;
    WINTUN_OPEN_ADAPTER_FUNC open;
    WINTUN_CLOSE_ADAPTER_FUNC close_adapter;
    WINTUN_START_SESSION_FUNC start;
    WINTUN_END_SESSION_FUNC end;
    WINTUN_GET_READ_WAIT_EVENT_FUNC event;
    WINTUN_RECEIVE_PACKET_FUNC recv;
    WINTUN_RELEASE_RECEIVE_PACKET_FUNC release;
    WINTUN_ALLOCATE_SEND_PACKET_FUNC alloc;
    WINTUN_SEND_PACKET_FUNC send;
};

static DWORD WINAPI pump_main(LPVOID arg) {
    struct lv_tun *tun = (struct lv_tun *)arg;
    struct win_plat *p = (struct win_plat *)tun->plat;
    if (p == NULL || p->session == NULL) {
        return 0;
    }
    WINTUN_SESSION_HANDLE session = p->session;
    HANDLE ev = p->event(session);

    while (atomic_load_explicit(&p->running, memory_order_acquire)) {
        DWORD len = 0;
        BYTE *pkt = p->recv(session, &len);
        if (pkt == NULL) {
            if (GetLastError() == ERROR_NO_MORE_ITEMS) {
                WaitForSingleObject(ev, PUMP_WAIT_MS);
                continue;
            }
            break;
        }
        if (len > LV_TUN_MAX_PACKET) {
            atomic_fetch_add_explicit(&tun->ring.dropped, 1u, memory_order_relaxed);
        } else {
            lv_slot_t *slot = lv_ring_acquire(&tun->ring);
            if (slot != NULL) {
                memcpy(slot->data, pkt, len);
                lv_ring_commit(&tun->ring, len);
            }
            /* If slot == NULL, lv_ring_acquire already incremented dropped */
        }
        p->release(session, pkt);
    }
    return 0;
}

static void teardown(struct win_plat *p) {
    if (p == NULL) {
        return;
    }
    if (p->session) {
        p->end(p->session);
        p->session = NULL;
    }
    if (p->adapter) {
        p->close_adapter(p->adapter);
        p->adapter = NULL;
    }
    if (p->dll) {
        FreeLibrary(p->dll);
        p->dll = NULL;
    }
    free(p);
}

int32_t lv_platform_open(struct lv_tun *tun, const char *name) {
    if (tun == NULL || name == NULL) {
        return LV_ERR_INVALID_ARG;
    }
    size_t name_len = strlen(name);
    if (name_len == 0 || name_len > LV_TUN_NAME_MAX) {
        return LV_ERR_INVALID_ARG;
    }

    struct win_plat *p = (struct win_plat *)calloc(1, sizeof(*p));
    if (p == NULL) {
        return LV_ERR_NOMEM;
    }
    p->dll = LoadLibraryExW(WINTUN_DLL, NULL, LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (p->dll == NULL) {
        free(p);
        return LV_ERR_DRIVER;
    }
#define LOAD(field, sym) (p->field = (void *)GetProcAddress(p->dll, sym))
    if (!LOAD(create, "WintunCreateAdapter") || !LOAD(open, "WintunOpenAdapter") ||
        !LOAD(close_adapter, "WintunCloseAdapter") ||
        !LOAD(start, "WintunStartSession") || !LOAD(end, "WintunEndSession") ||
        !LOAD(event, "WintunGetReadWaitEvent") || !LOAD(recv, "WintunReceivePacket") ||
        !LOAD(release, "WintunReleaseReceivePacket") || !LOAD(alloc, "WintunAllocateSendPacket") ||
        !LOAD(send, "WintunSendPacket")) {
        teardown(p);
        return LV_ERR_DRIVER;
    }
#undef LOAD

    WCHAR wname[LV_TUN_NAME_MAX + 1u];
    if (MultiByteToWideChar(CP_UTF8, 0, name, -1, wname, LV_TUN_NAME_MAX + 1u) == 0) {
        teardown(p);
        return LV_ERR_INVALID_ARG;
    }
    p->adapter = p->create(wname, TUNNEL_TYPE, NULL);
    if (p->adapter == NULL) {
        DWORD create_err = GetLastError();
        p->adapter = p->open(wname);
        if (p->adapter == NULL) {
            DWORD e = (create_err == ERROR_ACCESS_DENIED || GetLastError() == ERROR_ACCESS_DENIED)
                          ? ERROR_ACCESS_DENIED
                          : create_err;
            teardown(p);
            return e == ERROR_ACCESS_DENIED ? LV_ERR_PRIVILEGE : LV_ERR_OPEN;
        }
    }
    p->session = p->start(p->adapter, WINTUN_RING_CAPACITY);
    if (p->session == NULL) {
        teardown(p);
        return LV_ERR_OPEN;
    }
    memcpy(tun->name, name, name_len);
    tun->name[name_len] = '\0';

    tun->plat = p;
    atomic_init(&p->running, 1);
    p->pump = CreateThread(NULL, 0, pump_main, tun, 0, NULL);
    if (p->pump == NULL) {
        tun->plat = NULL;
        teardown(p);
        return LV_ERR_IO;
    }
    return LV_OK;
}

int32_t lv_platform_write(struct lv_tun *tun, const uint8_t *pkt, size_t len) {
    if (tun == NULL || pkt == NULL || len == 0) {
        return LV_ERR_INVALID_ARG;
    }
    struct win_plat *p = (struct win_plat *)tun->plat;
    if (p == NULL || !atomic_load_explicit(&p->running, memory_order_acquire) || p->session == NULL) {
        return LV_ERR_IO;
    }
    BYTE *out = p->alloc(p->session, (DWORD)len);
    if (out == NULL) {
        return LV_ERR_IO;
    }
    memcpy(out, pkt, len);
    p->send(p->session, out);
    return LV_OK;
}

void lv_platform_close(struct lv_tun *tun) {
    if (tun == NULL) {
        return;
    }
    struct win_plat *p = (struct win_plat *)tun->plat;
    if (p == NULL) {
        return;
    }
    atomic_store_explicit(&p->running, 0, memory_order_release);
    if (p->session) {
        p->end(p->session);
        p->session = NULL;
    }
    if (p->pump) {
        WaitForSingleObject(p->pump, INFINITE);
        CloseHandle(p->pump);
        p->pump = NULL;
    }
    tun->plat = NULL;
    teardown(p);
}
