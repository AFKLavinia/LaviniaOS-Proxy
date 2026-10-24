//! XDG autostart entry (starts the tray app hidden at login).
use std::fs;
use std::path::PathBuf;

use crate::platform::PlatformError;

const FILE: &str = "lavinia-client.desktop";

fn entry_path() -> Result<PathBuf, PlatformError> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or_else(|| PlatformError::Command("no config directory available".into()))?;
    Ok(base.join("autostart").join(FILE))
}

pub fn is_enabled() -> bool {
    entry_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> Result<(), PlatformError> {
    let path = entry_path()?;
    if !enabled {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe()?;
    let exe = exe.to_str().filter(|s| !s.contains(['\n', '"', '%'])).ok_or_else(|| {
        PlatformError::Command("executable path cannot be used in a desktop entry".into())
    })?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(
        &path,
        format!("[Desktop Entry]\nType=Application\nName=Lavinia Client\nExec=\"{exe}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"),
    )?;
    Ok(())
}
