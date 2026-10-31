//! Linux adapter: NetworkManager checks, nftables transparent redirect, SO_ORIGINAL_DST.
mod autostart;
mod firewall;
mod network;
mod proc;

pub use autostart::{is_enabled as autostart_enabled, set_enabled as set_autostart};
pub use firewall::{apply_firewall, Guard};
use crate::platform::PlatformError;
pub use network::check_uplink;

use std::io;
use std::mem;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::os::fd::{AsRawFd, FromRawFd};

const SO_ORIGINAL_DST: libc::c_int = 80;

pub fn original_dst(stream: &tokio::net::TcpStream) -> io::Result<SocketAddrV4> {
    let mut addr = libc::sockaddr_in {
        sin_family: 0,
        sin_port: 0,
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    let mut len = mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    // SAFETY: `addr`/`len` are valid, correctly sized out-parameters for this option; fd is live.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_IP,
            SO_ORIGINAL_DST,
            (&mut addr as *mut libc::sockaddr_in).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    if (len as usize) < mem::size_of::<libc::sockaddr_in>()
        || addr.sin_family != libc::AF_INET as libc::sa_family_t
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid original_dst address or family",
        ));
    }
    Ok(SocketAddrV4::new(
        Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr)),
        u16::from_be(addr.sin_port),
    ))
}

const IP_TRANSPARENT: libc::c_int = 19;
const IP_RECVORIGDSTADDR: libc::c_int = 20;

fn set_opt(fd: libc::c_int, level: libc::c_int, name: libc::c_int) -> io::Result<()> {
    let one: libc::c_int = 1;
    // SAFETY: `one` is a valid c_int for the given boolean socket option; fd is live.
    let rc = unsafe {
        libc::setsockopt(
            fd,
            level,
            name,
            (&one as *const libc::c_int).cast(),
            mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn sockaddr(a: SocketAddrV4) -> libc::sockaddr_in {
    libc::sockaddr_in {
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: a.port().to_be(),
        sin_addr: libc::in_addr { s_addr: u32::from(*a.ip()).to_be() },
        sin_zero: [0; 8],
    }
}

fn bound_udp(addr: SocketAddrV4, spoof: bool) -> io::Result<std::net::UdpSocket> {
    // SAFETY: plain socket(2) call; result is checked and then owned by `OwnedFd`.
    let raw = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a fresh, valid fd exclusively owned here.
    let owned = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) };
    let fd = owned.as_raw_fd();
    set_opt(fd, libc::SOL_IP, IP_TRANSPARENT)?;
    if spoof {
        set_opt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR)?;
        set_opt(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT)?;
    } else {
        set_opt(fd, libc::SOL_IP, IP_RECVORIGDSTADDR)?;
    }
    let sa = sockaddr(addr);
    // SAFETY: `sa` is a valid sockaddr_in and the length matches its size.
    let rc = unsafe {
        libc::bind(fd, (&sa as *const libc::sockaddr_in).cast(), mem::size_of::<libc::sockaddr_in>() as libc::socklen_t)
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(std::net::UdpSocket::from(owned))
}

/// Wildcard UDP socket receiving TPROXY-diverted datagrams with their original destination.
pub fn udp_intercept_socket() -> io::Result<std::net::UdpSocket> {
    bound_udp(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0), false)
}

/// UDP socket bound to a remote address so replies appear to originate from it (needs CAP_NET_ADMIN).
pub fn udp_spoof_socket(src: SocketAddrV4) -> io::Result<std::net::UdpSocket> {
    bound_udp(src, true)
}

/// Receives one datagram: (len, client source, original destination).
pub async fn recv_udp_orig(
    sock: &tokio::net::UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddrV4, SocketAddrV4)> {
    sock.async_io(tokio::io::Interest::READABLE, || recv_once(sock.as_raw_fd(), buf)).await
}

fn recv_once(fd: libc::c_int, buf: &mut [u8]) -> io::Result<(usize, SocketAddrV4, SocketAddrV4)> {
    // SAFETY: zeroed sockaddr_in/msghdr are valid initial states for recvmsg out-parameters.
    let mut src: libc::sockaddr_in = unsafe { mem::zeroed() };
    let mut ctrl = [0u8; 128];
    let mut iov = libc::iovec { iov_base: buf.as_mut_ptr().cast(), iov_len: buf.len() };
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_name = (&mut src as *mut libc::sockaddr_in).cast();
    msg.msg_namelen = mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = ctrl.as_mut_ptr().cast();
    msg.msg_controllen = ctrl.len() as _;
    // SAFETY: msg points at live buffers sized as declared above.
    let n = unsafe { libc::recvmsg(fd, &mut msg, 0) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    if (msg.msg_flags & libc::MSG_CTRUNC) != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "cmsg truncated"));
    }
    if (msg.msg_namelen as usize) < mem::size_of::<libc::sockaddr_in>()
        || src.sin_family != libc::AF_INET as libc::sa_family_t
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid source address family or length",
        ));
    }
    let mut dst = None;
    // SAFETY: CMSG_* walk only the control buffer the kernel just filled in `msg`.
    unsafe {
        let mut c = libc::CMSG_FIRSTHDR(&msg);
        let min_cmsg_len = libc::CMSG_LEN(mem::size_of::<libc::sockaddr_in>() as u32) as usize;
        while !c.is_null() {
            if (*c).cmsg_level == libc::SOL_IP
                && (*c).cmsg_type == IP_RECVORIGDSTADDR
                && ((*c).cmsg_len as usize) >= min_cmsg_len
            {
                let a = std::ptr::read_unaligned(libc::CMSG_DATA(c) as *const libc::sockaddr_in);
                if a.sin_family == libc::AF_INET as libc::sa_family_t {
                    dst = Some(SocketAddrV4::new(
                        Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)),
                        u16::from_be(a.sin_port),
                    ));
                }
            }
            c = libc::CMSG_NXTHDR(&msg, c);
        }
    }
    let dst = dst.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing original destination"))?;
    let src = SocketAddrV4::new(Ipv4Addr::from(u32::from_be(src.sin_addr.s_addr)), u16::from_be(src.sin_port));
    Ok((n as usize, src, dst))
}

/// Grants CAP_NET_ADMIN to the running executable through polkit (credentials are asked by the system prompt).
pub fn grant_net_admin() -> Result<(), PlatformError> {
    let exe = std::env::current_exe()?;
    let path = exe.to_str().ok_or_else(|| {
        PlatformError::Command("executable path is not valid UTF-8".into())
    })?;
    if path.contains(['\0', '\n']) {
        return Err(PlatformError::Command("executable path contains unsafe characters".into()));
    }
    let escaped = path.replace('\'', "'\\''");
    proc::privileged_sh(&format!("setcap cap_net_admin+ep '{escaped}'"))
}
