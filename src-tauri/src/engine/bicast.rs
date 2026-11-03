//! Multipath bicasting / dual-uplink support for concurrent Wi-Fi and USB tethering.
//! Sends latency-critical packets (gaming/VoIP) over both interfaces simultaneously to achieve zero packet loss.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;

pub struct BicastUplink {
    primary: Arc<UdpSocket>,
    secondary: Option<Arc<UdpSocket>>,
    enabled: AtomicBool,
}

impl BicastUplink {
    pub fn new(primary: Arc<UdpSocket>, secondary: Option<Arc<UdpSocket>>) -> Self {
        let has_secondary = secondary.is_some();
        Self {
            primary,
            secondary,
            enabled: AtomicBool::new(has_secondary),
        }
    }

    /// Transmits a datagram. If bicasting is enabled and a secondary uplink (e.g. USB tethering) is present,
    /// duplicates the datagram simultaneously on both interfaces.
    pub async fn send_to(&self, buf: &[u8], target: SocketAddr) -> std::io::Result<usize> {
        if self.enabled.load(Ordering::Relaxed) {
            if let Some(ref sec) = self.secondary {
                let (prim_res, sec_res) = tokio::join!(
                    self.primary.send_to(buf, target),
                    sec.send_to(buf, target)
                );
                return match (prim_res, sec_res) {
                    (Ok(n), _) => Ok(n),
                    (Err(_), Ok(n)) => Ok(n),
                    (Err(e), Err(_)) => Err(e),
                };
            }
        }
        self.primary.send_to(buf, target).await
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val && self.secondary.is_some(), Ordering::Relaxed);
    }

    pub fn is_active(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

