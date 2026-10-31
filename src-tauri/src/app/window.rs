use std::time::Duration;
use tauri::{AppHandle, Manager};

pub const MAIN_LABEL: &str = "main";

pub fn show_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        eprintln!("main window not found");
        return;
    };
    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
    }
    if let Err(e) = win.show().and_then(|_| win.set_focus()) {
        eprintln!("failed to show main window: {e}");
    }
}

/// Fallback safety timer: shows the window if the frontend has not called `ui_ready` within 4 seconds.
pub fn spawn_fallback_show(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        if let Some(win) = app.get_webview_window(MAIN_LABEL) {
            if !win.is_visible().unwrap_or(false) {
                show_main(&app);
            }
        }
    });
}

/// Invoked by the UI after first paint; the window is created hidden to avoid the white flash.
#[tauri::command]
pub fn ui_ready(app: AppHandle) {
    show_main(&app);
}
