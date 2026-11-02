//! Minimal SOCKS5 client (CONNECT + UDP ASSOCIATE; no-auth or username/password).
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const VER: u8 = 5;
const METHOD_NO_AUTH: u8 = 0;
const METHOD_USERPASS: u8 = 2;
const METHOD_NONE_ACCEPTABLE: u8 = 0xff;
const AUTH_VER: u8 = 1;
const CMD_CONNECT: u8 = 1;
const CMD_UDP_ASSOCIATE: u8 = 3;
const ATYP_V4: u8 = 1;
const ATYP_DOMAIN: u8 = 3;
const ATYP_V6: u8 = 4;
const REP_OK: u8 = 0;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// RFC 1929 credentials. Deliberately no `Debug`/`Display` so they cannot leak into logs.
pub struct Auth {
    user: Vec<u8>,
    pass: Vec<u8>,
}

impl Auth {
    /// Both fields must be 1..=255 bytes (protocol limit).
    pub fn new(user: &str, pass: &str) -> Option<Self> {
        let ok = |s: &str| (1..=255).contains(&s.len());
        (ok(user) && ok(pass)).then(|| Self { user: user.as_bytes().to_vec(), pass: pass.as_bytes().to_vec() })
    }
}

/// SOCKS5 upstream endpoint with optional credentials; cheap to clone.
#[derive(Clone)]
pub struct Upstream {
    pub addr: SocketAddr,
    auth: Option<Arc<Auth>>,
}

impl Upstream {
    pub fn new(addr: SocketAddr, auth: Option<Auth>) -> Self {
        Self { addr, auth: auth.map(Arc::new) }
    }
}

fn proto(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Method selection plus (when negotiated) username/password sub-negotiation.
async fn negotiate(s: &mut TcpStream, up: &Upstream) -> io::Result<()> {
    match &up.auth {
        Some(_) => s.write_all(&[VER, 1, METHOD_USERPASS]).await?,
        None => s.write_all(&[VER, 1, METHOD_NO_AUTH]).await?,
    }
    let mut reply = [0u8; 2];
    s.read_exact(&mut reply).await?;
    if reply[0] != VER {
        return Err(proto("invalid socks5 reply version"));
    }
    match (reply[1], &up.auth) {
        (METHOD_NO_AUTH, None) => Ok(()),
        (METHOD_NO_AUTH, Some(_)) => Err(proto("socks5 server downgraded authentication")),
        (METHOD_USERPASS, Some(auth)) => {
            let mut msg = Vec::with_capacity(3 + auth.user.len() + auth.pass.len());
            msg.push(AUTH_VER);
            msg.push(auth.user.len() as u8);
            msg.extend_from_slice(&auth.user);
            msg.push(auth.pass.len() as u8);
            msg.extend_from_slice(&auth.pass);
            s.write_all(&msg).await?;
            let mut status = [0u8; 2];
            s.read_exact(&mut status).await?;
            if status[0] != AUTH_VER {
                return Err(proto("invalid socks5 auth reply version"));
            }
            if status[1] != 0 {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "socks5 authentication failed"));
            }
            Ok(())
        }
        (METHOD_NONE_ACCEPTABLE, None) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socks5 server requires a username and password",
        )),
        _ => Err(proto("socks5 server rejected the offered auth methods")),
    }
}

async fn with_timeout<T>(limit: Duration, what: &'static str, f: impl std::future::Future<Output = io::Result<T>>) -> io::Result<T> {
    match timeout(limit, f).await {
        Ok(r) => r,
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, what)),
    }
}

pub async fn connect(proxy: &Upstream, dst: SocketAddr) -> io::Result<TcpStream> {
    with_timeout(HANDSHAKE_TIMEOUT, "socks5 handshake timed out", handshake(proxy, dst)).await
}

/// Health probe: TCP connect plus SOCKS5 negotiation (including auth) only. Returns RTT duration.
pub async fn probe(proxy: &Upstream) -> io::Result<Duration> {
    let negotiate = async {
        let start = std::time::Instant::now();
        let mut s = TcpStream::connect(proxy.addr).await?;
        s.set_nodelay(true)?;
        negotiate(&mut s, proxy).await?;
        Ok(start.elapsed())
    };
    with_timeout(PROBE_TIMEOUT, "socks5 probe timed out", negotiate).await
}

