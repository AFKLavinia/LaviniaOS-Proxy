//! Transparent SOCKS5 engine. Public entry: [`Engine`], [`Counters`], [`EngineError`].
pub mod aqm;
mod counters;
pub mod debloat;
mod dns;
mod socks5;
mod tcp;
mod udp;

use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tokio::net::{TcpListener, UdpSocket};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::watch;

pub use counters::Counters;
pub use dns::DEFAULT_RESOLVER;
pub use socks5::Auth;

use socks5::Upstream;

use crate::platform::{self, FirewallGuard, FirewallSpec, PlatformError, Uplink};

const WORKER_THREADS: usize = 2;
/// Time the privileged watchdog needs to notice the released flag and delete the rules.
const WATCHDOG_GRACE: Duration = Duration::from_millis(150);
const PROBE_INTERVAL: Duration = Duration::from_secs(2);
const PROBE_FAILURES_TO_DOWN: u32 = 2;

/// Periodically probes the SOCKS5 upstream and publishes link health through the counters.
async fn monitor(proxy: Upstream, counters: Arc<Counters>, mut stop: watch::Receiver<bool>) {
    let mut failures = 0u32;
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            _ = tokio::time::sleep(PROBE_INTERVAL) => {}
        }
        let probe_result = tokio::select! {
            _ = stop.changed() => break,
            r = socks5::probe(&proxy) => r,
        };
        match probe_result {
            Ok(()) => {
                failures = 0;
                counters.link_up.store(true, Ordering::Relaxed);
            }
            Err(_) => {
                failures = failures.saturating_add(1);
                if failures >= PROBE_FAILURES_TO_DOWN {
                    counters.link_up.store(false, Ordering::Relaxed);
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum EngineError {
    Platform(PlatformError),
    Io(io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(e) => e.fmt(f),
            Self::Io(e) => write!(f, "engine i/o error: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<PlatformError> for EngineError {
    fn from(e: PlatformError) -> Self {
        Self::Platform(e)
    }
}

impl From<io::Error> for EngineError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// How UDP (voice/video) traffic is handled for this session.
#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UdpMode {
    /// Relayed through the upstream via SOCKS5 UDP ASSOCIATE.
    Active,
    /// Process lacks CAP_NET_ADMIN; UDP stays blocked.
    NoPrivilege,
    /// Upstream refused UDP ASSOCIATE; UDP stays blocked.
    Unsupported,
}

pub struct Engine {
    uplink: Uplink,
    udp: UdpMode,
    runtime: Option<Runtime>,
    shutdown: watch::Sender<bool>,
    guard: Option<FirewallGuard>,
}

impl Engine {
    /// Blocking: validates the uplink, starts listeners, then installs redirect rules.
    pub fn start(
        phone: Ipv4Addr,
        port: u16,
        auth: Option<Auth>,
        resolver: Ipv4Addr,
        counters: Arc<Counters>,
    ) -> Result<Self, EngineError> {
        let uplink = platform::check_uplink(phone)?;

        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("lavinia-engine")
            .enable_all()
            .build()?;
        let proxy = Upstream::new(SocketAddr::from((phone, port)), auth);
        let (stop_tx, stop_rx) = watch::channel(false);

        let udp = if !platform::udp_capable() {
            UdpMode::NoPrivilege
        } else if runtime.block_on(socks5::probe_udp(&proxy)).is_err() {
            UdpMode::Unsupported
        } else {
            UdpMode::Active
        };
        let udp_std = if udp == UdpMode::Active { Some(platform::udp_intercept_socket()?) } else { None };
        let (tcp_listener, dns_socket, udp_socket) = runtime.block_on(async {
            let t = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
            let u = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await?;
            let x = match udp_std {
                Some(s) => Some(UdpSocket::from_std(s)?),
                None => None,
            };
            Ok::<_, io::Error>((t, u, x))
        })?;
        let spec = FirewallSpec {
            phone,
            tcp_port: tcp_listener.local_addr()?.port(),
            dns_port: dns_socket.local_addr()?.port(),
            udp_port: match &udp_socket {
                Some(s) => Some(s.local_addr()?.port()),
                None => None,
            },
        };

        counters.link_up.store(true, Ordering::Relaxed);
        runtime.spawn(monitor(proxy.clone(), counters.clone(), stop_rx.clone()));
        runtime.spawn(tcp::serve(tcp_listener, proxy.clone(), counters.clone(), stop_rx.clone()));
        if let Some(sock) = udp_socket {
            runtime.spawn(udp::serve(sock, proxy.clone(), counters.clone(), stop_rx.clone()));
        }
        runtime.spawn(dns::serve(dns_socket, proxy, resolver, counters, stop_rx));

        let guard = platform::apply_firewall(&spec)?;
        Ok(Self { uplink, udp, runtime: Some(runtime), shutdown: stop_tx, guard: Some(guard) })
    }

    pub fn uplink(&self) -> &Uplink {
        &self.uplink
    }

    pub fn udp_mode(&self) -> UdpMode {
        self.udp
    }

    pub fn stop(mut self) {
        self.teardown();
    }

    fn teardown(&mut self) {
        if let Some(mut guard) = self.guard.take() {
            guard.release();
            thread::sleep(WATCHDOG_GRACE);
        }
        self.shutdown.send_replace(true);
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.teardown();
    }
}
