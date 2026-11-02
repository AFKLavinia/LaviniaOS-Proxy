use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Live traffic counters updated by the relay tasks, read by telemetry.
/// `*_packets` counts relayed read segments, not raw IP packets.
#[derive(Default)]
pub struct Counters {
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub dropped: AtomicU64,
    /// Live round-trip time in milliseconds to upstream proxy.
    pub rtt_ms: AtomicU64,
    /// Jitter in milliseconds.
    pub jitter_ms: AtomicU64,
    /// True while the SOCKS5 upstream answered the most recent health probe.
    pub link_up: AtomicBool,
}

impl Counters {
    pub fn reset(&self) {
        self.rx_bytes.store(0, Ordering::Relaxed);
        self.tx_bytes.store(0, Ordering::Relaxed);
        self.rx_packets.store(0, Ordering::Relaxed);
        self.tx_packets.store(0, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        self.rtt_ms.store(0, Ordering::Relaxed);
        self.jitter_ms.store(0, Ordering::Relaxed);
        self.link_up.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_clears_all_counters() {
        let c = Counters::default();
        c.rx_bytes.store(100, Ordering::Relaxed);
        c.tx_bytes.store(200, Ordering::Relaxed);
        c.rx_packets.store(10, Ordering::Relaxed);
        c.tx_packets.store(20, Ordering::Relaxed);
        c.dropped.store(5, Ordering::Relaxed);
        c.link_up.store(true, Ordering::Relaxed);

        c.reset();

        assert_eq!(c.rx_bytes.load(Ordering::Relaxed), 0);
        assert_eq!(c.tx_bytes.load(Ordering::Relaxed), 0);
        assert_eq!(c.rx_packets.load(Ordering::Relaxed), 0);
        assert_eq!(c.tx_packets.load(Ordering::Relaxed), 0);
        assert_eq!(c.dropped.load(Ordering::Relaxed), 0);
        assert!(!c.link_up.load(Ordering::Relaxed));
    }
}
