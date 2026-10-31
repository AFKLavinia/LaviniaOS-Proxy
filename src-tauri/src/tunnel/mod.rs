//! Tunnel control surface: IPC commands, state and telemetry. Entry: [`TunnelState`], [`spawn_telemetry`].
pub mod commands;
mod dto;
mod state;
mod store;

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

pub use dto::{Phase, Status};
pub use state::TunnelState;

use std::path::Path;

/// Loads the last used endpoint from `config_dir` into the state, if any.
pub fn restore_last_target(state: &TunnelState, config_dir: &Path) {
    if let Some(t) = store::load(config_dir) {
        state.preset_target(t);
    }
}

use dto::Telemetry;

const TELEMETRY_EVENT: &str = "telemetry";
const TELEMETRY_INTERVAL: Duration = Duration::from_secs(1);

pub fn spawn_telemetry(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let state = app.state::<TunnelState>();
        let c = &state.counters;
        let (mut last_rx, mut last_tx) = (0u64, 0u64);
        let mut last_tick = Instant::now();
        loop {
            thread::sleep(TELEMETRY_INTERVAL);
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick).as_secs_f64();
            last_tick = now;

            let rx = c.rx_bytes.load(Ordering::Relaxed);
            let tx = c.tx_bytes.load(Ordering::Relaxed);

            if rx < last_rx {
                last_rx = 0;
            }
            if tx < last_tx {
                last_tx = 0;
            }

            let rx_delta = rx.saturating_sub(last_rx);
            let tx_delta = tx.saturating_sub(last_tx);

            let (rx_bps, tx_bps) = if elapsed > 0.0 {
                (
                    ((rx_delta as f64) / elapsed).round() as u64,
                    ((tx_delta as f64) / elapsed).round() as u64,
                )
            } else {
                (rx_delta, tx_delta)
            };

            let payload = Telemetry {
                phase: state.status().phase,
                rx_bps,
                tx_bps,
                rx_packets: c.rx_packets.load(Ordering::Relaxed),
                tx_packets: c.tx_packets.load(Ordering::Relaxed),
                dropped: c.dropped.load(Ordering::Relaxed),
            };
            (last_rx, last_tx) = (rx, tx);
            if let Err(e) = app.emit(TELEMETRY_EVENT, payload) {
                eprintln!("telemetry emit failed: {e}");
            }
        }
    });
}
