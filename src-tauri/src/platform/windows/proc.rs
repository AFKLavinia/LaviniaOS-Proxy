use std::process::{Command, Stdio};

use crate::platform::PlatformError;

/// Runs PowerShell or cmd command synchronously and returns stdout.
pub fn run(program: &str, args: &[&str]) -> Result<String, PlatformError> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| PlatformError::Command(format!("`{program}` unavailable: {e}")))?;
    if !out.status.success() {
        return Err(PlatformError::Command(format!(
            "`{program}` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Executes a PowerShell script.
pub fn run_powershell(script: &str) -> Result<String, PlatformError> {
    run("powershell.exe", &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
}

/// Runs a script with administrator elevation via PowerShell Start-Process -Verb RunAs.
pub fn privileged_powershell(script: &str) -> Result<(), PlatformError> {
    // Escape single quotes for PowerShell literal
    let escaped = script.replace('\'', "''");
    let launcher = format!(
        "Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"& {{ {} }}\"'",
        escaped
    );
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &launcher])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| PlatformError::Command(format!("failed to request elevation on Windows: {e}")))?;

    if !out.status.success() {
        return Err(PlatformError::Command("UAC elevation cancelled or failed".into()));
    }
    Ok(())
}
