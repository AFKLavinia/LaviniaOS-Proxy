//! NetworkManager integration: confirm the phone is reachable through a managed, connected link.
use std::net::Ipv4Addr;

use super::proc::run;
use crate::platform::{LinkKind, PlatformError, Uplink};

pub fn check_uplink(phone: Ipv4Addr) -> Result<Uplink, PlatformError> {
    let route = run("ip", &["-4", "route", "get", &phone.to_string()])?;
    let tokens: Vec<&str> = route.split_whitespace().collect();
    if tokens.first() == Some(&"local") {
        return Err(PlatformError::Unreachable("proxy address belongs to this machine".into()));
    }
    let device = tokens
        .windows(2)
        .find(|w| w[0] == "dev")
        .map(|w| w[1].to_string())
        .ok_or_else(|| PlatformError::Unreachable(format!("no route to {phone}")))?;
    if device == "lo" {
        return Err(PlatformError::Unreachable(format!("{phone} resolves to loopback")));
    }

    let listing = run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"])
        .map_err(|e| PlatformError::Command(format!("NetworkManager is required: {e}")))?;
    for line in listing.lines() {
        let mut parts = line.splitn(4, ':');
        let (Some(dev), Some(kind), Some(state)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let name = parts.next().unwrap_or_default().replace("\\:", ":");
        if dev == device {
            if state.starts_with("connected") {
                let usb = usb_hardware(&device);
                return Ok(Uplink { kind: classify(kind, usb.is_some()), hardware: usb, device, name });
            }
            return Err(PlatformError::Unreachable(format!(
                "NetworkManager reports {device} as '{state}'; connect to the phone's Wi-Fi first"
            )));
        }
    }
    Err(PlatformError::Unreachable(format!("{device} is not managed by NetworkManager")))
}

fn classify(nm_type: &str, usb: bool) -> LinkKind {
    match nm_type {
        "wifi" => LinkKind::Wifi,
        "bluetooth" => LinkKind::Bluetooth,
        "gsm" | "cdma" => LinkKind::Cellular,
        _ if usb => LinkKind::Usb,
        "ethernet" => LinkKind::Ethernet,
        _ => LinkKind::Other,
    }
}

/// Manufacturer/product strings of the USB device backing `device`, if it is USB-attached.
fn usb_hardware(device: &str) -> Option<String> {
    let real = std::fs::canonicalize(format!("/sys/class/net/{device}/device")).ok()?;
    if !real.components().any(|c| c.as_os_str() == "usb") {
        return None;
    }
    let read = |dir: &std::path::Path, f: &str| {
        std::fs::read_to_string(dir.join(f)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    };
    for dir in real.ancestors().take(4) {
        if let Some(product) = read(dir, "product") {
            return Some(match read(dir, "manufacturer") {
                Some(m) if !product.contains(&m) => format!("{m} {product}"),
                _ => product,
            });
        }
    }
    Some("USB device".to_string())
}
