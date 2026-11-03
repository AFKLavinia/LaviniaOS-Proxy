//! Desktop notification dispatcher using freedesktop D-Bus / notify-send.
//! Implements strict rate limiting and synchronous replacement to prevent desktop spam.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Notifier;

static LAST_NOTIFIED_SECS: AtomicU64 = AtomicU64::new(0);
const NOTIFY_COOLDOWN_SECS: u64 = 4; // Minimum 4 seconds between notification dispatches

impl Notifier {
    /// Dispatches a low-overhead system notification with synchronous update and rate-limiting.
    pub fn notify(summary: &str, body: &str) {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let last = LAST_NOTIFIED_SECS.load(Ordering::Relaxed);
        if now_secs.saturating_sub(last) < NOTIFY_COOLDOWN_SECS {
            return;
        }
        LAST_NOTIFIED_SECS.store(now_secs, Ordering::Relaxed);

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("notify-send")
                .args([
                    "-a",
                    "LaviniaOS",
                    "-i",
                    "network-vpn",
                    "-h",
                    "string:x-canonical-private-synchronous:lavinia-mode",
                    "-t",
                    "3500",
                    summary,
                    body,
                ])
                .spawn();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (summary, body);
        }
    }
}

