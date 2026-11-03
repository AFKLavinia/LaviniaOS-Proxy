//! Dedicated Network Classifier for Games, Voice, Web, and Streaming.
//! Implements strict Single Responsibility Principle (SRP).

use std::net::SocketAddr;

/// DiffServ / DSCP Traffic Classes (RFC 4594 / RFC 8622)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DscpClass {
    /// DNS, Network Control (CS6 = 48 -> 0xC0)
    ControlDns = 48,
    /// Discord, Voice/VoIP (EF = 46 -> 0xB8)
    VoiceVoip = 46,
    /// Gaming UDP: War Thunder, Hearts of Iron IV, Source/Steam (AF41 = 34 -> 0x88)
    GamingUdp = 34,
    /// Interactive Web & Gaming Handshakes (AF21 = 18 -> 0x48)
    InteractiveWeb = 18,
    /// Bulk Downloads, Steam Content, Torrents (Best Effort = 0 -> 0x00)
    Bulk = 0,
}

impl DscpClass {
    #[inline]
    pub fn to_tos(self) -> u8 {
        (self as u8) << 2
    }
}

/// Specialized detector for War Thunder, Hearts of Iron IV, Steam, Discord, and Competitive Games.
pub struct TrafficClassifier;

impl TrafficClassifier {
    /// Classifies an IP destination and datagram length into appropriate DSCP class.
    pub fn classify(dst: &SocketAddr, payload_len: usize, is_udp: bool) -> DscpClass {
        let port = dst.port();

        // 1. DNS / DoT Control
        if port == 53 || port == 853 {
            return DscpClass::ControlDns;
        }

        if is_udp {
            // 2. War Thunder Dedicated Ports (20000-20500 UDP & 3478-3480 STUN/Voice)
            if (20000..=20500).contains(&port) || (3478..=3480).contains(&port) {
                return DscpClass::GamingUdp;
            }

            // 3. Paradox Interactive / Hearts of Iron IV Multiplayer (1630-1640 UDP, Nakama & Steam SDR 27015-27060)
            if (1630..=1640).contains(&port) || (27015..=27060).contains(&port) {
                return DscpClass::GamingUdp;
            }

            // 4. Steam P2P / Networking Sockets & Voice (27000-27100, 3478, 4379-4380 UDP)
            if (27000..=27100).contains(&port) || port == 3478 || (4379..=4380).contains(&port) {
                return DscpClass::GamingUdp;
            }

            // 5. Discord WebRTC Voice / Video (19295-19350, 50000-65535 UDP)
            if (19295..=19350).contains(&port) || (50000..=65535).contains(&port) {
                return DscpClass::VoiceVoip;
            }

            // 6. Generic Competitive Gaming (Unreal/Source/Riot/Battle.net ports)
            if (5000..=5500).contains(&port) || (7000..=8000).contains(&port) {
                return DscpClass::GamingUdp;
            }

            // 7. QUIC / HTTP/3 (Ports 443 / 8443)
            if port == 443 || port == 8443 {
                return if payload_len <= 512 {
                    DscpClass::InteractiveWeb
                } else {
                    DscpClass::Bulk
                };
            }

            // 8. Heuristic payload inspection for gaming and voice datagrams (only when payload > 0)
            if payload_len > 0 && payload_len <= 350 {
                return DscpClass::VoiceVoip;
            } else if payload_len > 350 && payload_len <= 1100 {
                return DscpClass::GamingUdp;
            }

            DscpClass::Bulk
        } else {
            // TCP Classification
            // Hearts of Iron IV Nakama / Steam MP TCP ports (Steam 27015-27030, Paradox ports)
            if (27014..=27050).contains(&port) {
                return DscpClass::GamingUdp;
            }

            if payload_len <= 512 {
                return DscpClass::InteractiveWeb;
            }

            DscpClass::Bulk
        }
    }

    /// Determines if a packet is latency-critical (realtime voice/gaming/dns)
    #[inline]
    pub fn is_realtime(dscp: DscpClass) -> bool {
        matches!(
            dscp,
            DscpClass::ControlDns | DscpClass::VoiceVoip | DscpClass::GamingUdp
        )
    }
}

