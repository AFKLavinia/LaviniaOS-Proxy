//! Tray-resident Tauri shell. Public entry point: [`run`].
mod glyph;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, RunEvent, WindowEvent};

use crate::tunnel::{self, TunnelState};

pub(crate) static EXITING: AtomicBool = AtomicBool::new(false);

pub fn run() {
    tauri::Builder::default()
        .manage(TunnelState::new())
        .invoke_handler(tauri::generate_handler![
            window::ui_ready,
            tunnel::commands::get_status,
            tunnel::commands::start_tunnel,
            tunnel::commands::stop_tunnel,
            tunnel::commands::grant_udp_privilege,
            tunnel::commands::get_autostart,
            tunnel::commands::set_autostart,
            tunnel::commands::get_adblock,
            tunnel::commands::set_adblock
        ])
        .setup(|app| {
            tray::install(app.handle())?;
            match app.path().app_config_dir() {
                Ok(dir) => tunnel::restore_last_target(&app.state::<TunnelState>(), &dir),
                Err(e) => eprintln!("config dir unavailable: {e}"),
            }
            tunnel::spawn_telemetry(app.handle());
            spawn_tray_sync(app.handle());
            window::spawn_fallback_show(app.handle());
            Ok(())
        })
        // Background app: closing the window hides it; exit only via tray "Quit".
        .on_window_event(|win, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if EXITING.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                if let Err(e) = win.hide() {
                    eprintln!("failed to hide window: {e}");
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("tauri runtime failed")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<TunnelState>().stop();
            }
        });
}

/// Keeps the tray icon in step with tunnel phase / uplink changes.
fn spawn_tray_sync(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<TunnelState>();
        let mut last = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let status = state.status();
            let key = (
                status.phase,
                status.target.host,
                status.target.port,
                status.link.as_ref().map(|l| (l.kind, l.name.clone(), l.hardware.clone())),
            );
            if last.as_ref() != Some(&key) {
                tray::refresh(&app, &status);
                last = Some(key);
            }
        }
    });
}
