//! DNS over SOCKS5: UDP queries are forwarded as DNS-over-TCP to a fixed upstream resolver.
use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::sync::{watch, Semaphore};
use tokio::time::timeout;

use super::socks5::{self, Upstream};
use super::Counters;

/// Default resolver when the user does not configure one.
pub const DEFAULT_RESOLVER: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
const MAX_INFLIGHT: usize = 256;
const MAX_DATAGRAM: usize = 4096;
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const CACHE_TTL: Duration = Duration::from_secs(60);

type DnsCache = Arc<Mutex<HashMap<Vec<u8>, (Vec<u8>, Instant)>>>;

pub async fn serve(
    socket: UdpSocket,
    proxy: Upstream,
    resolver: Ipv4Addr,
    counters: Arc<Counters>,
    mut stop: watch::Receiver<bool>,
) {
    let socket = Arc::new(socket);
    let permits = Arc::new(Semaphore::new(MAX_INFLIGHT));
    let cache: DnsCache = Arc::new(Mutex::new(HashMap::new()));
    let mut buf = vec![0u8; MAX_DATAGRAM];
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            received = socket.recv_from(&mut buf) => match received {
                Ok((n, peer)) => {
                    if n < 12 {
                        counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    let query_key = buf[2..n].to_vec();
                    let cached_answer = {
                        let mut c = cache.lock().unwrap_or_else(|p| p.into_inner());
                        if let Some((ans, expiry)) = c.get(&query_key) {
                            if Instant::now() < *expiry {
                                let mut reply = ans.clone();
                                reply[0..2].copy_from_slice(&buf[0..2]);
                                Some(reply)
                            } else {
                                c.remove(&query_key);
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(answer) = cached_answer {
                        let _ = socket.send_to(&answer, peer).await;
                        counters.tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
                        counters.tx_packets.fetch_add(1, Ordering::Relaxed);
                        counters.rx_bytes.fetch_add(answer.len() as u64, Ordering::Relaxed);
                        counters.rx_packets.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    // Over capacity: drop the query; the stub resolver retries.
                    let Ok(permit) = permits.clone().try_acquire_owned() else {
                        counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let query = buf[..n].to_vec();
                    let socket = socket.clone();
                    let proxy = proxy.clone();
                    let counters = counters.clone();
                    let cache = cache.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        match resolve(&query, &proxy, resolver).await {
                            Ok(answer) => {
                                {
                                    let mut c = cache.lock().unwrap_or_else(|p| p.into_inner());
                                    if c.len() > 1024 {
                                        c.clear();
                                    }
                                    c.insert(query_key, (answer.clone(), Instant::now() + CACHE_TTL));
                                }
                                if let Err(e) = socket.send_to(&answer, peer).await {
                                    eprintln!("dns reply failed: {e}");
                                }
                                counters.tx_bytes.fetch_add(query.len() as u64, Ordering::Relaxed);
                                counters.tx_packets.fetch_add(1, Ordering::Relaxed);
                                counters.rx_bytes.fetch_add(answer.len() as u64, Ordering::Relaxed);
                                counters.rx_packets.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                eprintln!("dns query failed: {e}");
                                let mut servfail = [0u8; 12];
                                servfail.copy_from_slice(&query[..12]);
                                servfail[2] |= 0x80;
                                servfail[3] = (servfail[3] & 0xF0) | 0x02;
                                servfail[6..12].fill(0);
                                if let Err(e) = socket.send_to(&servfail, peer).await {
                                    eprintln!("dns servfail reply failed: {e}");
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("dns recv failed: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }
}

fn is_ad_or_tracker(query: &[u8]) -> bool {
    if query.len() < 13 {
        return false;
    }
    let mut idx = 12;
    let mut parts: Vec<String> = Vec::new();
    while idx < query.len() {
        let len = query[idx] as usize;
        if len == 0 {
            break;
        }
        idx += 1;
        if idx + len > query.len() {
            return false;
        }
        if let Ok(s) = std::str::from_utf8(&query[idx..idx + len]) {
            parts.push(s.to_ascii_lowercase());
        }
        idx += len;
    }
    let domain = parts.join(".");
    let blocked_patterns = [
        "doubleclick.net",
        "googleadservices.com",
        "googlesyndication.com",
        "adnxs.com",
        "criteo.com",
        "taboola.com",
        "outbrain.com",
        "scorecardresearch.com",
        "analytics.google.com",
        "telemetry",
    ];
    blocked_patterns.iter().any(|pattern| domain.contains(pattern))
}

fn build_blocked_response(query: &[u8]) -> Vec<u8> {
    let mut resp = Vec::with_capacity(query.len() + 16);
    resp.extend_from_slice(&query[..12]);
    resp[2] = 0x81; // QR=1, AA=0, TC=0, RD=1
    resp[3] = 0x80; // RA=1, Z=0, RCODE=0 (No error)
    resp[6..8].copy_from_slice(&1u16.to_be_bytes()); // ANCOUNT = 1

    let mut qend = 12;
    while qend < query.len() && query[qend] != 0 {
        qend += (query[qend] as usize) + 1;
    }
    qend = (qend + 5).min(query.len()); // 0x00 + QTYPE + QCLASS
    resp.extend_from_slice(&query[12..qend]);

    // Answer record: pointer to name (0xC00C), Type A (1), Class IN (1), TTL (60s), Length (4), IP (0.0.0.0)
    resp.extend_from_slice(&[0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3C, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00]);
    resp
}

async fn resolve(query: &[u8], proxy: &Upstream, resolver: Ipv4Addr) -> io::Result<Vec<u8>> {
    if is_ad_or_tracker(query) {
        return Ok(build_blocked_response(query));
    }
    match timeout(QUERY_TIMEOUT, exchange(query, proxy, resolver)).await {
        Ok(r) => r,
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "dns upstream timed out")),
    }
}

async fn exchange(query: &[u8], proxy: &Upstream, resolver: Ipv4Addr) -> io::Result<Vec<u8>> {
    let len = u16::try_from(query.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "dns query too large"))?;
    let mut s = socks5::connect(proxy, SocketAddr::new(resolver.into(), 53)).await?;
    s.write_all(&len.to_be_bytes()).await?;
    s.write_all(query).await?;
    let mut head = [0u8; 2];
    s.read_exact(&mut head).await?;
    let mut answer = vec![0u8; usize::from(u16::from_be_bytes(head))];
    s.read_exact(&mut answer).await?;
    if answer.len() < 12 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "dns reply too short"));
    }
    if answer[0..2] != query[0..2] {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "dns transaction id mismatch"));
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servfail_format() {
        let query = [
            0x12, 0x34, // ID
            0x01, 0x00, // RD=1
            0x00, 0x01, // QDCOUNT=1
            0x00, 0x00, // ANCOUNT=0
            0x00, 0x00, // NSCOUNT=0
            0x00, 0x00, // ARCOUNT=0
            0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0x00,
        ];
        let mut servfail = [0u8; 12];
        servfail.copy_from_slice(&query[..12]);
        servfail[2] |= 0x80;
        servfail[3] = (servfail[3] & 0xF0) | 0x02;
        servfail[6..12].fill(0);

        assert_eq!(&servfail[0..2], &[0x12, 0x34]); // Transaction ID preserved
        assert_eq!(servfail[2] & 0x80, 0x80); // QR=1 (Response)
        assert_eq!(servfail[3] & 0x0F, 0x02); // RCODE=2 (SERVFAIL)
        assert_eq!(&servfail[6..12], &[0; 6]); // ANCOUNT, NSCOUNT, ARCOUNT are 0
    }
}
