//! Safe wrapper over the C TUN driver. Public entry point: [`TunDevice`].
mod ffi;

use std::ffi::CString;
use std::fmt;
use std::os::raw::c_char;
use std::ptr::NonNull;
use std::sync::Mutex;

pub const MAX_PACKET: usize = 2048;
pub const DEFAULT_RING_SLOTS: u32 = 4096;
pub const NAME_MAX: usize = 63;
pub const NAME_BUF: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunError {
    InvalidArgument,
    Open,
    Privilege,
    OutOfMemory,
    Io,
    PacketTooBig,
    BufferTooSmall,
    Driver,
    InvalidUtf8,
    Unknown(i32),
}

impl TunError {
    fn from_code(code: i32) -> Self {
        match code {
            ffi::LV_ERR_INVALID_ARG => Self::InvalidArgument,
            ffi::LV_ERR_OPEN => Self::Open,
            ffi::LV_ERR_PRIVILEGE => Self::Privilege,
            ffi::LV_ERR_NOMEM => Self::OutOfMemory,
            ffi::LV_ERR_IO => Self::Io,
            ffi::LV_ERR_TOO_BIG => Self::PacketTooBig,
            ffi::LV_ERR_DRIVER => Self::Driver,
            other => Self::Unknown(other),
        }
    }
}

impl fmt::Display for TunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => f.write_str("invalid argument"),
            Self::Open => f.write_str("failed to open TUN device"),
            Self::Privilege => f.write_str("insufficient privileges for TUN device"),
            Self::OutOfMemory => f.write_str("out of memory"),
            Self::Io => f.write_str("TUN I/O error"),
            Self::PacketTooBig => f.write_str("packet exceeds size limit"),
            Self::BufferTooSmall => f.write_str("buffer too small"),
            Self::Driver => f.write_str("TUN driver unavailable or incompatible"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8 string"),
            Self::Unknown(c) => write!(f, "unknown TUN error code {c}"),
        }
    }
}

impl std::error::Error for TunError {}

pub struct TunDevice {
    raw: NonNull<ffi::LvTun>,
    write_lock: Mutex<()>,
}

// The C ring is SPSC: reads require exclusive access (`&mut self`), while writes
// are protected by `write_lock` (`Mutex<()>`). `TunDevice` is safe to send between
// threads and share across threads for full-duplex operation.
unsafe impl Send for TunDevice {}
unsafe impl Sync for TunDevice {}

impl TunDevice {
    pub fn open(name: &str, ring_slots: u32) -> Result<Self, TunError> {
        if name.len() > NAME_MAX {
            return Err(TunError::InvalidArgument);
        }
        let cname = CString::new(name).map_err(|_| TunError::InvalidArgument)?;
        let mut out: *mut ffi::LvTun = std::ptr::null_mut();
        // SAFETY: `cname` is NUL-terminated and `out` is a valid out-pointer.
        let rc = unsafe { ffi::lv_tun_open(cname.as_ptr(), ring_slots, &mut out) };
        if rc != ffi::LV_OK {
            return Err(TunError::from_code(rc));
        }
        NonNull::new(out)
            .map(|raw| Self {
                raw,
                write_lock: Mutex::new(()),
            })
            .ok_or(TunError::Open)
    }

    /// Non-blocking read of one raw IP packet; `Ok(None)` when no packet is queued.
    pub fn read_packet(&mut self, buf: &mut [u8]) -> Result<Option<usize>, TunError> {
        // SAFETY: handle is live; buf pointer/len describe a valid writable slice.
        let rc = unsafe { ffi::lv_tun_read(self.raw.as_ptr(), buf.as_mut_ptr(), buf.len()) };
        match rc {
            0 => Ok(None),
            n if n > 0 => Ok(Some(n as usize)),
            ffi::LV_ERR_TOO_BIG => Err(TunError::BufferTooSmall),
            e => Err(TunError::from_code(e)),
        }
    }

    pub fn write_packet(&self, pkt: &[u8]) -> Result<(), TunError> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: handle is live; pkt describes a valid readable slice.
        let rc = unsafe { ffi::lv_tun_write(self.raw.as_ptr(), pkt.as_ptr(), pkt.len()) };
        if rc == ffi::LV_OK {
            Ok(())
        } else {
            Err(TunError::from_code(rc))
        }
    }

    pub fn dropped_packets(&self) -> u64 {
        // SAFETY: handle is live.
        unsafe { ffi::lv_tun_dropped(self.raw.as_ptr()) }
    }

    pub fn name(&self) -> Result<String, TunError> {
        let mut buf = [0u8; NAME_BUF];
        // SAFETY: buf is writable for NAME_BUF (64) bytes; C side writes at most NAME_BUF bytes.
        let rc = unsafe {
            ffi::lv_tun_name(self.raw.as_ptr(), buf.as_mut_ptr() as *mut c_char, buf.len())
        };
        if rc != ffi::LV_OK {
            if rc == ffi::LV_ERR_TOO_BIG {
                return Err(TunError::BufferTooSmall);
            }
            return Err(TunError::from_code(rc));
        }
        let nul_pos = buf.iter().position(|&b| b == 0).ok_or(TunError::BufferTooSmall)?;
        let s = std::str::from_utf8(&buf[..nul_pos]).map_err(|_| TunError::InvalidUtf8)?;
        Ok(s.to_owned())
    }
}

impl Drop for TunDevice {
    fn drop(&mut self) {
        // SAFETY: handle is live and owned exclusively; never used after this.
        unsafe { ffi::lv_tun_close(self.raw.as_ptr()) }
    }
}
