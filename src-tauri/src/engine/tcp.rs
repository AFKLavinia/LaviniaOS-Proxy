//! Transparent TCP relay: redirected connection -> original dst -> SOCKS5 upstream.
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};

use super::socks5::{self, Upstream};
use super::Counters;
use crate::platform;

const MAX_CONNECTIONS: usize = 2048;
const RELAY_BUF: usize = 16 * 1024;
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

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

    let (cr, cw) = client.into_split();
    let (ur, uw) = upstream.into_split();
    tokio::try_join!(
        pump(cr, uw, &c.tx_bytes, &c.tx_packets),
        pump(ur, cw, &c.rx_bytes, &c.rx_packets),
    )?;
    Ok(())
}

async fn pump<R, W>(mut r: R, mut w: W, bytes: &AtomicU64, segments: &AtomicU64) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; RELAY_BUF];
    loop {
        let n = match tokio::time::timeout(IDLE_TIMEOUT, r.read(&mut buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "tcp relay idle timeout")),
        };
        if n == 0 {
            return w.shutdown().await;
        }
        w.write_all(&buf[..n]).await?;
        bytes.fetch_add(n as u64, Ordering::Relaxed);
        segments.fetch_add(1, Ordering::Relaxed);
    }
}

/// Peer resets/closes are normal relay termination; anything else is logged.
fn report(e: &io::Error) {
    use io::ErrorKind::*;
    if !matches!(e.kind(), ConnectionReset | BrokenPipe | ConnectionAborted | UnexpectedEof | NotConnected) {
        eprintln!("relay ended: {e}");
    }
}
