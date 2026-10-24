//! Transparent UDP relay (voice/video/games): diverted datagram -> SOCKS5 UDP ASSOCIATE -> reply spoofed as original dst.
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Semaphore};

use super::socks5::{self, Upstream};
use super::Counters;
use crate::platform;

const MAX_SESSIONS: usize = 1024;
const MAX_DATAGRAM: usize = 65_535;
const SESSION_QUEUE: usize = 256;
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

type Key = (SocketAddrV4, SocketAddrV4);
type Sessions = Arc<Mutex<HashMap<Key, mpsc::Sender<Vec<u8>>>>>;

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
                continue;
            }
        };
        if dst.ip().is_loopback() {
            continue;
        }
        let key = (client, dst);
        let payload = buf[..n].to_vec();
        let existing = sessions.lock().ok().and_then(|m| m.get(&key).cloned());
        if let Some(tx) = existing {
            if tx.try_send(payload).is_err() {
                counters.dropped.fetch_add(1, Ordering::Relaxed);
            }
            continue;
        }
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            counters.dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let (tx, rx) = mpsc::channel(SESSION_QUEUE);
        let _ = tx.try_send(payload);
        if let Ok(mut m) = sessions.lock() {
            m.insert(key, tx);
        }
        let (sessions, counters, proxy) = (sessions.clone(), counters.clone(), proxy.clone());
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = session(key, &proxy, rx, &counters).await {
                eprintln!("udp session ended: {e}");
            }
            if let Ok(mut m) = sessions.lock() {
                m.remove(&key);
            }
        });
    }
}

async fn session(
    (client, dst): Key,
    proxy: &Upstream,
    mut rx: mpsc::Receiver<Vec<u8>>,
    c: &Counters,
) -> io::Result<()> {
    let (mut control, relay) = socks5::udp_associate(proxy).await?;
    let upstream = UdpSocket::bind("0.0.0.0:0").await?;
    upstream.connect(relay).await?;
    let reply = UdpSocket::from_std(platform::udp_spoof_socket(dst)?)?;

    let mut down = vec![0u8; MAX_DATAGRAM];
    let mut ctl = [0u8; 1];
    // Idle deadline is pushed back by traffic in either direction.
    let idle = tokio::time::sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    loop {
        tokio::select! {
            _ = &mut idle => return Ok(()),
            out = rx.recv() => {
                let Some(payload) = out else { return Ok(()) };
                upstream.send(&socks5::encode_udp(dst, &payload)).await?;
                c.tx_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                c.tx_packets.fetch_add(1, Ordering::Relaxed);
                idle.as_mut().reset(tokio::time::Instant::now() + IDLE_TIMEOUT);
            }
            inbound = upstream.recv(&mut down) => {
                let n = inbound?;
                idle.as_mut().reset(tokio::time::Instant::now() + IDLE_TIMEOUT);
                if let Some((_, payload)) = socks5::decode_udp(&down[..n]) {
                    reply.send_to(payload, SocketAddr::V4(client)).await?;
                    c.rx_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                    c.rx_packets.fetch_add(1, Ordering::Relaxed);
                }
            }
            // Control TCP closing terminates the association per RFC 1928.
            closed = control.read(&mut ctl) => {
                closed?;
                return Ok(());
            }
        }
    }
}
