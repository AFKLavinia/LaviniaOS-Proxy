//! DNS over SOCKS5: UDP queries are forwarded as DNS-over-TCP to a fixed upstream resolver.
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

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

pub async fn serve(
    socket: UdpSocket,
    proxy: Upstream,
    resolver: Ipv4Addr,
    counters: Arc<Counters>,
    mut stop: watch::Receiver<bool>,
) {
    let socket = Arc::new(socket);
    let permits = Arc::new(Semaphore::new(MAX_INFLIGHT));
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
                    // Over capacity: drop the query; the stub resolver retries.
                    let Ok(permit) = permits.clone().try_acquire_owned() else {
                        counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let query = buf[..n].to_vec();
                    let socket = socket.clone();
                    let proxy = proxy.clone();
                    let counters = counters.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        match resolve(&query, &proxy, resolver).await {
                            Ok(answer) => {
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

async fn resolve(query: &[u8], proxy: &Upstream, resolver: Ipv4Addr) -> io::Result<Vec<u8>> {
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
