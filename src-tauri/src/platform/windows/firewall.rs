//! Windows routing and firewall / DNS management for Wintun transparent proxying.
use std::fs::{self, File};
use std::net::Ipv4Addr;
use std::path::PathBuf;

use super::proc::privileged_powershell;
use crate::platform::{FirewallSpec, PlatformError};

/// Owns the liveness flag file on Windows; removal triggers cleanup in the watchdog.
pub struct Guard {
    flag: PathBuf,
    phone: Ipv4Addr,
}

impl Guard {
    pub fn release(&mut self) {
        if let Err(e) = fs::remove_file(&self.flag) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("failed to remove Windows watchdog flag: {e}");
            }
        }
        // Also perform immediate cleanup
        let _ = cleanup(&self.phone);
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.release();
    }
}

fn flag_path() -> Result<PathBuf, PlatformError> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("lavinia-{}.run", std::process::id()));
    Ok(path)
}

fn cleanup(phone: &Ipv4Addr) -> Result<(), PlatformError> {
    let script = format!(
        "Remove-NetRoute -DestinationPrefix '{phone}/32' -Confirm:$false -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block Leaks' -ErrorAction SilentlyContinue; \
         Set-NetIPInterface -InterfaceAlias 'Lavinia' -InterfaceMetric 256 -ErrorAction SilentlyContinue"
    );
    privileged_powershell(&script)
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<Guard, PlatformError> {
    let flag = flag_path()?;
    File::create(&flag)?;
    let guard = Guard {
        flag: flag.clone(),
        phone: spec.phone,
    };

    let flag_str = flag.to_string_lossy();
    let pid = std::process::id();
    let phone = spec.phone;

    // 1. Route phone's IP via existing physical adapter to prevent proxy connection loop
    // 2. Set default route metric or configure Wintun interface
    // 3. Spawn a detached background PowerShell watchdog process that monitors the PID and flag file
    let script = format!(
        "$uplink = Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 1; \
         if ($uplink) {{ \
             New-NetRoute -DestinationPrefix '{phone}/32' -InterfaceIndex $uplink.InterfaceIndex -NextHop $uplink.NextHop -Metric 1 -ErrorAction SilentlyContinue \
         }}; \
         Start-Process powershell.exe -WindowStyle Hidden -ArgumentList '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"& {{ \
             while (Test-Path ''{flag_str}'') {{ \
                 if (-not (Get-Process -Id {pid} -ErrorAction SilentlyContinue)) {{ break }}; \
                 Start-Sleep -Seconds 1 \
             }}; \
             Remove-NetRoute -DestinationPrefix ''{phone}/32'' -Confirm:$false -ErrorAction SilentlyContinue; \
             Remove-Item -Path ''{flag_str}'' -ErrorAction SilentlyContinue \
         }}\"'"
    );

    privileged_powershell(&script)?;
    Ok(guard)
}
