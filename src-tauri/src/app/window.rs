use tauri::{AppHandle, Manager};

pub const MAIN_LABEL: &str = "main";

pub fn show_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        eprintln!("main window not found");
        return;
    };
    if let Err(e) = win.show().and_then(|_| win.set_focus()) {
        eprintln!("failed to show main window: {e}");
    }
}

/// Invoked by the UI after first paint; the window is created hidden to avoid the white flash.
#[tauri::command]
pub fn ui_ready(app: AppHandle) {
    show_main(&app);
}
