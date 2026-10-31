//! OS abstraction layer. All `cfg(target_os)` gating lives here only.
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use serde::Serialize;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[derive(Debug)]
pub enum PlatformError {
    Unsupported(&'static str),
    Command(String),
    Unreachable(String),
    Io(io::Error),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(m) => write!(f, "unsupported on this platform: {m}"),
            Self::Command(m) | Self::Unreachable(m) => f.write_str(m),
            Self::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<io::Error> for PlatformError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Physical medium of the uplink that reaches the phone.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Wifi,
    Usb,
    Ethernet,
    Bluetooth,
    Cellular,
    Other,
}

/// Physical uplink that reaches the phone, validated through the network manager.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Uplink {
    pub device: String,
    pub kind: LinkKind,
    /// Connection profile name (Wi-Fi SSID / tethering profile).
    pub name: String,
    /// USB manufacturer + product string when the link is USB tethering.
    pub hardware: Option<String>,
}

pub struct FirewallSpec {
    pub phone: Ipv4Addr,
    pub tcp_port: u16,
    pub dns_port: u16,
    /// `None` keeps UDP blocked (no privilege or upstream without UDP support).
    pub udp_port: Option<u16>,
}

/// RAII handle for installed redirect/leak-prevention rules and their crash watchdog.
pub struct FirewallGuard(imp::Guard);

impl FirewallGuard {
    /// Signals the watchdog to remove the rules (idempotent).
    pub fn release(&mut self) {
        self.0.release();
    }
}

pub fn check_uplink(phone: Ipv4Addr) -> Result<Uplink, PlatformError> {
    imp::check_uplink(phone)
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<FirewallGuard, PlatformError> {
    imp::apply_firewall(spec).map(FirewallGuard)
}

/// Original (pre-redirect) destination of an intercepted TCP connection.
pub fn original_dst(stream: &tokio::net::TcpStream) -> io::Result<SocketAddrV4> {
    imp::original_dst(stream)
}

/// Socket receiving diverted UDP datagrams (original destination reported per datagram).
pub fn udp_intercept_socket() -> io::Result<std::net::UdpSocket> {
    imp::udp_intercept_socket()
}

/// Socket whose replies carry `src` as source address.
pub fn udp_spoof_socket(src: SocketAddrV4) -> io::Result<std::net::UdpSocket> {
    imp::udp_spoof_socket(src)
}

/// Receives one diverted datagram: (len, client source, original destination).
pub async fn recv_udp_orig(
    sock: &tokio::net::UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddrV4, SocketAddrV4)> {
    imp::recv_udp_orig(sock, buf).await
}

/// True when the process may create IP_TRANSPARENT sockets (required to spoof UDP replies).
pub fn udp_capable() -> bool {
    #[cfg(target_os = "linux")]
    {
        imp::udp_spoof_socket(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        imp::udp_capable()
    }
}

/// Elevates the running binary so UDP relaying works (Linux: file capability via polkit).
pub fn grant_net_admin() -> Result<(), PlatformError> {
    imp::grant_net_admin()
}

pub fn autostart_enabled() -> bool {
    imp::autostart_enabled()
}

/// Enables/disables launching the app at user login.
pub fn set_autostart(enabled: bool) -> Result<(), PlatformError> {
    imp::set_autostart(enabled)
}
