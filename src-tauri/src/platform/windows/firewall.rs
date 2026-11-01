//! Windows routing and firewall / DNS management for Wintun transparent proxying.
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use super::proc::privileged_powershell;
use crate::platform::{FirewallSpec, PlatformError};

/// Owns the liveness flag file on Windows; removal triggers cleanup in the background watchdog.
pub struct Guard {
    flag: PathBuf,
    #[allow(dead_code)]
    phone: Ipv4Addr,
}

impl Guard {
    /// Signals the privileged watchdog to remove routing and firewall rules.
    pub fn release(&mut self) {
        if let Err(e) = fs::remove_file(&self.flag) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("failed to remove Windows watchdog flag: {e}");
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
    let dir = std::env::temp_dir();
    let path = dir.join(format!("lavinia-{}.run", std::process::id()));
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    Ok(path)
}

#[allow(dead_code)]
fn cleanup(phone: &Ipv4Addr) -> Result<(), PlatformError> {
    let script = format!(
        "Remove-NetRoute -DestinationPrefix '{phone}/32' -Confirm:$false -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block IPv6' -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block Leaks' -ErrorAction SilentlyContinue; \
         Set-NetIPInterface -InterfaceAlias 'Lavinia' -InterfaceMetric 256 -ErrorAction SilentlyContinue"
    );
    privileged_powershell(&script)
}

pub fn apply_firewall(spec: &FirewallSpec) -> Result<Guard, PlatformError> {
    let flag = flag_path()?;
    let guard = Guard {
        flag: flag.clone(),
        phone: spec.phone,
    };

    let flag_str = flag.to_string_lossy().replace('\'', "''");
    let pid = std::process::id();
    let phone = spec.phone;

    // 1. Watchdog runs elevated in the background. It exits and cleans up when the flag file
    //    is removed or the parent process terminates.
    let watchdog_script = format!(
        "while (Test-Path -LiteralPath '{flag_str}') {{ \
            if (-not (Get-Process -Id {pid} -ErrorAction SilentlyContinue)) {{ break }}; \
            Start-Sleep -Seconds 1; \
        }}; \
        Remove-NetRoute -DestinationPrefix '{phone}/32' -Confirm:$false -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block IPv6' -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block DNS-53' -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block DoT-853' -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block LLMNR-5355' -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block mDNS-5353' -ErrorAction SilentlyContinue; \
        Remove-NetFirewallRule -DisplayName 'Lavinia Block Leaks' -ErrorAction SilentlyContinue; \
        Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient' -Name 'DisableSmartNameResolution' -Value 0 -Type DWord -ErrorAction SilentlyContinue; \
        Set-NetIPInterface -InterfaceAlias 'Lavinia' -InterfaceMetric 256 -ErrorAction SilentlyContinue; \
        Remove-Item -LiteralPath '{flag_str}' -Force -ErrorAction SilentlyContinue"
    );
    let watchdog_b64 = super::proc::encode_ps_base64(&watchdog_script);

    // 2. Setup script: finds correct physical uplink towards phone, installs /32 route,
    //    sets MTU to 1380, disables SMHNR, blocks external DNS leaks, and starts the detached background watchdog.
    let script = format!(
        "$uplink = Find-NetRoute -RemoteIPAddress '{phone}' -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if (-not $uplink) {{ \
             $uplink = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1; \
         }}; \
         if ($uplink) {{ \
             Remove-NetRoute -DestinationPrefix '{phone}/32' -Confirm:$false -ErrorAction SilentlyContinue; \
             if ($uplink.NextHop -and $uplink.NextHop -ne '0.0.0.0') {{ \
                 New-NetRoute -DestinationPrefix '{phone}/32' -InterfaceIndex $uplink.InterfaceIndex -NextHop $uplink.NextHop -PolicyStore ActiveStore -ErrorAction SilentlyContinue; \
             }} else {{ \
                 New-NetRoute -DestinationPrefix '{phone}/32' -InterfaceIndex $uplink.InterfaceIndex -PolicyStore ActiveStore -ErrorAction SilentlyContinue; \
             }} \
         }}; \
         Set-NetIPInterface -InterfaceAlias 'Lavinia' -NlMtu 1380 -ErrorAction SilentlyContinue; \
         $dnsPolicyPath = 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows NT\\DNSClient'; \
         if (-not (Test-Path $dnsPolicyPath)) {{ New-Item -Path $dnsPolicyPath -Force | Out-Null }}; \
         Set-ItemProperty -Path $dnsPolicyPath -Name 'DisableSmartNameResolution' -Value 1 -Type DWord -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block IPv6' -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'Lavinia Block IPv6' -Direction Outbound -Action Block -RemoteAddress '::/0' -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block DNS-53' -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'Lavinia Block DNS-53' -Direction Outbound -Action Block -Protocol UDP,TCP -RemotePort 53 -InterfaceAlias $uplink.InterfaceAlias -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block DoT-853' -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'Lavinia Block DoT-853' -Direction Outbound -Action Block -Protocol TCP -RemotePort 853 -InterfaceAlias $uplink.InterfaceAlias -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block LLMNR-5355' -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'Lavinia Block LLMNR-5355' -Direction Outbound -Action Block -Protocol UDP -RemotePort 5355 -InterfaceAlias $uplink.InterfaceAlias -ErrorAction SilentlyContinue; \
         Remove-NetFirewallRule -DisplayName 'Lavinia Block mDNS-5353' -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'Lavinia Block mDNS-5353' -Direction Outbound -Action Block -Protocol UDP -RemotePort 5353 -InterfaceAlias $uplink.InterfaceAlias -ErrorAction SilentlyContinue; \
         Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand','{watchdog_b64}'"
    );

    privileged_powershell(&script)?;
    Ok(guard)
}
