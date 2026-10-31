use tauri::{AppHandle, Manager, State};

use super::dto::{StartOptions, Status, TargetDto, TunnelError};
use super::state::TunnelState;

#[tauri::command]
pub fn get_status(state: State<'_, TunnelState>) -> Status {
    state.status()
}

#[tauri::command]
pub async fn start_tunnel(
    app: AppHandle,
    state: State<'_, TunnelState>,
    target: TargetDto,
) -> Result<Status, TunnelError> {
    let opts = StartOptions::try_from(target)?;
    let target = opts.target;
    let config_dir_res = app.path().app_config_dir().map_err(|e| e.to_string());
    let state = state.inner().clone();
    let status = tauri::async_runtime::spawn_blocking(move || {
        let status = state.start(opts)?;
        match config_dir_res {
            Ok(dir) => {
                if let Err(e) = super::store::save(&dir, &target) {
                    eprintln!("failed to persist last target: {e}");
                }
            }
            Err(e) => eprintln!("config dir unavailable: {e}"),
        }
        Ok::<Status, TunnelError>(status)
    })
    .await
    .map_err(|_| TunnelError::Engine("start worker failed".into()))??;
    Ok(status)
}

#[tauri::command]
pub async fn stop_tunnel(state: State<'_, TunnelState>) -> Result<Status, TunnelError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.stop())
        .await
        .map_err(|_| TunnelError::Engine("stop worker failed".into()))
}

/// Grants the UDP capability via the system authorization prompt, then restarts the app to apply it.
#[tauri::command]
pub async fn grant_udp_privilege(
    app: AppHandle,
    state: State<'_, TunnelState>,
) -> Result<(), TunnelError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::platform::grant_net_admin().map_err(|e| TunnelError::Engine(e.to_string()))?;
        state.stop();
        Ok::<(), TunnelError>(())
    })
    .await
    .map_err(|_| TunnelError::Engine("privilege worker failed".into()))??;
    app.restart()
}

#[tauri::command]
pub async fn get_autostart() -> bool {
    tauri::async_runtime::spawn_blocking(crate::platform::autostart_enabled)
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<bool, TunnelError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::platform::set_autostart(enabled).map_err(|e| TunnelError::Engine(e.to_string()))?;
        Ok(crate::platform::autostart_enabled())
    })
    .await
    .map_err(|_| TunnelError::Engine("autostart worker failed".into()))?
}
