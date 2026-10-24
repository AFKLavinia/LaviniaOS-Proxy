use std::process::{Command, Stdio};

use crate::platform::PlatformError;

/// Exit codes pkexec uses for "dismissed" / "not authorized".
const PKEXEC_DISMISSED: i32 = 126;
const PKEXEC_DENIED: i32 = 127;

fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Runs a shell script with root privileges (directly if already root, otherwise via polkit).
pub fn privileged_sh(script: &str) -> Result<(), PlatformError> {
    let mut cmd = if is_root() {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(script);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg("/bin/sh").arg("-c").arg(script);
        c
    };
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| PlatformError::Command(format!("failed to launch privileged helper: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    match out.status.code() {
        Some(PKEXEC_DISMISSED) | Some(PKEXEC_DENIED) if !is_root() => {
            Err(PlatformError::Command("authorization was cancelled or denied".into()))
        }
        _ => Err(PlatformError::Command(format!(
            "firewall setup failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
    }
}

/// Runs an unprivileged command and returns stdout.
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
