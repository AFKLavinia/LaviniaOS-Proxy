//! Active Queue Management and Adaptive Delay Tracking (RFC 8289 CoDel Algorithm).
//! Implements strict Single Responsibility Principle (SRP) and zero-alloc state tracking.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START_INSTANT: OnceLock<Instant> = OnceLock::new();

#[inline]
pub fn monotonic_micros(now: Instant) -> u64 {
    let start = START_INSTANT.get_or_init(Instant::now);
    now.saturating_duration_since(*start).as_micros() as u64
}

/// RFC 8289 CoDel State Machine for queue buffer debloating.
struct CodelState {
    dropping: bool,
    first_above_time: u64,
    drop_next: u64,
    count: u32,
    last_drop_micros: u64,
}

static CODEL_STATE: Mutex<CodelState> = Mutex::new(CodelState {
    dropping: false,
    first_above_time: 0,
    drop_next: 0,
    count: 0,
    last_drop_micros: 0,
});

/// Minimum sojourn time tracker over sliding 100ms intervals.
struct MinSojournTracker {
    epoch_start_micros: AtomicU64,
    min_sojourn_micros: AtomicU32,
}

static MIN_TRACKER: MinSojournTracker = MinSojournTracker {
    epoch_start_micros: AtomicU64::new(0),
    min_sojourn_micros: AtomicU32::new(u32::MAX),
};

pub struct AdaptiveQueueController;

impl AdaptiveQueueController {
    /// Evaluates if a queued datagram should be dropped according to RFC 8289 CoDel.
    /// Protects against cellular tethering jitter bursts (HARQ/DRX) and bufferbloat.
    #[inline]
    pub fn should_drop(is_realtime: bool, enqueued_at: Instant, now: Instant) -> bool {
        if !is_realtime {
            return false;
        }

        let sojourn = now.saturating_duration_since(enqueued_at);
        let sojourn_micros = sojourn.as_micros() as u32;
        let now_micros = monotonic_micros(now);

        let in_game = crate::engine::game_detector::is_game_active();

        // RFC 8289 parameters:
        // Standard LAN target is 5ms (5,000µs). On LTE/5G cellular tethering, jitter bursts
        // frequently hit 20-40ms, so target is adaptive: 15ms in game mode, 10ms otherwise.
        let target_micros: u32 = if in_game { 15_000 } else { 10_000 };
        let interval_micros: u64 = 100_000; // 100ms sliding window

        // Update sliding window minimum sojourn time
        let epoch_start = MIN_TRACKER.epoch_start_micros.load(Ordering::Relaxed);
        if now_micros.saturating_sub(epoch_start) > interval_micros {
            MIN_TRACKER.epoch_start_micros.store(now_micros, Ordering::Relaxed);
            MIN_TRACKER.min_sojourn_micros.store(sojourn_micros, Ordering::Relaxed);
        } else {
            let mut cur_min = MIN_TRACKER.min_sojourn_micros.load(Ordering::Relaxed);
            while sojourn_micros < cur_min {
                match MIN_TRACKER.min_sojourn_micros.compare_exchange_weak(
                    cur_min,
                    sojourn_micros,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => cur_min = actual,
                }
            }
        }

        let min_sojourn = MIN_TRACKER.min_sojourn_micros.load(Ordering::Relaxed);

        // CoDel control law state transition
        let mut state = CODEL_STATE.lock().unwrap_or_else(|p| p.into_inner());

        if min_sojourn < target_micros {
            // Queue sojourn is healthy; exit dropping state
            state.dropping = false;
            state.first_above_time = 0;
            return false;
        }

        // Sojourn is exceeding target
        if !state.dropping {
            if state.first_above_time == 0 {
                // First packet that breached target: record time
                state.first_above_time = now_micros;
            } else if now_micros.saturating_sub(state.first_above_time) >= interval_micros {
                // Persistent queue: minimum sojourn remained above target for entire interval
                state.dropping = true;
                state.count = 1;
                state.drop_next = Self::control_law(now_micros, interval_micros, 1);
                state.last_drop_micros = now_micros;
                return true;
            }
        } else {
            // Currently in dropping state: drop next packet when scheduled time arrives
            if now_micros >= state.drop_next {
                state.count = state.count.saturating_add(1);
                state.drop_next = Self::control_law(state.drop_next, interval_micros, state.count);
                state.last_drop_micros = now_micros;
                return true;
            }
        }

        false
    }

    /// RFC 8289 Control Law: next_drop = t + interval / sqrt(count)
    #[inline]
    fn control_law(base_time: u64, interval_micros: u64, count: u32) -> u64 {
        let sqrt_c = (count as f64).sqrt().max(1.0);
        let delta = (interval_micros as f64 / sqrt_c) as u64;
        base_time.saturating_add(delta.max(2_000)) // Min 2ms spacing to prevent catastrophic drop storms
    }
}

/// Timestamped datagram with traffic classification metadata for zero-sleep queue management.
pub struct TimestampedDatagram {
    pub payload: Vec<u8>,
    pub enqueued_at: Instant,
    pub is_realtime: bool,
    pub is_control: bool,
}

impl TimestampedDatagram {
    #[inline]
    pub fn new(payload: Vec<u8>, is_realtime: bool) -> Self {
        Self {
            payload,
            enqueued_at: Instant::now(),
            is_realtime,
            is_control: false,
        }
    }

    #[inline]
    pub fn new_control(payload: Vec<u8>) -> Self {
        Self {
            payload,
            enqueued_at: Instant::now(),
            is_realtime: true,
            is_control: true,
        }
    }

    #[inline]
    pub fn is_stale(&self, now: Instant) -> bool {
        if self.is_control {
            return false;
        }
        AdaptiveQueueController::should_drop(self.is_realtime, self.enqueued_at, now)
    }
}
