//! Ingress DTOs (raw transport) and outbound IPC payloads.
use std::fmt;
use std::net::{IpAddr, Ipv4Addr};

use crate::engine::{Auth, EngineError, UdpMode, DEFAULT_RESOLVER};
use crate::platform::Uplink;

use serde::{Deserialize, Serialize};

pub const DEFAULT_HOST: &str = "192.168.43.1";
pub const DEFAULT_PORT: u16 = 10808;

/// Raw, untrusted UI input. Unknown fields are rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDto {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub dns: Option<String>,
}

/// Validated start request: endpoint plus optional credentials and resolver.
pub struct StartOptions {
    pub target: Target,
    pub auth: Option<Auth>,
    pub resolver: Ipv4Addr,
}

impl TryFrom<TargetDto> for StartOptions {
    type Error = TunnelError;

    fn try_from(dto: TargetDto) -> Result<Self, Self::Error> {
        let auth = match (dto.username.as_deref().unwrap_or(""), dto.password.as_deref().unwrap_or("")) {
            ("", "") => None,
            (u, p) => Some(Auth::new(u, p).ok_or(TunnelError::Validation(
                "username and password must both be set (1-255 bytes each)",
            ))?),
        };
        let resolver = match dto.dns.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => DEFAULT_RESOLVER,
            Some(s) => {
                let ip: Ipv4Addr =
                    s.parse().map_err(|_| TunnelError::Validation("DNS server must be an IPv4 address"))?;
                if ip.is_unspecified() || ip.is_multicast() || ip.is_loopback() || ip.is_broadcast() {
                    return Err(TunnelError::Validation("DNS server address is not usable"));
                }
                ip
            }
        };
        let target = Target::try_from(dto)?;
        Ok(Self { target, auth, resolver })
    }
}

/// Validated domain model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub host: IpAddr,
    pub port: u16,
}

impl Target {
    pub fn default_phone() -> Self {
        Self {
            host: DEFAULT_HOST.parse().expect("valid default host"),
            port: DEFAULT_PORT,
        }
    }
}

impl TryFrom<TargetDto> for Target {
    type Error = TunnelError;

    fn try_from(dto: TargetDto) -> Result<Self, Self::Error> {
        let host: IpAddr = dto
            .host
            .trim()
            .parse()
            .map_err(|_| TunnelError::Validation("host must be a valid IP address"))?;
        if host.is_unspecified() || host.is_multicast() || host.is_loopback() {
            return Err(TunnelError::Validation("host address is not routable"));
        }
        match host {
            IpAddr::V4(v4) => {
                if v4.is_broadcast() {
                    return Err(TunnelError::Validation("host address is not routable"));
                }
            }
            IpAddr::V6(_) => {
                return Err(TunnelError::Validation("only IPv4 proxy addresses are supported"));
            }
        }
        if dto.port == 0 {
            return Err(TunnelError::Validation("port must be between 1 and 65535"));
        }
        Ok(Self { host, port: dto.port })
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Stopped,
    Starting,
    Running,
    Degraded,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub target: Target,
    /// Uplink that carries the phone connection (present while the tunnel is up).
    pub link: Option<Uplink>,
    pub udp: Option<UdpMode>,
}

impl PartialEq for Status {
    fn eq(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.target == other.target
            && self.udp == other.udp
            && match (&self.link, &other.link) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.device == b.device
                        && a.kind == b.kind
                        && a.name == b.name
                        && a.hardware == b.hardware
                }
                _ => false,
            }
    }
}

impl Eq for Status {}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub phase: Phase,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub dropped: u64,
}

#[derive(Debug)]
pub enum TunnelError {
    Validation(&'static str),
    Conflict(&'static str),
    Engine(String),
}

impl From<EngineError> for TunnelError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e.to_string())
    }
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(m) | Self::Conflict(m) => f.write_str(m),
            Self::Engine(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for TunnelError {}

impl Serialize for TunnelError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
