//! Traffic classification, DSCP/TOS tagging, and BBR-style Token Bucket Pacer.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// DiffServ / DSCP Traffic Classes
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DscpClass {
    /// DNS, Network Control (CS6 = 48 -> 0xC0)
    ControlDns = 48,
    /// Discord, Voice/VoIP (EF = 46 -> 0xB8)
    VoiceVoip = 46,
    /// Gaming UDP (AF41 = 34 -> 0x88)
    GamingUdp = 34,
    /// Interactive Web (AF21 = 18 -> 0x48)
    InteractiveWeb = 18,
    /// Bulk Downloads, Steam, Torrent (Best Effort = 0 -> 0x00)
    Bulk = 0,
}

impl DscpClass {
    #[inline]
    pub fn to_tos(self) -> u8 {
        (self as u8) << 2
    }
}

/// Heuristically classify traffic based on destination port and payload size.
pub fn classify_traffic(dst: &SocketAddr, payload_len: usize, is_udp: bool) -> DscpClass {
    let port = dst.port();
    if port == 53 || port == 853 {
        return DscpClass::ControlDns;
    }
    if is_udp {
        // Discord voice UDP range or small real-time payloads
        if (50000..=65535).contains(&port) || payload_len <= 250 {
            return DscpClass::VoiceVoip;
        }
        // Gaming ports (Source engine, Riot, Battle.net, etc.) or medium UDP
        if (27015..=27050).contains(&port) || (5000..=5500).contains(&port) || payload_len <= 800 {
            return DscpClass::GamingUdp;
        }
    } else {
        // Interactive web handshakes or small TCP packets
        if payload_len <= 512 {
            return DscpClass::InteractiveWeb;
        }
    }
    DscpClass::Bulk
}

/// Token Bucket Pacer for smoothing cellular traffic micro-bursts without high CPU context switches.
pub struct TokenBucketPacer {
    tokens_bytes: f64,
    max_burst_bytes: f64,
    last_update: Instant,
}

impl TokenBucketPacer {
    pub fn new() -> Self {
        Self {
            tokens_bytes: 1500.0 * 4.0, // 4-packet initial burst credit
            max_burst_bytes: 64.0 * 1024.0,
            last_update: Instant::now(),
        }
    }

    /// Smooths packet pacing by sleeping only when tokens are depleted beyond 1ms deficit.
    pub async fn pace(&mut self, pkt_len: usize, pacing_rate_bps: u64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        let rate_bytes_per_sec = (pacing_rate_bps / 8) as f64;
        self.tokens_bytes = (self.tokens_bytes + elapsed * rate_bytes_per_sec).min(self.max_burst_bytes);
        self.tokens_bytes -= pkt_len as f64;

        if self.tokens_bytes < 0.0 {
            let deficit = -self.tokens_bytes;
            let wait_secs = deficit / rate_bytes_per_sec;
            let wait_dur = Duration::from_secs_f64(wait_secs);

            // Avoid expensive thread sleep for micro-delays (<1ms); sleep only for meaningful backoff
            if wait_dur >= Duration::from_millis(1) {
                tokio::time::sleep(wait_dur).await;
                self.last_update = Instant::now();
                self.tokens_bytes = 0.0;
            }
        }
    }
}
