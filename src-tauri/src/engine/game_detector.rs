//! Process monitoring and auto game-mode detection for low-latency gaming.
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static GAME_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

const KNOWN_GAME_BINARIES: &[&str] = &[
    "aces",
    "aces.x86_64",
    "aces.exe",
    "hoi4",
    "hoi4.exe",
    "dowser",
    "dowser.exe",
    "cs2",
    "cs2.exe",
    "dota2",
    "steam",
    "Discord",
    "discord",
    "DiscordCanary",
    "discord-canary",
    "DiscordPTB",
    "discord-ptb",
    "vesktop",
    "webcord",
    "armcord",
    "valorant",
    "leagueoflegends",
];

/// Returns true if a known latency-sensitive game or voice app is running.
pub fn is_game_active() -> bool {
    GAME_MODE_ACTIVE.load(Ordering::Relaxed)
}

/// Spawns a background thread scanning /proc for game processes with debounce hysteresis
/// and adaptive power-saving sleep intervals.
pub fn spawn_game_detector() {
    std::thread::spawn(move || {
        let mut consecutive_misses: u32 = 0;
        let mut cached_pid: Option<u32> = None;
        const HYSTERESIS_MISS_THRESHOLD: u32 = 3;
        const ACTIVE_SLEEP: Duration = Duration::from_secs(3);
        const IDLE_SLEEP: Duration = Duration::from_secs(10);

        loop {
            let found_now = check_active_or_scan(&mut cached_pid);
            let current_active = GAME_MODE_ACTIVE.load(Ordering::Relaxed);

            if found_now {
                consecutive_misses = 0;
                if !current_active {
                    GAME_MODE_ACTIVE.store(true, Ordering::Relaxed);
                    eprintln!("[Lavinia Game Detector] Gaming process detected! Low-latency AQM active.");
                    crate::app::notifier::Notifier::notify(
                        "Lavinia Oyun Modu Aktif",
                        "Düşük gecikmeli CoDel AQM ve oyun önceliği devrede.",
                    );
                }
            } else if current_active {
                consecutive_misses = consecutive_misses.saturating_add(1);
                if consecutive_misses >= HYSTERESIS_MISS_THRESHOLD {
                    GAME_MODE_ACTIVE.store(false, Ordering::Relaxed);
                    cached_pid = None;
                    eprintln!("[Lavinia Game Detector] Gaming process closed. Default AQM restored.");
                    crate::app::notifier::Notifier::notify(
                        "Lavinia Standart Mod",
                        "Oyun kapatıldı, standart debloat profiline dönüldü.",
                    );
                }
            }

            // Adaptive sleep: when active, poll every 3s for quick shutdown response.
            // When completely idle, sleep for 10s to conserve CPU wakeups and battery.
            let sleep_dur = if current_active || found_now {
                ACTIVE_SLEEP
            } else {
                IDLE_SLEEP
            };
            std::thread::sleep(sleep_dur);
        }
    });
}

/// Fast path: check cached PID first. If still valid, returns true (zero /proc scan).
/// Otherwise falls back to a zero-alloc /proc inspection and caches any found game PID.
fn check_active_or_scan(cached_pid: &mut Option<u32>) -> bool {
    let mut comm_buf = [0u8; 64];

    if let Some(pid) = *cached_pid {
        if verify_pid_game(pid, &mut comm_buf) {
            return true;
        }
        *cached_pid = None;
    }

    if let Some(pid) = scan_processes(&mut comm_buf) {
        *cached_pid = Some(pid);
        true
    } else {
        false
    }
}

/// Checks whether a single PID is still running a target game binary.
fn verify_pid_game(pid: u32, comm_buf: &mut [u8; 64]) -> bool {
    let mut path_buf = [0u8; 48];
    let path = match format_pid_subpath(&mut path_buf, pid, "/comm") {
        Some(p) => p,
        None => return false,
    };

    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let Ok(n) = f.read(comm_buf) else {
        return false;
    };
    let comm = trim_ascii(&comm_buf[..n]);

    if matches_target(comm) {
        return true;
    }

    if is_runner(comm) {
        let mut cmd_buf = [0u8; 512];
        let path = match format_pid_subpath(&mut path_buf, pid, "/cmdline") {
            Some(p) => p,
            None => return false,
        };
        if let Ok(mut cf) = File::open(path) {
            if let Ok(cn) = cf.read(&mut cmd_buf) {
                return inspect_cmdline(&cmd_buf[..cn]);
            }
        }
    }

    false
}

/// Inspects /proc with zero heap allocations per process.
fn scan_processes(comm_buf: &mut [u8; 64]) -> Option<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return None;
    };

    let mut path_buf = [0u8; 48];
    let mut cmd_buf = [0u8; 512];

    for entry in entries.flatten() {
        let os_name = entry.file_name();
        let bytes = os_name.as_encoded_bytes();

        // Only parse numeric directory names (PIDs)
        if bytes.is_empty() || !bytes.iter().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let pid: u32 = match std::str::from_utf8(bytes).ok().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };

        let path = match format_pid_subpath(&mut path_buf, pid, "/comm") {
            Some(p) => p,
            None => continue,
        };

        let Ok(mut f) = File::open(path) else {
            continue;
        };
        let Ok(n) = f.read(comm_buf) else {
            continue;
        };
        let comm = trim_ascii(&comm_buf[..n]);

        if matches_target(comm) {
            return Some(pid);
        }

        if is_runner(comm) {
            let cmd_path = match format_pid_subpath(&mut path_buf, pid, "/cmdline") {
                Some(p) => p,
                None => continue,
            };
            if let Ok(mut cf) = File::open(cmd_path) {
                if let Ok(cn) = cf.read(&mut cmd_buf) {
                    if inspect_cmdline(&cmd_buf[..cn]) {
                        return Some(pid);
                    }
                }
            }
        }
    }
    None
}

#[inline]
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\n' || bytes[start] == b'\r' || bytes[start] == b'\t') {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\n' || bytes[end - 1] == b'\r' || bytes[end - 1] == b'\t' || bytes[end - 1] == 0) {
        end -= 1;
    }
    &bytes[start..end]
}

#[inline]
fn matches_target(comm: &[u8]) -> bool {
    KNOWN_GAME_BINARIES
        .iter()
        .any(|target| target.as_bytes().eq_ignore_ascii_case(comm))
}

#[inline]
fn is_runner(comm: &[u8]) -> bool {
    comm.starts_with(b"wine")
        || comm.starts_with(b"pressure-vessel")
        || comm == b"wineserver"
}

fn inspect_cmdline(cmdline: &[u8]) -> bool {
    for arg in cmdline.split(|&b| b == 0) {
        if arg.is_empty() {
            continue;
        }
        let filename = arg
            .rsplit(|&b| b == b'/' || b == b'\\')
            .next()
            .unwrap_or(arg);

        if matches_target(filename) {
            return true;
        }
    }
    false
}

/// Formats `/proc/<pid><suffix>` into a stack buffer returning a `&str` without heap allocation.
fn format_pid_subpath<'a>(buf: &'a mut [u8; 48], pid: u32, suffix: &str) -> Option<&'a str> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(&mut buf[..]);
    write!(cursor, "/proc/{}{}", pid, suffix).ok()?;
    let len = cursor.position() as usize;
    std::str::from_utf8(&buf[..len]).ok()
}
