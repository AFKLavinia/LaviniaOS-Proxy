//! Windows network interface and route detection.
use std::net::Ipv4Addr;

use super::proc::run_powershell;
use crate::platform::{LinkKind, PlatformError, Uplink};

pub fn check_uplink(phone: Ipv4Addr) -> Result<Uplink, PlatformError> {
    // Find the network adapter reaching the phone IP using PowerShell Find-NetRoute
    let script = format!(
        "$r = Find-NetRoute -RemoteIPAddress '{phone}' -ErrorAction SilentlyContinue | Select-Object -First 1; \
        if (-not $r) {{ $r = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1 }}; \
        if ($r) {{ \
            $iface = Get-NetAdapter -InterfaceIndex $r.InterfaceIndex -ErrorAction SilentlyContinue; \
            if ($iface) {{ \
                Write-Output \"$($iface.Name)|$($iface.InterfaceDescription)|$($iface.Status)|$($iface.MediaType)|$($iface.PnPDeviceID)\" \
            }} \
        }}"
    );

    let output = run_powershell(&script)?;
    let line = output.lines().find(|l| !l.trim().is_empty()).ok_or_else(|| {
        PlatformError::Unreachable(format!("no active route found to reach {phone}"))
    })?;

    let parts: Vec<&str> = line.trim().split('|').collect();
    if parts.len() < 4 {
        return Err(PlatformError::Unreachable(format!(
            "invalid network adapter info for {phone}"
        )));
    }

    let dev_name = parts[0].to_string();
    let dev_desc = parts[1].to_string();
    let status = parts[2];
    let media = parts[3].to_lowercase();
    let pnp_id = parts.get(4).copied().unwrap_or("");

    if !status.eq_ignore_ascii_case("Up") {
        return Err(PlatformError::Unreachable(format!(
            "adapter {dev_name} ({dev_desc}) is down"
        )));
    }

    let kind = classify(&dev_desc, &media, pnp_id);
    let hardware = if kind == LinkKind::Usb {
        Some(dev_desc.clone())
    } else {
        None
    };

    Ok(Uplink {
        device: dev_name,
        kind,
        name: dev_desc,
        hardware,
    })
}

fn classify(desc: &str, media: &str, pnp_id: &str) -> LinkKind {
    let lower_desc = desc.to_lowercase();
    let upper_pnp = pnp_id.to_uppercase();

    // Check USB tethering indicators, common virtual tethering adapters, and PnP Device ID
    if lower_desc.contains("usb")
        || lower_desc.contains("rndis")
        || lower_desc.contains("remote ndis")
        || lower_desc.contains("apple mobile")
        || lower_desc.contains("pdanet")
        || lower_desc.contains("android")
        || upper_pnp.contains("USB\\")
    {
        LinkKind::Usb
    } else if lower_desc.contains("wi-fi")
        || lower_desc.contains("wireless")
        || media.contains("native 802.11")
    {
        LinkKind::Wifi
    } else if lower_desc.contains("bluetooth") {
        LinkKind::Bluetooth
    } else if lower_desc.contains("cellular") || lower_desc.contains("mobile") {
        LinkKind::Cellular
    } else if lower_desc.contains("ethernet") || media.contains("802.3") {
        LinkKind::Ethernet
    } else {
        LinkKind::Other
    }
}
