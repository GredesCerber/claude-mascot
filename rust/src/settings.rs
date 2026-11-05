//! User preferences only. Loading never executes skin files or starts external programs.
use crate::engine::Skin;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub skin: Skin,
    pub scale: f64,
    pub hide_fullscreen: bool,
    pub show_bubbles: bool,
    pub paused: bool,
    pub auto_updates: bool,
    pub update_repo: String,
    /// Optional directory with tagilla_frames.png + tagilla_frames.json. Data only.
    pub custom_atlas: Option<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            skin: Skin::Claude,
            scale: 1.0,
            hide_fullscreen: true,
            show_bubbles: true,
            paused: false,
            auto_updates: true,
            update_repo: option_env!("MASCOT_UPDATE_REPO")
                .unwrap_or("kerimovernes/claude-mascot")
                .into(),
            custom_atlas: None,
        }
    }
}
impl Settings {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Cannot read settings: {e}")),
        };
        use std::io::Read;
        let mut s = String::new();
        file.take(65537)
            .read_to_string(&mut s)
            .map_err(|e| format!("Cannot read settings: {e}"))?;
        if s.len() > 65536 {
            return Err("Settings file exceeds 64 KiB".into());
        }
        let value: Self = serde_json::from_str(&s)
            .map_err(|e| format!("Invalid settings in {}: {e}", path.display()))?;
        if value.version != 1 {
            return Err(format!("Unsupported settings version {}", value.version));
        }
        if ![0.75, 1.0, 1.5].contains(&value.scale) {
            return Err("Scale must be 0.75, 1.0 or 1.5".into());
        }
        Ok(value)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        #[cfg(windows)]
        {
            use windows_sys::Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
            };
            let from = wide(temp.as_os_str());
            let to = wide(path.as_os_str());
            // SAFETY: nul-terminated paths live through the call; no borrowed handles.
            if unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        #[cfg(not(windows))]
        std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
pub fn config_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("claude-mascot-rust/settings.json")
}
#[cfg(windows)]
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    use windows_sys::Win32::System::Registry::*;
    let key = wide(std::ffi::OsStr::new(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    ));
    let name = wide(std::ffi::OsStr::new("ClaudeMascotRust"));
    let mut size = 0;
    // SAFETY: query only; all pointers refer to live buffers, data buffer is null by design.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        ) == 0
    }
}
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::*;
    let sub = wide(std::ffi::OsStr::new(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    ));
    let name = wide(std::ffi::OsStr::new("ClaudeMascotRust"));
    let mut key = std::ptr::null_mut();
    // SAFETY: opens only this user's Run key; the handle is closed on all later paths.
    let code = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if code != 0 {
        return Err(format!("Cannot open startup settings ({code})"));
    }
    let result = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string());
        match exe {
            Ok(exe) => {
                let value = wide(std::ffi::OsStr::new(&format!("\"{}\"", exe.display())));
                unsafe {
                    RegSetValueExW(
                        key,
                        name.as_ptr(),
                        0,
                        REG_SZ,
                        value.as_ptr() as *const u8,
                        (value.len() * 2) as u32,
                    )
                }
            }
            Err(e) => {
                unsafe { RegCloseKey(key) };
                return Err(e);
            }
        }
    } else {
        unsafe { RegDeleteValueW(key, name.as_ptr()) }
    };
    unsafe { RegCloseKey(key) };
    if result != 0 && !(result == 2 && !enabled) {
        return Err(format!("Cannot update startup entry ({result})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backwards_compatible_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.scale, 1.0);
        assert!(s.hide_fullscreen);
        assert!(!s.paused);
    }
    #[test]
    fn round_trip_and_replace() {
        let p =
            std::env::temp_dir().join(format!("mascot-settings-test-{}.json", std::process::id()));
        let mut s = Settings::default();
        s.save(&p).unwrap();
        s.skin = Skin::Tagilla;
        s.scale = 1.5;
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p).unwrap().skin, Skin::Tagilla);
        std::fs::remove_file(p).unwrap();
    }
}
