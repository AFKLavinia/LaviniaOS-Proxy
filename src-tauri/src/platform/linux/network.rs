//! NetworkManager integration: confirm the phone is reachable through a managed, connected link.
use std::net::Ipv4Addr;
use std::path::Path;

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

    if let Ok(listing) = run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"]) {
        for line in listing.lines() {
            let unescaped_line = line.replace("\\:", "\x00");
            let mut parts = unescaped_line.splitn(4, ':');
            let (Some(dev), Some(kind), Some(state)) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            let dev = dev.replace('\x00', ":");
            let kind = kind.replace('\x00', ":");
            let state = state.replace('\x00', ":");
            let name = parts.next().unwrap_or_default().replace('\x00', ":");
            if dev == device {
                if state.starts_with("connected") {
                    let usb = usb_hardware(&device);
                    return Ok(Uplink { kind: classify(&kind, usb.is_some()), hardware: usb, device, name });
                }
                if state == "unmanaged" {
                    return fallback_uplink(device);
                }
                return Err(PlatformError::Unreachable(format!(
                    "NetworkManager reports {device} as '{state}'; connect to the phone's Wi-Fi first"
                )));
            }
        }
    }

    fallback_uplink(device)
}

fn fallback_uplink(device: String) -> Result<Uplink, PlatformError> {
    let dev_path = format!("/sys/class/net/{device}");
    if !Path::new(&dev_path).exists() {
        return Err(PlatformError::Unreachable(format!("device {device} does not exist")));
    }
    let operstate = std::fs::read_to_string(format!("{dev_path}/operstate")).unwrap_or_default();
    let state = operstate.trim();
    if state == "down" {
        return Err(PlatformError::Unreachable(format!("interface {device} is down")));
    }
    let is_wifi = Path::new(&format!("{dev_path}/wireless")).exists()
        || Path::new(&format!("{dev_path}/phy80211")).exists();
    let usb = usb_hardware(&device);
    let kind = if is_wifi {
        LinkKind::Wifi
    } else if usb.is_some() {
        LinkKind::Usb
    } else {
        LinkKind::Ethernet
    };
    let name = device.clone();
    Ok(Uplink {
        kind,
        hardware: usb,
        device,
        name,
    })
}

fn classify(nm_type: &str, usb: bool) -> LinkKind {
    match nm_type {
        "wifi" => LinkKind::Wifi,
        "bluetooth" => LinkKind::Bluetooth,
        "gsm" | "cdma" | "wwan" => LinkKind::Cellular,
        _ if usb => LinkKind::Usb,
        "ethernet" => LinkKind::Ethernet,
        _ => LinkKind::Other,
    }
}

/// Manufacturer/product strings of the USB device backing `device`, if it is USB-attached.
fn usb_hardware(device: &str) -> Option<String> {
    let real = std::fs::canonicalize(format!("/sys/class/net/{device}/device")).ok()?;
    if !real.components().any(|c| c.as_os_str().to_string_lossy().starts_with("usb")) {
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