async fn handshake(proxy: &Upstream, dst: SocketAddr) -> io::Result<TcpStream> {
    let mut s = TcpStream::connect(proxy.addr).await?;
    s.set_nodelay(true)?;
    negotiate(&mut s, proxy).await?;

    let mut req = Vec::with_capacity(22);
    req.extend_from_slice(&[VER, CMD_CONNECT, 0]);
    match dst {
        SocketAddr::V4(a) => {
            req.push(ATYP_V4);
            req.extend_from_slice(&a.ip().octets());
        }
        SocketAddr::V6(a) => {
            req.push(ATYP_V6);
            req.extend_from_slice(&a.ip().octets());
        }
    }
    req.extend_from_slice(&dst.port().to_be_bytes());
    s.write_all(&req).await?;
    read_reply(&mut s, "socks5 connect failed").await?;
    Ok(s)
}

/// Reads a command reply; returns the bound address.
async fn read_reply(s: &mut TcpStream, failure: &'static str) -> io::Result<(IpAddr, u16)> {
    let mut head = [0u8; 4];
    s.read_exact(&mut head).await?;
    if head[0] != VER {
        return Err(proto("invalid socks5 reply version"));
    }
    if head[1] != REP_OK {
        return Err(io::Error::new(io::ErrorKind::ConnectionRefused, format!("{failure} (code {})", head[1])));
    }
    if head[2] != 0 {
        return Err(proto("invalid socks5 reserved byte"));
    }
    let ip = match head[3] {
        ATYP_V4 => {
            let mut o = [0u8; 4];
            s.read_exact(&mut o).await?;
            IpAddr::V4(Ipv4Addr::from(o))
        }
        ATYP_V6 => {
            let mut o = [0u8; 16];
            s.read_exact(&mut o).await?;
            IpAddr::V6(o.into())
        }
        ATYP_DOMAIN => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            let len = usize::from(l[0]);
            let mut name = [0u8; 255];
            s.read_exact(&mut name[..len]).await?;
            if let Ok(domain_str) = std::str::from_utf8(&name[..len]) {
                if let Ok(ip) = domain_str.parse::<IpAddr>() {
                    ip
                } else if let Ok(mut addrs) = tokio::net::lookup_host((domain_str, 0)).await {
                    addrs.next().map(|sa| sa.ip()).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
                } else {
                    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                }
            } else {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            }
        }
        _ => return Err(proto("unknown socks5 address type")),
    };
    let mut port = [0u8; 2];
    s.read_exact(&mut port).await?;
    Ok((ip, u16::from_be_bytes(port)))
}

/// Opens a UDP association. The returned TCP stream must stay open for the association's lifetime.
/// Returns the relay endpoint datagrams must be sent to.
pub async fn udp_associate(proxy: &Upstream) -> io::Result<(TcpStream, SocketAddr)> {
    with_timeout(HANDSHAKE_TIMEOUT, "socks5 udp associate timed out", associate(proxy)).await
}

async fn associate(proxy: &Upstream) -> io::Result<(TcpStream, SocketAddr)> {
    let mut s = TcpStream::connect(proxy.addr).await?;
    s.set_nodelay(true)?;
    negotiate(&mut s, proxy).await?;
    // Zero address/port: our outgoing UDP source is not known in advance.
    s.write_all(&[VER, CMD_UDP_ASSOCIATE, 0, ATYP_V4, 0, 0, 0, 0, 0, 0]).await?;
    let (ip, port) = read_reply(&mut s, "socks5 udp associate failed").await?;
    if port == 0 {
        return Err(proto("socks5 udp associate returned port 0"));
    }
    // Servers commonly answer 0.0.0.0: the relay then lives on the proxy host itself.
    let ip = if ip.is_unspecified() { proxy.addr.ip() } else { ip };
    Ok((s, SocketAddr::new(ip, port)))
}

