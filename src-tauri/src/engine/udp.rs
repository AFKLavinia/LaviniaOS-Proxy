//! Transparent UDP relay (voice/video/games): diverted datagram -> SOCKS5 UDP ASSOCIATE -> reply spoofed as original dst.
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr, SocketAddrV4};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Semaphore};

use super::debloat::{self, TimestampedDatagram};
use super::socks5::{self, Upstream};
use super::Counters;
use crate::platform;

const MAX_SESSIONS: usize = 1024;
const MAX_DATAGRAM: usize = 65_535;
const SESSION_QUEUE: usize = 64;
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

type Key = (SocketAddrV4, SocketAddrV4);
type Sessions = Arc<Mutex<HashMap<Key, mpsc::Sender<TimestampedDatagram>>>>;

struct SessionGuard {
    key: Key,
    sessions: Sessions,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.key);
    }
}

pub async fn serve(socket: UdpSocket, proxy: Upstream, counters: Arc<Counters>, mut stop: watch::Receiver<bool>) {
    let sessions: Sessions = Arc::default();
    let permits = Arc::new(Semaphore::new(MAX_SESSIONS));
    let mut buf = vec![0u8; MAX_DATAGRAM];
    loop {
        let received = tokio::select! {
            _ = stop.changed() => break,
            r = platform::recv_udp_orig(&socket, &mut buf) => r,
        };
        let (n, client, dst) = match received {
            Ok(v) => v,
            Err(e) => {
                eprintln!("udp recv failed: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        if dst.ip().is_loopback()
            || dst.ip().is_unspecified()
            || dst.ip().is_multicast()
            || dst.ip().is_broadcast()
            || IpAddr::V4(*dst.ip()) == proxy.addr.ip()
        {
            continue;
        }
        let key = (client, dst);
        let traffic_class = debloat::classify_traffic(&SocketAddr::V4(dst), n, true);
        let is_realtime = matches!(
            traffic_class,
            debloat::DscpClass::ControlDns
                | debloat::DscpClass::VoiceVoip
                | debloat::DscpClass::GamingUdp
        );
        let datagram = TimestampedDatagram::new(buf[..n].to_vec(), is_realtime);
        let existing = sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
            .cloned();
        if let Some(tx) = existing {
            if tx.try_send(datagram).is_err() {
                counters.dropped.fetch_add(1, Ordering::Relaxed);
            }
            continue;
        }
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            counters.dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let (tx, rx) = mpsc::channel(SESSION_QUEUE);
        let _ = tx.try_send(datagram);
        sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key, tx);
        let (sessions, counters, proxy) = (sessions.clone(), counters.clone(), proxy.clone());
        let task_stop = stop.clone();
        let guard = SessionGuard {
            key,
            sessions: sessions.clone(),
        };
        tokio::spawn(async move {
            let _permit = permit;
            let _guard = guard;
            if let Err(e) = session(key, &proxy, rx, &counters, task_stop).await {
                eprintln!("udp session ended: {e}");
            }
        });
    }
}

async fn session(
    (client, dst): Key,
    proxy: &Upstream,
    mut rx: mpsc::Receiver<TimestampedDatagram>,
    c: &Counters,
    mut stop: watch::Receiver<bool>,
) -> io::Result<()> {
    let (mut control, relay) = socks5::udp_associate(proxy).await?;
    let bind_addr: SocketAddr = if relay.is_ipv6() {
        ([0u16; 8], 0).into()
    } else {
        ([0u8; 4], 0).into()
    };
    let upstream = UdpSocket::bind(bind_addr).await?;
    upstream.connect(relay).await?;
    let reply = UdpSocket::from_std(platform::udp_spoof_socket(dst)?)?;

    let sock = socket2::SockRef::from(&upstream);
    let dscp = debloat::classify_traffic(&SocketAddr::V4(dst), 0, true);
    let _ = sock.set_tos(dscp.to_tos() as u32);

    let mut down = vec![0u8; MAX_DATAGRAM];
    let mut ctl = [0u8; 1];
    let mut spoof_cache: std::collections::HashMap<SocketAddrV4, UdpSocket> = std::collections::HashMap::new();
    const MAX_SPOOF_CACHE: usize = 32;

    // Idle deadline is pushed back by traffic in either direction.
    let idle = tokio::time::sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    loop {
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            _ = &mut idle => return Ok(()),
            out = rx.recv() => {
                let Some(dg) = out else { return Ok(()) };
                // Dynamic Adaptive Queue Management (RFC 8289 CoDel):
                // Realtime gaming (War Thunder, HOI4) and voice packets that exceed latency
                // tolerance are safely dropped to eliminate bufferbloat, while non-realtime
                // packets are fully preserved.
                if dg.is_stale(tokio::time::Instant::now().into()) {
                    c.dropped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                upstream.send(&socks5::encode_udp(dst, &dg.payload)).await?;
                c.tx_bytes.fetch_add(dg.payload.len() as u64, Ordering::Relaxed);
                c.tx_packets.fetch_add(1, Ordering::Relaxed);
                idle.as_mut().reset(tokio::time::Instant::now() + IDLE_TIMEOUT);
            }
            inbound = upstream.recv(&mut down) => {
                let n = inbound?;
                idle.as_mut().reset(tokio::time::Instant::now() + IDLE_TIMEOUT);
                if let Some((src, payload)) = socks5::decode_udp(&down[..n]) {
                    if src == dst {
                        reply.send_to(payload, SocketAddr::V4(client)).await?;
                    } else if let Some(cached_sock) = spoof_cache.get(&src) {
                        let _ = cached_sock.send_to(payload, SocketAddr::V4(client)).await;
                    } else if spoof_cache.len() < MAX_SPOOF_CACHE {
                        if let Ok(std_sock) = platform::udp_spoof_socket(src) {
                            if let Ok(tokio_sock) = UdpSocket::from_std(std_sock) {
                                let _ = tokio_sock.send_to(payload, SocketAddr::V4(client)).await;
                                spoof_cache.insert(src, tokio_sock);
                            }
                        }
                    }
                    c.rx_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                    c.rx_packets.fetch_add(1, Ordering::Relaxed);
                }
            }
            // Control TCP closing terminates the association per RFC 1928.
            closed = control.read(&mut ctl) => {
                match closed {
                    Ok(0) | Err(_) => return Ok(()),
                    Ok(_) => {}
                }
            }
        }
    }
}
