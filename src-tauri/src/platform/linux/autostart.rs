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
    let Ok(path) = entry_path() else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return false;
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "Hidden=true"
            || trimmed == "X-GNOME-Autostart-enabled=false"
            || trimmed.eq_ignore_ascii_case("hidden=true")
            || trimmed.eq_ignore_ascii_case("x-gnome-autostart-enabled=false")
        {
            return false;
        }
    }
    true
}

pub fn set_enabled(enabled: bool) -> Result<(), PlatformError> {
    let path = entry_path()?;
    if !enabled {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let exe = std::env::var_os("APPIMAGE")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)?;
    let exe = exe.to_str().filter(|s| !s.contains(['\n', '"', '%'])).ok_or_else(|| {
        PlatformError::Command("executable path cannot be used in a desktop entry".into())
    })?;
    let escaped_exe = exe.replace('\\', "\\\\").replace('$', "\\$");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::symlink_metadata(&path).is_ok() {
        let _ = fs::remove_file(&path);
    }
    fs::write(
        &path,
        format!("[Desktop Entry]\nType=Application\nName=Lavinia Client\nExec=\"{escaped_exe}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"),
    )?;
    Ok(())
}
