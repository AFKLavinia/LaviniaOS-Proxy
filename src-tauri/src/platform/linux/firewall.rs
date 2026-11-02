//! nftables transparent redirect + leak prevention, with a crash-safe privileged watchdog.
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use super::proc::privileged_sh;
use crate::platform::{FirewallSpec, PlatformError};

const TABLE: &str = "lavinia";
const WATCHDOG_TICK_SECS: u32 = 1;
const FWMARK: &str = "0x4c41";
const ROUTE_TABLE: u32 = 4177;
const PRIVATE_NETS: &str = "10.0.0.0/8, 100.64.0.0/10, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16";

/// Owns the liveness flag; removing it (or process death) makes the watchdog delete the rules.
pub struct Guard {
    flag: PathBuf,
}

impl Guard {
    pub fn release(&mut self) {
        if let Err(e) = fs::remove_file(&self.flag) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("failed to remove watchdog flag: {e}");
            }
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.release();
    }
}

fn flag_path() -> Result<PathBuf, PlatformError> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("lavinia-{}.run", std::process::id()));
    let ok = path
        .to_str()
        .is_some_and(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c)));
    if !ok {
        return Err(PlatformError::Command("runtime directory path contains unsafe characters".into()));
    }
    if let Err(e) = fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(PlatformError::Io(e));
        }
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    Ok(path)
}

/// Marks outbound UDP and diverts it (TPROXY) to the local relay; only installed when UDP is usable.
fn udp_chains(phone: &std::net::Ipv4Addr, udp_port: &u16) -> String {
    format!(
        "chain mark_out {{\n\
           type route hook output priority -150; policy accept;\n\
           socket transparent 1 return\n\
           fib daddr type {{ local, broadcast, multicast }} return\n\
           ip daddr 127.0.0.0/8 return\n\
           ip daddr 255.255.255.255 return\n\
           udp dport 53 return\n\
           ip daddr {phone} return\n\
           ip daddr {{ {private} }} return\n\
           meta l4proto udp meta mark set {mark}\n\
         }}\n\
         chain tproxy_in {{\n\
           type filter hook prerouting priority -150; policy accept;\n\
           meta mark {mark} meta l4proto udp tproxy to :{udp}\n\
         }}\n",
        phone = phone,
        private = PRIVATE_NETS,
        mark = FWMARK,
        udp = udp_port,
    )
}

fn ruleset(spec: &FirewallSpec) -> String {
    let FirewallSpec { phone, tcp_port, dns_port, udp_port } = spec;
    let udp_chains = udp_port.map(|p| udp_chains(phone, &p)).unwrap_or_default();
    let udp_accept = if udp_port.is_some() {
        format!("            meta mark {mark} accept\n", mark = FWMARK)
    } else {
        String::new()
    };
    let ip6_init = if std::path::Path::new("/proc/sys/net/ipv6").exists() {
        format!("table ip6 {t}\ndelete table ip6 {t}\n", t = TABLE)
    } else {
        String::new()
    };
    let ip6_table = if std::path::Path::new("/proc/sys/net/ipv6").exists() {
        format!(
            "table ip6 {t} {{\n\
               chain filter_out {{\n\
                 type filter hook output priority 0; policy accept;\n\
                 oifname \"lo\" accept\n\
                 udp dport 53 reject\n\
                 tcp dport 53 reject\n\
                 ip6 daddr fe80::/10 accept\n\
                 reject\n\
               }}\n\
             }}\n",
            t = TABLE,
        )
    } else {
        String::new()
    };

    format!(
        "table ip {t}\n\
         delete table ip {t}\n\
         {ip6_init}\
         table ip {t} {{\n\
           chain nat_out {{\n\
             type nat hook output priority -100; policy accept;\n\
             ip daddr 127.0.0.0/8 return\n\
             ip daddr {phone} return\n\
             ip daddr {{ {private} }} return\n\
             udp dport 53 redirect to :{dns}\n\
             meta l4proto tcp redirect to :{tcp}\n\
           }}\n\
           {udp_chains}\
           chain filter_out {{\n\
             type filter hook output priority 0; policy accept;\n\
             oifname \"lo\" accept\n\
             fib daddr type {{ local, broadcast, multicast }} accept\n\
             ip daddr 127.0.0.0/8 accept\n\
             udp sport 68 udp dport 67 accept\n\
             ip daddr {phone} accept\n\
             ip daddr {{ {private} }} accept\n\
             {udp_accept}\
             meta l4proto udp reject\n\
           }}\n\
         }}\n\
         {ip6_table}",
        t = TABLE,
        dns = dns_port,
        tcp = tcp_port,
        udp_chains = udp_chains,
        udp_accept = udp_accept,
        phone = phone,
        private = PRIVATE_NETS,
        ip6_init = ip6_init,
        ip6_table = ip6_table,
    )
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<Guard, PlatformError> {
    let flag = flag_path()?;
    let guard = Guard { flag: flag.clone() };
    let flag_q = quote(flag.to_str().unwrap_or_default());
    let pid = std::process::id();

    let route_add = if spec.udp_port.is_none() {
        String::new()
    } else {
        format!(
            "while ip rule del fwmark {m} lookup {rt} 2>/dev/null; do :; done\n\
             ip rule add fwmark {m} lookup {rt}\n\
             ip route replace local 0.0.0.0/0 dev lo table {rt}\n",
            m = FWMARK,
            rt = ROUTE_TABLE,
        )
    };
    let script = format!(
        "set -e\n\
         {route_add}\
         nft -f - <<'LAVINIA_EOF'\n{rules}LAVINIA_EOF\n\
         setsid /bin/sh -c 'trap \"nft delete table ip {t} 2>/dev/null; nft delete table ip6 {t} 2>/dev/null; while ip rule del fwmark {m} lookup {rt} 2>/dev/null; do :; done; ip route flush table {rt} 2>/dev/null; rm -f \\\"\\$1\\\"; exit 0\" TERM INT HUP EXIT; while [ -e \"$1\" ] && kill -0 \"$2\" 2>/dev/null; do sleep {tick}; done' _ {flag_q} {pid} >/dev/null 2>&1 < /dev/null &\n",
        route_add = route_add,
        rules = ruleset(spec),
        t = TABLE,
        m = FWMARK,
        rt = ROUTE_TABLE,
        tick = WATCHDOG_TICK_SECS,
        flag_q = flag_q,
        pid = pid,
    );
    privileged_sh(&script)?;
    Ok(guard)
}
