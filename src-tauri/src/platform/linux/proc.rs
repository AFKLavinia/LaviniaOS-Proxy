use std::process::{Command, Stdio};

use crate::platform::PlatformError;

/// Exit codes pkexec uses for "dismissed" / "not authorized".
const PKEXEC_DISMISSED: i32 = 126;
const PKEXEC_DENIED: i32 = 127;
pub const HELPER_PATH: &str = "/usr/lib/lavinia/lavinia-net-helper";

pub fn helper_available() -> bool {
    std::path::Path::new(HELPER_PATH).exists()
}

fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

fn sudo_cached() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Executes the privileged lavinia-net-helper with strictly bounded arguments.
pub fn privileged_helper(args: &[&str]) -> Result<(), PlatformError> {
    let cmd = if is_root() {
        let mut c = Command::new(HELPER_PATH);
        c.args(args);
        c
    } else if sudo_cached() {
        let mut c = Command::new("sudo");
        c.arg("-n").arg(HELPER_PATH).args(args);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg(HELPER_PATH).args(args);
        c
    };

    execute_and_verify(cmd)
}

/// Fallback runner for arbitrary development shell scripts when helper is not installed.
pub fn privileged_sh(script: &str) -> Result<(), PlatformError> {
    let cmd = if is_root() {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(script);
        c
    } else if sudo_cached() {
        let mut c = Command::new("sudo");
        c.args(["-n", "/bin/sh", "-c"]).arg(script);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg("/bin/sh").arg("-c").arg(script);
        c
    };

    execute_and_verify(cmd)
}

fn execute_and_verify(mut cmd: Command) -> Result<(), PlatformError> {
    let out = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| PlatformError::Command(format!("failed to launch privileged helper: {e}")))?;

    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let msg = if !stderr.trim().is_empty() {
        stderr.trim()
    } else if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        "command exited with non-zero status"
    };

    match out.status.code() {
        Some(PKEXEC_DISMISSED) | Some(PKEXEC_DENIED) if !is_root() => {
            if msg.contains("not found") || msg.contains("No such file") {
                Err(PlatformError::Command(format!("privileged operation failed: {msg}")))
            } else {
                Err(PlatformError::Command("authorization was cancelled or denied".into()))
            }
        }
        Some(code) => Err(PlatformError::Command(format!("privileged operation failed (exit code {code}): {msg}"))),
        None => Err(PlatformError::Command(format!("privileged operation terminated by signal: {msg}"))),
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
