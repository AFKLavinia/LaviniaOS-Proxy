//! Re-exports classifier and AQM types for backward compatibility and clean decoupling.

pub use super::aqm_controller::TimestampedDatagram;
pub use super::classifier::{DscpClass, TrafficClassifier};

/// Helper function keeping backward compatibility with existing engine modules.
#[inline]
pub fn classify_traffic(dst: &std::net::SocketAddr, payload_len: usize, is_udp: bool) -> DscpClass {
    TrafficClassifier::classify(dst, payload_len, is_udp)
}
