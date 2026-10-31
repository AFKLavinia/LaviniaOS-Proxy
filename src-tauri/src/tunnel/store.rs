//! Persists the last used proxy endpoint (plain "ip port"; never credentials).
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::dto::{Target, TargetDto};

const FILE: &str = "last_target.txt";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Reads and re-validates the stored endpoint; unreadable or invalid content is ignored.
pub fn load(dir: &Path) -> Option<Target> {
    let f = fs::File::open(dir.join(FILE)).ok()?;
    let mut text = String::new();
    f.take(256).read_to_string(&mut text).ok()?;
    let mut it = text.split_whitespace();
    let host = it.next()?;
    let port = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Target::try_from(TargetDto {
        host: host.to_string(),
        port,
        username: None,
        password: None,
        dns: None,
    })
    .ok()
}

/// Persists the last used proxy endpoint with atomic write semantics and Windows file locking fallback.
pub fn save(dir: &Path, target: &Target) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let tmp = dir.join(format!("{FILE}.{pid}_{counter}.tmp"));
    let target_path = dir.join(FILE);
    let payload = format!("{} {}\n", target.host, target.port);

    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(payload.as_bytes())?;
        file.sync_all()?;
    }

    // Attempt rename with brief retry backoff for Windows sharing locks
    let mut rename_err = None;
    for attempt in 0..5 {
        match fs::rename(&tmp, &target_path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                rename_err = Some(e);
                if attempt < 4 {
                    std::thread::sleep(std::time::Duration::from_millis(15 * (attempt as u64 + 1)));
                }
            }
        }
    }

    // Windows fallback: if rename failed due to file locking, write directly to the target file
    let direct_result = (|| {
        let mut file = fs::File::create(&target_path)?;
        file.write_all(payload.as_bytes())?;
        file.sync_all()
    })();

    // Always attempt to clean up temporary file
    let _ = fs::remove_file(&tmp);

    direct_result.map_err(|e| rename_err.unwrap_or(e))
}
