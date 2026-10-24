//! DNS over SOCKS5: UDP queries are forwarded as DNS-over-TCP to a fixed upstream resolver.
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::sync::{watch, Semaphore};
use tokio::time::timeout;

use super::socks5::{self, Upstream};

/// Default resolver when the user does not configure one.
pub const DEFAULT_RESOLVER: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
const MAX_INFLIGHT: usize = 256;
const MAX_DATAGRAM: usize = 4096;
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn serve(socket: UdpSocket, proxy: Upstream, resolver: Ipv4Addr, mut stop: watch::Receiver<bool>) {
    let socket = Arc::new(socket);
    let permits = Arc::new(Semaphore::new(MAX_INFLIGHT));
    let mut buf = vec![0u8; MAX_DATAGRAM];
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            received = socket.recv_from(&mut buf) => match received {
                Ok((n, peer)) => {
                    // Over capacity: drop the query; the stub resolver retries.
                    let Ok(permit) = permits.clone().try_acquire_owned() else { continue };
                    let query = buf[..n].to_vec();
                    let socket = socket.clone();
                    let proxy = proxy.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        match resolve(&query, &proxy, resolver).await {
                            Ok(answer) => {
                                if let Err(e) = socket.send_to(&answer, peer).await {
                                    eprintln!("dns reply failed: {e}");
                                }
                            }
                            Err(e) => eprintln!("dns query failed: {e}"),
                        }
                    });
                }
                Err(e) => eprintln!("dns recv failed: {e}"),
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
    Ok(answer)
}
