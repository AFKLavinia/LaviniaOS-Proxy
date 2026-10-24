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

typedef WINTUN_ADAPTER_HANDLE(WINAPI *fn_create)(LPCWSTR, LPCWSTR, const GUID *);
typedef void(WINAPI *fn_close_adapter)(WINTUN_ADAPTER_HANDLE);
typedef WINTUN_SESSION_HANDLE(WINAPI *fn_start)(WINTUN_ADAPTER_HANDLE, DWORD);
typedef void(WINAPI *fn_end)(WINTUN_SESSION_HANDLE);
typedef HANDLE(WINAPI *fn_event)(WINTUN_SESSION_HANDLE);
typedef BYTE *(WINAPI *fn_recv)(WINTUN_SESSION_HANDLE, DWORD *);
typedef void(WINAPI *fn_release)(WINTUN_SESSION_HANDLE, const BYTE *);
typedef BYTE *(WINAPI *fn_alloc)(WINTUN_SESSION_HANDLE, DWORD);
typedef void(WINAPI *fn_send)(WINTUN_SESSION_HANDLE, const BYTE *);

struct win_plat {
    HMODULE dll;
    WINTUN_ADAPTER_HANDLE adapter;
    WINTUN_SESSION_HANDLE session;
    HANDLE pump;
    atomic_bool running;
    fn_create create;
    fn_close_adapter close_adapter;
    fn_start start;
    fn_end end;
    fn_event event;
    fn_recv recv;
    fn_release release;
    fn_alloc alloc;
    fn_send send;
};

static DWORD WINAPI pump_main(LPVOID arg) {
    struct lv_tun *tun = (struct lv_tun *)arg;
    struct win_plat *p = (struct win_plat *)tun->plat;
    HANDLE ev = p->event(p->session);

    while (atomic_load_explicit(&p->running, memory_order_acquire)) {
        DWORD len = 0;
        BYTE *pkt = p->recv(p->session, &len);
        if (pkt == NULL) {
            if (GetLastError() == ERROR_NO_MORE_ITEMS) {
                WaitForSingleObject(ev, PUMP_WAIT_MS);
                continue;
            }
            break;
        }
        lv_slot_t *slot = lv_ring_acquire(&tun->ring);
        if (slot != NULL && len <= LV_TUN_MAX_PACKET) {
            memcpy(slot->data, pkt, len);
            lv_ring_commit(&tun->ring, len);
        }
        p->release(p->session, pkt);
    }
    return 0;
}

static void teardown(struct win_plat *p) {
    if (p->session) p->end(p->session);
    if (p->adapter) p->close_adapter(p->adapter);
    if (p->dll) FreeLibrary(p->dll);
    free(p);
}

int32_t lv_platform_open(struct lv_tun *tun, const char *name) {
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
    if (!LOAD(create, "WintunCreateAdapter") || !LOAD(close_adapter, "WintunCloseAdapter") ||
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
        DWORD e = GetLastError();
        teardown(p);
        return e == ERROR_ACCESS_DENIED ? LV_ERR_PRIVILEGE : LV_ERR_OPEN;
    }
    p->session = p->start(p->adapter, WINTUN_RING_CAPACITY);
    if (p->session == NULL) {
        teardown(p);
        return LV_ERR_OPEN;
    }
    strncpy(tun->name, name, LV_TUN_NAME_MAX);
    tun->name[LV_TUN_NAME_MAX] = '\0';

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
    struct win_plat *p = (struct win_plat *)tun->plat;
    BYTE *out = p->alloc(p->session, (DWORD)len);
    if (out == NULL) {
        return LV_ERR_IO;
    }
    memcpy(out, pkt, len);
    p->send(p->session, out);
    return LV_OK;
}

void lv_platform_close(struct lv_tun *tun) {
    struct win_plat *p = (struct win_plat *)tun->plat;
    if (p == NULL) {
        return;
    }
    atomic_store_explicit(&p->running, 0, memory_order_release);
    WaitForSingleObject(p->pump, INFINITE);
    CloseHandle(p->pump);
    tun->plat = NULL;
    teardown(p);
}
