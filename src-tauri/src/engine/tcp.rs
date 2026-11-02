//! Transparent TCP relay: redirected connection -> original dst -> SOCKS5 upstream.
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};

use super::debloat;
use super::socks5::{self, Upstream};
use super::Counters;
use crate::platform;

const MAX_CONNECTIONS: usize = 2048;
const RELAY_BUF: usize = 64 * 1024;
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

fn apply_socket_opts(stream: &TcpStream, dst: &SocketAddr) {
    let socket = socket2::SockRef::from(stream);
    let dscp = debloat::classify_traffic(dst, 0, false);
    let tos = dscp.to_tos() as u32;
    let _ = socket.set_tos(tos);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(30))
        .with_interval(Duration::from_secs(10));
    let _ = socket.set_tcp_keepalive(&keepalive);
}

pub async fn serve(
    listener: TcpListener,
    proxy: Upstream,
    counters: Arc<Counters>,
    mut stop: watch::Receiver<bool>,
) {
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((client, _)) => {
                    // At capacity: refuse by closing, and account for it.
                    let Ok(permit) = permits.clone().try_acquire_owned() else {
                        counters.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let counters = counters.clone();
                    let proxy = proxy.clone();
                    let mut task_stop = stop.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        let res = tokio::select! {
                            _ = task_stop.changed() => Ok(()),
                            r = relay(client, &proxy, &counters) => r,
                        };
                        if let Err(e) = res {
                            report(&e);
                        }
                    });
                }
                Err(e) => {
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::Interrupted
                    ) {
                        continue;
                    }
                    eprintln!("tcp accept failed: {e}");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            }
        }
    }
}

async fn relay(client: TcpStream, proxy: &Upstream, c: &Counters) -> io::Result<()> {
    let dst = platform::original_dst(&client)?;
    if dst.ip().is_loopback()
        || dst.ip().is_unspecified()
        || dst.ip().is_multicast()
        || dst.ip().is_broadcast()
        || SocketAddr::V4(dst).ip() == proxy.addr.ip()
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "destination address refused"));
    }
    let upstream = socks5::connect(proxy, SocketAddr::V4(dst)).await?;
    client.set_nodelay(true)?;
    upstream.set_nodelay(true)?;
    apply_socket_opts(&client, &SocketAddr::V4(dst));
    apply_socket_opts(&upstream, &SocketAddr::V4(dst));

    let (mut cr, mut cw) = client.into_split();
    let (mut ur, mut uw) = upstream.into_split();

    let client_to_upstream = async {
        let mut buf = vec![0u8; RELAY_BUF];
        loop {
            let n = cr.read(&mut buf).await?;
            if n == 0 {
                uw.shutdown().await?;
                break;
            }
            uw.write_all(&buf[..n]).await?;
            c.tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
            c.tx_packets.fetch_add(1, Ordering::Relaxed);
        }
        Ok::<(), io::Error>(())
    };

    let upstream_to_client = async {
        let mut buf = vec![0u8; RELAY_BUF];
        loop {
            let n = ur.read(&mut buf).await?;
            if n == 0 {
                cw.shutdown().await?;
                break;
            }
            cw.write_all(&buf[..n]).await?;
            c.rx_bytes.fetch_add(n as u64, Ordering::Relaxed);
            c.rx_packets.fetch_add(1, Ordering::Relaxed);
        }
        Ok::<(), io::Error>(())
    };

    tokio::select! {
        res = async { tokio::try_join!(client_to_upstream, upstream_to_client) } => {
            res?;
        }
        _ = tokio::time::sleep(IDLE_TIMEOUT) => {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "tcp relay idle timeout"));
        }
    }
    Ok(())
}

/// Peer resets/closes are normal relay termination; anything else is logged.
fn report(e: &io::Error) {
    use io::ErrorKind::*;
    if !matches!(e.kind(), ConnectionReset | BrokenPipe | ConnectionAborted | UnexpectedEof | NotConnected) {
        eprintln!("relay ended: {e}");
    }
}
