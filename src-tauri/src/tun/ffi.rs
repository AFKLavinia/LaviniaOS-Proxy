//! Raw C-ABI declarations mirroring `src/c/include/lavinia_tun.h`.
use std::os::raw::c_char;

#[repr(C)]
pub struct LvTun {
    _private: [u8; 0],
}

pub const LV_OK: i32 = 0;
pub const LV_ERR_INVALID_ARG: i32 = -1;
pub const LV_ERR_OPEN: i32 = -2;
pub const LV_ERR_PRIVILEGE: i32 = -3;
pub const LV_ERR_NOMEM: i32 = -4;
pub const LV_ERR_IO: i32 = -5;
pub const LV_ERR_TOO_BIG: i32 = -6;
pub const LV_ERR_DRIVER: i32 = -7;

extern "C" {
    pub fn lv_tun_open(name: *const c_char, ring_slots: u32, out: *mut *mut LvTun) -> i32;
    pub fn lv_tun_read(tun: *mut LvTun, buf: *mut u8, cap: usize) -> i32;
    pub fn lv_tun_write(tun: *mut LvTun, pkt: *const u8, len: usize) -> i32;
    pub fn lv_tun_dropped(tun: *const LvTun) -> u64;
    pub fn lv_tun_name(tun: *const LvTun, buf: *mut c_char, cap: usize) -> i32;
    pub fn lv_tun_close(tun: *mut LvTun);
}
