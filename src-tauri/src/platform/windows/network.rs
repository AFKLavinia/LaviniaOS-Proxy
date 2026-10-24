//! Windows network interface and route detection.
use std::net::Ipv4Addr;

use super::proc::run_powershell;
use crate::platform::{LinkKind, PlatformError, Uplink};

pub fn check_uplink(phone: Ipv4Addr) -> Result<Uplink, PlatformError> {
    // Find the network adapter reaching the phone IP using PowerShell Get-NetRoute
    let script = format!(
        "$route = Get-NetRoute -DestinationPrefix '{phone}/32' -ErrorAction SilentlyContinue | Select-Object -First 1; \
        if (-not $route) {{ $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 1 }}; \
        if ($route) {{ \
            $iface = Get-NetAdapter -InterfaceIndex $route.InterfaceIndex; \
            Write-Output \"$($iface.Name)|$($iface.InterfaceDescription)|$($iface.Status)|$($iface.MediaType)\" \
        }}"
    );

    let output = run_powershell(&script)?;
    let line = output.lines().find(|l| !l.trim().is_empty()).ok_or_else(|| {
        PlatformError::Unreachable(format!("no active route found to reach {phone}"))
    })?;

    let parts: Vec<&str> = line.trim().split('|').collect();
    if parts.len() < 4 {
        return Err(PlatformError::Unreachable(format!("invalid network adapter info for {phone}")));
    }

    let dev_name = parts[0].to_string();
    let dev_desc = parts[1].to_string();
    let status = parts[2];
    let media = parts[3].to_lowercase();

    if !status.eq_ignore_ascii_case("Up") {
        return Err(PlatformError::Unreachable(format!(
            "adapter {dev_name} ({dev_desc}) is down"
        )));
    }

    let kind = classify(&dev_desc, &media);
    let hardware = if kind == LinkKind::Usb {
        Some(dev_desc.clone())
    } else {
        None
    };

    Ok(Uplink {
        device: dev_name.clone(),
        kind,
        name: dev_desc,
        hardware,
    })
}

fn classify(desc: &str, media: &str) -> LinkKind {
    let lower_desc = desc.to_lowercase();
    if lower_desc.contains("wi-fi") || lower_desc.contains("wireless") || media.contains("native 802.11") {
        LinkKind::Wifi
    } else if lower_desc.contains("bluetooth") {
        LinkKind::Bluetooth
    } else if lower_desc.contains("cellular") || lower_desc.contains("mobile") {
        LinkKind::Cellular
    } else if lower_desc.contains("usb") || lower_desc.contains("remote ndis") || lower_desc.contains("rndis") {
        LinkKind::Usb
    } else if lower_desc.contains("ethernet") || media.contains("802.3") {
        LinkKind::Ethernet
    } else {
        LinkKind::Other
    }
}
