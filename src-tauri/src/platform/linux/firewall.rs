//! nftables transparent redirect + virtual interface (`lavinia0`) policy routing + crash-safe privileged watchdog.
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use crate::platform::{FirewallSpec, PlatformError};

const TABLE: &str = "lavinia";
const WATCHDOG_TICK_SECS: &str = "0.5";
const FWMARK: &str = "0x4c41";
const BYPASS_FWMARK: &str = "0x4c42";
const ROUTE_TABLE: u32 = 4177;
pub const VETH_IF: &str = "lavinia0";
pub const VETH_PEER: &str = "lavinia0-gw";
pub const VETH_ROUTING_TABLE: u32 = 4178;
const PRIVATE_NETS: &str = "10.0.0.0/8, 100.64.0.0/10, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16";

/// Owns the liveness flag; removing it (or process death) makes the watchdog delete the rules.
pub struct Guard {
    session_id: Option<String>,
    flag: Option<PathBuf>,
}

impl Guard {
    pub fn release(&mut self) {
        if let Some(session_id) = &self.session_id {
            let _ = super::proc::privileged_helper(&["cleanup-firewall", session_id]);
        }
        if let Some(flag) = &self.flag {
            let _ = fs::remove_file(flag);
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
           meta mark {bypass} return\n\
           socket transparent 1 return\n\
           fib daddr type {{ local, broadcast, multicast }} return\n\
           ip daddr 127.0.0.0/8 return\n\
           ip daddr 255.255.255.255 return\n\
           udp dport 53 return\n\
           ip daddr {phone} return\n\
           ip daddr {{ {private} }} return\n\
           oifname \"{veth}\" meta l4proto udp meta mark set {mark}\n\
           meta l4proto udp meta mark set {mark}\n\
         }}\n\
         chain tproxy_in {{\n\
           type filter hook prerouting priority -150; policy accept;\n\
           iifname \"{peer}\" udp dport != 53 meta mark set {mark} tproxy to 127.0.0.1:{udp} accept\n\
           meta mark {mark} meta l4proto udp tproxy to 127.0.0.1:{udp}\n\
         }}\n",
        phone = phone,
        private = PRIVATE_NETS,
        mark = FWMARK,
        bypass = BYPASS_FWMARK,
        veth = VETH_IF,
        peer = VETH_PEER,
        udp = udp_port,
    )
}

pub fn ruleset(spec: &FirewallSpec) -> String {
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
                 ip6 daddr {{ fe80::/10, ff02::/16 }} accept\n\
                 meta l4proto ipv6-icmp accept\n\
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
           chain nat_pre {{\n\
             type nat hook prerouting priority -100; policy accept;\n\
             iifname \"{peer}\" udp dport 53 redirect to :{dns}\n\
             iifname \"{peer}\" tcp dport 53 redirect to :{tcp}\n\
             iifname \"{peer}\" ip daddr != {{ 127.0.0.0/8, {phone}, {private} }} meta l4proto tcp redirect to :{tcp}\n\
           }}\n\
           chain nat_out {{\n\
             type nat hook output priority -100; policy accept;\n\
             meta mark {bypass} return\n\
             udp dport 53 ip daddr != 127.0.0.1 redirect to :{dns}\n\
             tcp dport 53 ip daddr != 127.0.0.1 redirect to :{tcp}\n\
             ip daddr 127.0.0.0/8 return\n\
             ip daddr {phone} return\n\
             ip daddr {{ {private} }} return\n\
             oifname \"{veth}\" meta l4proto tcp redirect to :{tcp}\n\
             meta l4proto tcp redirect to :{tcp}\n\
           }}\n\
           {udp_chains}\
           chain mss_clamp {{\n\
             type filter hook output priority -140; policy accept;\n\
             oifname \"lo\" return\n\
             fib daddr type {{ local, broadcast, multicast }} return\n\
             meta mark {bypass} return\n\
             ip daddr 127.0.0.0/8 return\n\
             ip daddr {phone} return\n\
             ip daddr {{ {private} }} return\n\
             oifname \"{veth}\" tcp flags syn tcp option maxseg size set 1380\n\
             tcp flags syn tcp option maxseg size set rt mtu\n\
           }}\n\
           chain filter_out {{\n\
             type filter hook output priority 0; policy accept;\n\
             oifname \"lo\" accept\n\
             oifname \"{veth}\" accept\n\
             fib daddr type {{ local, broadcast, multicast }} accept\n\
             meta mark {bypass} accept\n\
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
        bypass = BYPASS_FWMARK,
        veth = VETH_IF,
        peer = VETH_PEER,
        ip6_init = ip6_init,
        ip6_table = ip6_table,
    )
}

pub fn build_teardown_commands() -> String {
    format!(
        "nft delete table ip {t} 2>/dev/null || true; \
         nft delete table ip6 {t} 2>/dev/null || true; \
         while ip rule del priority 50 2>/dev/null; do :; done; \
         while ip rule del priority 55 2>/dev/null; do :; done; \
         while ip rule del priority 60 2>/dev/null; do :; done; \
         while ip rule del priority 100 2>/dev/null; do :; done; \
         ip route flush table {vrt} 2>/dev/null || true; \
         ip link del {veth} 2>/dev/null || true; \
         while ip rule del fwmark {m} lookup {rt} 2>/dev/null; do :; done; \
         ip route flush table {rt} 2>/dev/null || true",
        t = TABLE,
        vrt = VETH_ROUTING_TABLE,
        veth = VETH_IF,
        m = FWMARK,
        rt = ROUTE_TABLE,
    )
}

pub fn build_apply_script(spec: &FirewallSpec, flag_q: &str, pid: u32) -> String {
    let veth_setup = format!(
        "while ip rule del priority 50 2>/dev/null; do :; done\n\
         while ip rule del priority 55 2>/dev/null; do :; done\n\
         while ip rule del priority 60 2>/dev/null; do :; done\n\
         while ip rule del priority 100 2>/dev/null; do :; done\n\
         ip route flush table {vrt} 2>/dev/null || true\n\
         ip link del {veth} 2>/dev/null || true\n\
         ip link add {veth} type veth peer name {peer}\n\
         ip addr add 10.200.0.1/30 dev {veth}\n\
         ip addr add 10.200.0.2/30 dev {peer}\n\
         ip link set {veth} mtu 1420 up\n\
         ip link set {peer} mtu 1420 up\n\
         sysctl -w net.ipv4.conf.{veth}.rp_filter=0 >/dev/null 2>&1 || true\n\
         sysctl -w net.ipv4.conf.{peer}.rp_filter=0 >/dev/null 2>&1 || true\n\
         sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true\n\
         sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true\n\
         ip rule add to {phone} lookup main priority 50\n\
         ip rule add fwmark {bypass} lookup main priority 55\n\
         for cidr in 127.0.0.0/8 10.0.0.0/8 100.64.0.0/10 169.254.0.0/16 172.16.0.0/12 192.168.0.0/16 224.0.0.0/4 240.0.0.0/4; do\n\
             ip rule add to \"$cidr\" lookup main priority 60\n\
         done\n\
         ip route replace default via 10.200.0.2 dev {veth} table {vrt}\n\
         ip rule add lookup {vrt} priority 100\n",
        veth = VETH_IF,
        peer = VETH_PEER,
        vrt = VETH_ROUTING_TABLE,
        phone = spec.phone,
        bypass = BYPASS_FWMARK,
    );

    let route_add = if spec.udp_port.is_none() {
        String::new()
    } else {
        format!(
            "while ip rule del fwmark {m} lookup {rt} 2>/dev/null; do :; done\n\
             ip rule add fwmark {m} lookup {rt} priority 45\n\
             ip route replace local 0.0.0.0/0 dev lo table {rt}\n",
            m = FWMARK,
            rt = ROUTE_TABLE,
        )
    };

    let teardown = build_teardown_commands();

    format!(
        "set -e\n\
         {veth_setup}\
         {route_add}\
         nft -f - <<'LAVINIA_EOF'\n{rules}LAVINIA_EOF\n\
         setsid /bin/sh -c 'trap \"{teardown}; rm -f \\\"\\$1\\\"; exit 0\" TERM INT HUP EXIT; while [ -e \"$1\" ] && kill -0 \"$2\" 2>/dev/null; do sleep {tick}; done' _ {flag_q} {pid} >/dev/null 2>&1 < /dev/null &\n",
        veth_setup = veth_setup,
        route_add = route_add,
        rules = ruleset(spec),
        teardown = teardown,
        tick = WATCHDOG_TICK_SECS,
        flag_q = flag_q,
        pid = pid,
    )
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<Guard, PlatformError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let session_id = format!("lav_{:08x}{:08x}", std::process::id(), (now & 0xFFFFFFFF) as u32);

    if super::proc::helper_available() {
        let phone_str = spec.phone.to_string();
        let tcp_str = spec.tcp_port.to_string();
        let dns_str = spec.dns_port.to_string();
        let udp_str = spec.udp_port.map(|p| p.to_string()).unwrap_or_else(|| "0".into());
        let pid_str = std::process::id().to_string();

        super::proc::privileged_helper(&[
            "apply-firewall",
            &phone_str,
            &tcp_str,
            &dns_str,
            &udp_str,
            &session_id,
            &pid_str,
        ])?;

        Ok(Guard {
            session_id: Some(session_id),
            flag: None,
        })
    } else {
        let flag = flag_path()?;
        let guard = Guard { session_id: None, flag: Some(flag.clone()) };
        let flag_q = quote(flag.to_str().unwrap_or_default());
        let pid = std::process::id();
        let script = build_apply_script(spec, &flag_q, pid);
        super::proc::privileged_sh(&script)?;
        Ok(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn apply_script_includes_veth_pair_and_routing_table_4178() {
        let spec = FirewallSpec {
            phone: Ipv4Addr::new(192, 168, 43, 1),
            tcp_port: 41000,
            dns_port: 41053,
            udp_port: Some(41001),
        };
        let script = build_apply_script(&spec, "'/tmp/lavinia-test.run'", 12345);
        assert!(script.contains("ip link add lavinia0 type veth peer name lavinia0-gw"));
        assert!(script.contains("ip addr add 10.200.0.1/30 dev lavinia0"));
        assert!(script.contains("ip addr add 10.200.0.2/30 dev lavinia0-gw"));
        assert!(script.contains("ip route replace default via 10.200.0.2 dev lavinia0 table 4178"));
        assert!(script.contains("ip rule add to 192.168.43.1 lookup main priority 50"));
        assert!(script.contains("ip rule add fwmark 0x4c42 lookup main priority 55"));
        assert!(script.contains("ip rule add lookup 4178 priority 100"));
        assert!(script.contains("iifname \"lavinia0-gw\""));
        assert!(script.contains("oifname \"lavinia0\""));
    }

    #[test]
    fn teardown_commands_clean_veth_and_policy_routes() {
        let teardown = build_teardown_commands();
        assert!(teardown.contains("ip link del lavinia0"));
        assert!(teardown.contains("ip route flush table 4178"));
        assert!(teardown.contains("ip rule del priority 50"));
        assert!(teardown.contains("ip rule del priority 55"));
        assert!(teardown.contains("ip rule del priority 60"));
        assert!(teardown.contains("ip rule del priority 100"));
    }
}