/// Checks that the upstream grants UDP associations.
pub async fn probe_udp(proxy: &Upstream) -> io::Result<()> {
    with_timeout(PROBE_TIMEOUT, "socks5 udp probe timed out", associate(proxy)).await.map(|_| ())
}

/// Frames a payload for the SOCKS5 UDP relay (no fragmentation).
pub fn encode_udp(dst: SocketAddrV4, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + payload.len());
    out.extend_from_slice(&[0, 0, 0, ATYP_V4]);
    out.extend_from_slice(&dst.ip().octets());
    out.extend_from_slice(&dst.port().to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// Parses a relay datagram into (source, payload). Fragmented/non-IPv4 datagrams are rejected.
pub fn decode_udp(buf: &[u8]) -> Option<(SocketAddrV4, &[u8])> {
    if buf.len() < 10 || buf[2] != 0 || buf[3] != ATYP_V4 {
        return None;
    }
    let ip = Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]);
    let port = u16::from_be_bytes([buf[8], buf[9]]);
    Some((SocketAddrV4::new(ip, port), &buf[10..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn udp_frame_roundtrip() {
        let dst = SocketAddrV4::new(Ipv4Addr::new(162, 159, 130, 233), 50001);
        let frame = encode_udp(dst, b"voice");
        assert_eq!(decode_udp(&frame), Some((dst, &b"voice"[..])));
    }

    #[test]
    fn udp_decode_rejects_fragments_and_short_frames() {
        assert!(decode_udp(&[0, 0, 1, ATYP_V4, 1, 2, 3, 4, 0, 80]).is_none());
        assert!(decode_udp(&[0, 0, 0, ATYP_V4, 1, 2]).is_none());
        assert!(decode_udp(&[0, 0, 0, ATYP_DOMAIN, 1, 2, 3, 4, 0, 80]).is_none());
    }

    #[test]
    fn auth_rejects_empty_and_oversized_fields() {
        assert!(Auth::new("", "x").is_none());
        assert!(Auth::new("u", &"p".repeat(256)).is_none());
        assert!(Auth::new("u", "p").is_some());
    }

    /// Fake upstream: requires user/pass "alice"/"secret", then echoes after a CONNECT.
    async fn fake_server() -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else { return };
                tokio::spawn(async move {
                    let mut hello = [0u8; 2];
                    s.read_exact(&mut hello).await.unwrap();
                    let mut methods = vec![0u8; usize::from(hello[1])];
                    s.read_exact(&mut methods).await.unwrap();
                    if !methods.contains(&METHOD_USERPASS) {
                        s.write_all(&[VER, METHOD_NONE_ACCEPTABLE]).await.unwrap();
                        return;
                    }
                    s.write_all(&[VER, METHOD_USERPASS]).await.unwrap();
                    let mut h = [0u8; 2];
                    s.read_exact(&mut h).await.unwrap();
                    let mut user = vec![0u8; usize::from(h[1])];
                    s.read_exact(&mut user).await.unwrap();
                    let mut pl = [0u8; 1];
                    s.read_exact(&mut pl).await.unwrap();
                    let mut pass = vec![0u8; usize::from(pl[0])];
                    s.read_exact(&mut pass).await.unwrap();
                    if user != b"alice" || pass != b"secret" {
                        s.write_all(&[AUTH_VER, 1]).await.unwrap();
                        return;
                    }
                    s.write_all(&[AUTH_VER, 0]).await.unwrap();
                    let mut req = [0u8; 10];
                    s.read_exact(&mut req).await.unwrap();
                    s.write_all(&[VER, 0, 0, ATYP_V4, 0, 0, 0, 0, 0, 0]).await.unwrap();
                    let mut b = [0u8; 4];
                    if s.read_exact(&mut b).await.is_ok() {
                        s.write_all(&b).await.unwrap();
                    }
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn connect_with_valid_credentials_relays_data() {
        let addr = fake_server().await;
        let up = Upstream::new(addr, Auth::new("alice", "secret"));
        let mut s = connect(&up, "1.2.3.4:80".parse().unwrap()).await.unwrap();
        s.write_all(b"ping").await.unwrap();
        let mut back = [0u8; 4];
        s.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"ping");
    }

    #[tokio::test]
    async fn connect_with_wrong_or_missing_credentials_is_denied() {
        let addr = fake_server().await;
        let bad = Upstream::new(addr, Auth::new("alice", "nope"));
        let e = connect(&bad, "1.2.3.4:80".parse().unwrap()).await.unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        let none = Upstream::new(addr, None);
        let e = connect(&none, "1.2.3.4:80".parse().unwrap()).await.unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    }

    #[tokio::test]
    async fn auth_downgrade_is_rejected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut hello = [0u8; 2];
            s.read_exact(&mut hello).await.unwrap();
            let mut methods = vec![0u8; usize::from(hello[1])];
            s.read_exact(&mut methods).await.unwrap();
            // Rogue server tries to force METHOD_NO_AUTH when client offered USERPASS
            s.write_all(&[VER, METHOD_NO_AUTH]).await.unwrap();
        });
        let up = Upstream::new(addr, Auth::new("alice", "secret"));
        let e = connect(&up, "1.2.3.4:80".parse().unwrap()).await.unwrap_err();
        assert_eq!(e.to_string(), "socks5 server downgraded authentication");
    }

    #[tokio::test]
    async fn auth_reply_invalid_version_is_rejected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut hello = [0u8; 2];
            s.read_exact(&mut hello).await.unwrap();
            let mut methods = vec![0u8; usize::from(hello[1])];
            s.read_exact(&mut methods).await.unwrap();
            s.write_all(&[VER, METHOD_USERPASS]).await.unwrap();
            let mut h = [0u8; 2];
            s.read_exact(&mut h).await.unwrap();
            let mut user = vec![0u8; usize::from(h[1])];
            s.read_exact(&mut user).await.unwrap();
            let mut pl = [0u8; 1];
            s.read_exact(&mut pl).await.unwrap();
            let mut pass = vec![0u8; usize::from(pl[0])];
            s.read_exact(&mut pass).await.unwrap();
            // Invalid auth version 99
            s.write_all(&[99, 0]).await.unwrap();
        });
        let up = Upstream::new(addr, Auth::new("alice", "secret"));
        let e = connect(&up, "1.2.3.4:80".parse().unwrap()).await.unwrap_err();
        assert_eq!(e.to_string(), "invalid socks5 auth reply version");
    }

    #[tokio::test]
    async fn rsv_byte_non_zero_is_rejected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut hello = [0u8; 2];
            s.read_exact(&mut hello).await.unwrap();
            let mut methods = vec![0u8; usize::from(hello[1])];
            s.read_exact(&mut methods).await.unwrap();
            s.write_all(&[VER, METHOD_NO_AUTH]).await.unwrap();
            let mut req = [0u8; 10];
            s.read_exact(&mut req).await.unwrap();
            // Reply with RSV = 1 (invalid)
            s.write_all(&[VER, 0, 1, ATYP_V4, 127, 0, 0, 1, 4, 210]).await.unwrap();
        });
        let up = Upstream::new(addr, None);
        let e = connect(&up, "1.2.3.4:80".parse().unwrap()).await.unwrap_err();
        assert_eq!(e.to_string(), "invalid socks5 reserved byte");
    }

    #[tokio::test]
    async fn udp_associate_port_zero_is_rejected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut hello = [0u8; 2];
            s.read_exact(&mut hello).await.unwrap();
            let mut methods = vec![0u8; usize::from(hello[1])];
            s.read_exact(&mut methods).await.unwrap();
            s.write_all(&[VER, METHOD_NO_AUTH]).await.unwrap();
            let mut req = [0u8; 10];
            s.read_exact(&mut req).await.unwrap();
            // Reply with port = 0
            s.write_all(&[VER, 0, 0, ATYP_V4, 127, 0, 0, 1, 0, 0]).await.unwrap();
        });
        let up = Upstream::new(addr, None);
        let e = udp_associate(&up).await.unwrap_err();
        assert_eq!(e.to_string(), "socks5 udp associate returned port 0");
    }
}
