//! Windows registry Run key autostart management.
use super::proc::run_powershell;
use crate::platform::PlatformError;

const APP_NAME: &str = "LaviniaClient";
const REG_KEY: &str = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn is_enabled() -> bool {
    let script = format!(
        "$val = Get-ItemProperty -Path '{REG_KEY}' -Name '{APP_NAME}' -ErrorAction SilentlyContinue; \
        if ($val -and $val.{APP_NAME}) {{ Write-Output 'true' }} else {{ Write-Output 'false' }}"
    );
    run_powershell(&script)
        .map(|out| out.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> Result<(), PlatformError> {
    if enabled {
        let exe = std::env::current_exe()?;
        let exe_str = exe.to_str().ok_or_else(|| {
            PlatformError::Command("executable path contains invalid characters".into())
        })?;
        let script = format!(
            "Set-ItemProperty -Path '{REG_KEY}' -Name '{APP_NAME}' -Value '\"{exe_str}\"'"
        );
        run_powershell(&script)?;
    } else {
        let script = format!(
            "Remove-ItemProperty -Path '{REG_KEY}' -Name '{APP_NAME}' -ErrorAction SilentlyContinue"
        );
        run_powershell(&script)?;
    }
    Ok(())
}
