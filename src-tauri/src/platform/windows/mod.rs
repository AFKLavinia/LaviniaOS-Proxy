//! Windows platform adapter: Wintun L3 engine routing, uplink check, autostart, and UAC elevation.
mod autostart;
mod firewall;
mod network;
mod proc;

pub use autostart::{is_enabled as autostart_enabled, set_enabled as set_autostart};
pub use firewall::{apply_firewall, Guard};
pub use network::check_uplink;

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use crate::platform::PlatformError;

/// Windows does not use Linux SO_ORIGINAL_DST; original destination is parsed from Wintun IP headers or socks metadata.
pub fn original_dst(_stream: &tokio::net::TcpStream) -> io::Result<SocketAddrV4> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "SO_ORIGINAL_DST is not supported on Windows; use Wintun IP header metadata",
    ))
}

fn unsupported<T>(feature: &str) -> io::Result<T> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("{feature} is not supported on Windows direct socket redirect"),
    ))
}

pub fn udp_intercept_socket() -> io::Result<std::net::UdpSocket> {
    // Windows UDP is handled via Wintun packet pump rather than Linux TPROXY socket options
    unsupported("TPROXY UDP intercept socket")
}

pub fn udp_spoof_socket(_src: SocketAddrV4) -> io::Result<std::net::UdpSocket> {
    // Windows does not have IP_TRANSPARENT socket option for userspace spoofing
    unsupported("IP_TRANSPARENT UDP spoof socket")
}

pub async fn recv_udp_orig(
    _sock: &tokio::net::UdpSocket,
    _buf: &mut [u8],
) -> io::Result<(usize, SocketAddrV4, SocketAddrV4)> {
    unsupported("IP_RECVORIGDSTADDR")
}

/// Triggers UAC prompt on Windows to verify administrative access.
pub fn grant_net_admin() -> Result<(), PlatformError> {
    proc::privileged_powershell("Write-Output 'Administrator privileges granted'")
}
