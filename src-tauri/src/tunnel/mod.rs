//! Tunnel control surface: IPC commands, state and telemetry. Entry: [`TunnelState`], [`spawn_telemetry`].
pub mod commands;
mod dto;
mod state;
mod store;

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

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
        loop {
            thread::sleep(TELEMETRY_INTERVAL);
            let rx = c.rx_bytes.load(Ordering::Relaxed);
            let tx = c.tx_bytes.load(Ordering::Relaxed);
            let payload = Telemetry {
                phase: state.status().phase,
                rx_bps: rx.saturating_sub(last_rx),
                tx_bps: tx.saturating_sub(last_tx),
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
