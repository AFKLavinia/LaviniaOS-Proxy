//! nftables transparent redirect + leak prevention, with a crash-safe privileged watchdog.
use std::fs::{self, File};
use std::path::PathBuf;

use super::proc::privileged_sh;
use crate::platform::{FirewallSpec, PlatformError};

const TABLE: &str = "lavinia";
const WATCHDOG_TICK_SECS: u32 = 1;
const FWMARK: &str = "0x4c41";
const ROUTE_TABLE: u32 = 4177;
const PRIVATE_NETS: &str = "10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16";

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
    if ok {
        Ok(path)
    } else {
        Err(PlatformError::Command("runtime directory path contains unsafe characters".into()))
    }
}

/// Marks outbound UDP and diverts it (TPROXY) to the local relay; only installed when UDP is usable.
fn udp_chains(phone: &std::net::Ipv4Addr, udp_port: &u16) -> String {
    format!(
        "chain mark_out {{\n\
           type route hook output priority -150; policy accept;\n\
           fib daddr type local return\n\
           ip daddr 127.0.0.0/8 return\n\
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
    format!(
        "table ip {t}\n\
         delete table ip {t}\n\
         table ip6 {t}\n\
         delete table ip6 {t}\n\
         table ip {t} {{\n\
           chain nat_out {{\n\
             type nat hook output priority -100; policy accept;\n\
             ip daddr 127.0.0.0/8 return\n\
             udp dport 53 redirect to :{dns}\n\
             ip daddr {phone} return\n\
             ip daddr {{ {private} }} return\n\
             meta l4proto tcp redirect to :{tcp}\n\
           }}\n\
           {udp_chains}\
           chain filter_out {{\n\
             type filter hook output priority 0; policy accept;\n\
             oifname \"lo\" accept\n\
             fib daddr type local accept\n\
             ip daddr 127.0.0.0/8 accept\n\
             ip daddr {phone} accept\n\
             ip daddr {{ {private} }} accept\n\
             meta l4proto udp reject\n\
           }}\n\
         }}\n\
         table ip6 {t} {{\n\
           chain filter_out {{\n\
             type filter hook output priority 0; policy accept;\n\
             oifname \"lo\" accept\n\
             ip6 daddr fe80::/10 accept\n\
             drop\n\
           }}\n\
         }}\n",
        t = TABLE,
        dns = dns_port,
        tcp = tcp_port,
        udp_chains = udp_chains,
        phone = phone,
        private = PRIVATE_NETS,
    )
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<Guard, PlatformError> {
    let flag = flag_path()?;
    File::create(&flag)?;
    let guard = Guard { flag: flag.clone() };
    let flag_q = quote(flag.to_str().unwrap_or_default());
    let pid = std::process::id();

    let route_add = if spec.udp_port.is_none() {
        String::new()
    } else {
        format!(
        "ip rule del fwmark {m} lookup {rt} 2>/dev/null || true\n\
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
         setsid /bin/sh -c 'while [ -e {flag} ] && kill -0 {pid} 2>/dev/null; do sleep {tick}; done; \
         nft delete table ip {t}; nft delete table ip6 {t}; \
         ip rule del fwmark {m} lookup {rt}; ip route flush table {rt}' >/dev/null 2>&1 </dev/null &\n",
        route_add = route_add,
        rules = ruleset(spec),
        flag = flag_q,
        pid = pid,
        tick = WATCHDOG_TICK_SECS,
        t = TABLE,
        m = FWMARK,
        rt = ROUTE_TABLE,
    );
    privileged_sh(&script)?;
    Ok(guard)
}
