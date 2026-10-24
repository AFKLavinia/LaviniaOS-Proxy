//! Persists the last used proxy endpoint (plain "ip port"; never credentials).
use std::fs;
use std::io;
use std::path::Path;

use super::dto::{Target, TargetDto};

const FILE: &str = "last_target.txt";

/// Reads and re-validates the stored endpoint; unreadable or invalid content is ignored.
pub fn load(dir: &Path) -> Option<Target> {
    let text = fs::read_to_string(dir.join(FILE)).ok()?;
    let mut it = text.split_whitespace();
    let (host, port) = (it.next()?, it.next()?.parse().ok()?);
    Target::try_from(TargetDto { host: host.to_string(), port, username: None, password: None, dns: None }).ok()
}

/// Atomic write (temp file + rename) so a crash never leaves a truncated file.
pub fn save(dir: &Path, target: &Target) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    fs::write(&tmp, format!("{} {}\n", target.host, target.port))?;
    fs::rename(tmp, dir.join(FILE))
}
