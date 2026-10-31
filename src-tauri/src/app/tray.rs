use std::sync::atomic::Ordering;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use super::{glyph, window, EXITING};
use crate::platform::LinkKind;
use crate::tunnel::{Phase, Status, TunnelState};

const ID_SHOW: &str = "show";
const ID_QUIT: &str = "quit";
const TRAY_ID: &str = "main";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, ID_SHOW, "Show", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Lavinia Client")
        .on_menu_event(|app, event| match event.id.as_ref() {
            ID_SHOW => window::show_main(app),
            ID_QUIT => {
                app.state::<TunnelState>().stop();
                EXITING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window(window::MAIN_LABEL) {
                    if win.is_visible().unwrap_or(false) && win.is_focused().unwrap_or(false) {
                        let _ = win.hide();
                        return;
                    }
                }
                window::show_main(app);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Reflects tunnel phase and uplink medium (Wi-Fi / USB) in the tray icon and tooltip.
pub fn refresh(app: &AppHandle, status: &Status) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let color = match status.phase {
        Phase::Running => glyph::GREEN,
        Phase::Degraded => glyph::AMBER,
        Phase::Starting => {
            set(&tray, default_icon(app), "Lavinia Client - starting...");
            return;
        }
        Phase::Stopped => {
            set(&tray, default_icon(app), "Lavinia Client - idle");
            return;
        }
    };
    let (icon, medium) = match status.link.as_ref().map(|l| l.kind) {
        Some(LinkKind::Wifi) => (Some(glyph::wifi(color)), "Wi-Fi"),
        Some(LinkKind::Usb) => (Some(glyph::usb(color)), "USB"),
        Some(LinkKind::Ethernet) => (None, "Ethernet"),
        Some(LinkKind::Bluetooth) => (None, "Bluetooth"),
        Some(LinkKind::Cellular) => (None, "Cellular"),
        _ => (None, "link"),
    };
    let detail = status
        .link
        .as_ref()
        .map(|l| l.hardware.clone().unwrap_or_else(|| l.name.clone()))
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({s})"))
        .unwrap_or_default();
    let state = if status.phase == Phase::Degraded { "proxy unreachable" } else { "active" };
    let tip = format!(
        "Lavinia Client - {state} via {medium}{detail} {}:{}",
        status.target.host, status.target.port
    );
    set(&tray, icon.or_else(|| default_icon(app)), &tip);
}

fn set(tray: &tauri::tray::TrayIcon, icon: Option<tauri::image::Image<'static>>, tip: &str) {
    if let Some(icon) = icon {
        if let Err(e) = tray.set_icon(Some(icon)) {
            eprintln!("tray icon update failed: {e}");
        }
    }
    if let Err(e) = tray.set_tooltip(Some(tip)) {
        eprintln!("tray tooltip update failed: {e}");
    }
}

fn default_icon(app: &AppHandle) -> Option<tauri::image::Image<'static>> {
    app.default_window_icon()
        .map(|i| tauri::image::Image::new_owned(i.rgba().to_vec(), i.width(), i.height()))
}
