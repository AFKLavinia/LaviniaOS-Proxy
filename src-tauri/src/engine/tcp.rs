//! Transparent TCP relay: redirected connection -> original dst -> SOCKS5 upstream.
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};

use super::debloat;
use super::socks5::{self, Upstream};
use super::Counters;
use crate::platform;

const MAX_CONNECTIONS: usize = 2048;
const RELAY_BUF: usize = 16 * 1024;
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
    let _ = socket.set_nodelay(true);

    #[cfg(target_os = "linux")]
    {
        let fd = stream.as_raw_fd();
        // Enable TCP_QUICKACK to prevent 40ms delayed-ACK jitter in games like HoI4/War Thunder
        unsafe {
            let one: libc::c_int = 1;
            let _ = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_QUICKACK,
                &one as *const _ as *const libc::c_void,
                std::mem::size_of_val(&one) as libc::socklen_t,
            );
        }
        // Anti-bufferbloat: limit un-sent queue in kernel socket buffer to 16KB.
        // Fail-open: if unsupported by the kernel, ignore silently.
        const TCP_NOTSENT_LOWAT: libc::c_int = 25;
        let lowat: libc::c_uint = 16384;
        unsafe {
            let _ = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                TCP_NOTSENT_LOWAT,
                &lowat as *const _ as *const libc::c_void,
                std::mem::size_of_val(&lowat) as libc::socklen_t,
            );
        }

        // Try enabling BBR congestion control on the upstream/client socket if available.
        // Fail-open: if TCP_CONGESTION bbr is not loaded or unsupported, kernel remains on default (e.g. cubic).
        let bbr_name = b"bbr\0";
        unsafe {
            let _ = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_CONGESTION,
                bbr_name.as_ptr() as *const libc::c_void,
                (bbr_name.len() - 1) as libc::socklen_t,
            );
        }
    }
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

async fn relay(mut client: TcpStream, proxy: &Upstream, c: &Counters) -> io::Result<()> {
    let dst = platform::original_dst(&client)?;
    if dst.ip().is_loopback()
        || dst.ip().is_unspecified()
        || dst.ip().is_multicast()
        || dst.ip().is_broadcast()
        || SocketAddr::V4(dst).ip() == proxy.addr.ip()
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "destination address refused"));
    }
    let mut upstream = socks5::connect(proxy, SocketAddr::V4(dst)).await?;
    client.set_nodelay(true)?;
    upstream.set_nodelay(true)?;
    apply_socket_opts(&client, &SocketAddr::V4(dst));
    apply_socket_opts(&upstream, &SocketAddr::V4(dst));

    let (mut cr, mut cw) = client.split();
    let (mut ur, mut uw) = upstream.split();

    let last_activity = Arc::new(std::sync::atomic::AtomicU64::new(
        crate::engine::aqm_controller::monotonic_micros(std::time::Instant::now()),
    ));

    let act1 = Arc::clone(&last_activity);
    let c1 = c;
    let client_to_upstream = async move {
        let mut buf = vec![0u8; RELAY_BUF];
        loop {
            let n = cr.read(&mut buf).await?;
            if n == 0 {
                let _ = uw.shutdown().await;
                break;
            }
            uw.write_all(&buf[..n]).await?;
            act1.store(
                crate::engine::aqm_controller::monotonic_micros(std::time::Instant::now()),
                Ordering::Relaxed,
            );
            c1.tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
            c1.tx_packets.fetch_add(1, Ordering::Relaxed);
        }
        Ok::<(), io::Error>(())
    };

    let act2 = Arc::clone(&last_activity);
    let c2 = c;
    let upstream_to_client = async move {
        let mut buf = vec![0u8; RELAY_BUF];
        loop {
            let n = ur.read(&mut buf).await?;
            if n == 0 {
                let _ = cw.shutdown().await;
                break;
            }
            cw.write_all(&buf[..n]).await?;
            act2.store(
                crate::engine::aqm_controller::monotonic_micros(std::time::Instant::now()),
                Ordering::Relaxed,
            );
            c2.rx_bytes.fetch_add(n as u64, Ordering::Relaxed);
            c2.rx_packets.fetch_add(1, Ordering::Relaxed);
        }
        Ok::<(), io::Error>(())
    };

    let mut client_done = false;
    let mut upstream_done = false;

    tokio::pin!(client_to_upstream);
    tokio::pin!(upstream_to_client);

    let mut check_interval = tokio::time::interval(Duration::from_secs(15));
    check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            res = &mut client_to_upstream, if !client_done => {
                client_done = true;
                if let Err(e) = res {
                    report(&e);
                    return Err(e);
                }
                if upstream_done {
                    return Ok(());
                }
            }
            res = &mut upstream_to_client, if !upstream_done => {
                upstream_done = true;
                if let Err(e) = res {
                    report(&e);
                    return Err(e);
                }
                if client_done {
                    return Ok(());
                }
            }
            _ = check_interval.tick() => {
                let now_micros = crate::engine::aqm_controller::monotonic_micros(std::time::Instant::now());
                let last = last_activity.load(Ordering::Relaxed);
                if now_micros.saturating_sub(last) >= IDLE_TIMEOUT.as_micros() as u64 {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "tcp relay idle timeout"));
                }
            }
        }
    }
}

/// Peer resets/closes are normal relay termination; anything else is logged.
fn report(e: &io::Error) {
    use io::ErrorKind::*;
    if !matches!(e.kind(), ConnectionReset | BrokenPipe | ConnectionAborted | UnexpectedEof | NotConnected) {
        eprintln!("relay ended: {e}");
    }
}
