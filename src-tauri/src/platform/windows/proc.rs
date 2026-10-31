use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::platform::PlatformError;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Runs PowerShell or cmd command synchronously with hidden console window and returns stdout.
pub fn run(program: &str, args: &[&str]) -> Result<String, PlatformError> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
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

/// Executes a PowerShell script with UTF-8 console output encoding.
pub fn run_powershell(script: &str) -> Result<String, PlatformError> {
    let full_script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {script}");
    run(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &full_script,
        ],
    )
}

/// Convenience alias for run_powershell.
pub fn powershell(script: &str) -> Result<String, PlatformError> {
    run_powershell(script)
}

/// Encodes a PowerShell script into a UTF-16LE Base64 string for -EncodedCommand.
pub(crate) fn encode_ps_base64(script: &str) -> String {
    let mut utf16_bytes = Vec::with_capacity(script.len() * 2);
    for u in script.encode_utf16() {
        utf16_bytes.extend_from_slice(&u.to_le_bytes());
    }
    base64_encode(&utf16_bytes)
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut chunks = data.chunks_exact(3);
    for chunk in &mut chunks {
        let b0 = chunk[0] as usize;
        let b1 = chunk[1] as usize;
        let b2 = chunk[2] as usize;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) & 0x3F] as char);
        out.push(TABLE[(n >> 12) & 0x3F] as char);
        out.push(TABLE[(n >> 6) & 0x3F] as char);
        out.push(TABLE[n & 0x3F] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as usize) << 16;
            out.push(TABLE[(n >> 18) & 0x3F] as char);
            out.push(TABLE[(n >> 12) & 0x3F] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as usize) << 16) | ((rem[1] as usize) << 8);
            out.push(TABLE[(n >> 18) & 0x3F] as char);
            out.push(TABLE[(n >> 12) & 0x3F] as char);
            out.push(TABLE[(n >> 6) & 0x3F] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

/// Runs a script with administrator elevation via PowerShell Start-Process -Verb RunAs.
pub fn privileged_powershell(script: &str) -> Result<(), PlatformError> {
    let inner_script = format!("$ErrorActionPreference = 'Stop'; {script}");
    let b64 = encode_ps_base64(&inner_script);
    let launcher = format!(
        "$p = Start-Process -FilePath 'powershell.exe' -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand','{b64}'; if ($null -eq $p) {{ exit 1223 }}; exit $p.ExitCode"
    );
    let out = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &launcher,
        ])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| PlatformError::Command(format!("failed to request elevation on Windows: {e}")))?;

    if !out.status.success() {
        let code = out.status.code();
        if code == Some(1223) {
            return Err(PlatformError::Command("UAC elevation cancelled or failed".into()));
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(PlatformError::Command(if stderr.is_empty() {
            format!("privileged command failed (exit code: {:?})", code)
        } else {
            format!("privileged command failed: {stderr}")
        }));
    }
    Ok(())
}
