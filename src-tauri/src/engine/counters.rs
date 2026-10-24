use std::sync::atomic::{AtomicBool, AtomicU64};

/// Live traffic counters updated by the relay tasks, read by telemetry.
/// `*_packets` counts relayed read segments, not raw IP packets.
#[derive(Default)]
pub struct Counters {
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub dropped: AtomicU64,
    /// True while the SOCKS5 upstream answered the most recent health probe.
    pub link_up: AtomicBool,
}
