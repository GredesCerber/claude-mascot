//! Best-effort Windows media-session bridge. WinRT calls run away from the UI
//! thread; `World::video_window` is set only for a playing session typed as video.
use crate::World;
use std::collections::HashMap;
use std::sync::{
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Media::{
    Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    },
    MediaPlaybackType,
};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

const POLL_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Default)]
struct State {
    /// Canonical executable keys reported by currently playing video sessions.
    app_keys: Vec<String>,
    /// Cache process names for HWNDs currently present in the latest world snapshot.
    hwnd_apps: HashMap<usize, (u32, Option<String>)>,
}

/// Polls Windows Global System Media Transport Controls on a background thread.
/// Drop signals the worker and detaches its join handle so a blocked WinRT async
/// operation cannot stall application shutdown.
pub struct MediaMonitor {
    shared: Arc<Mutex<State>>,
    stop: SyncSender<()>,
    worker: Option<JoinHandle<()>>,
}

impl MediaMonitor {
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new(State::default()));
        let (stop, receiver) = mpsc::sync_channel(1);
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("mascot-media-monitor".into())
            .spawn(move || {
                media_worker(worker_shared, receiver);
            })
            .ok();
        Self {
            shared,
            stop,
            worker,
        }
    }

    /// Applies current media state to a fresh platform snapshot. Window matching
    /// uses process executable identity, never a title-string guess.
    pub fn apply(&self, world: &mut World) {
        let mut state = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let live: std::collections::HashSet<usize> = world.windows.iter().map(|w| w.id).collect();
        state.hwnd_apps.retain(|hwnd, _| live.contains(hwnd));
        for window in &world.windows {
            let pid = window_process_id(window.id);
            if pid != 0
                && state
                    .hwnd_apps
                    .get(&window.id)
                    .is_none_or(|(cached_pid, _)| *cached_pid != pid)
            {
                let app = process_executable(pid).map(|exe| canonical_exe(&exe));
                state.hwnd_apps.insert(window.id, (pid, app));
            }
        }
        let matches = |w: &&crate::WindowInfo| {
            state.hwnd_apps.get(&w.id).is_some_and(|(_, exe)| {
                exe.as_ref()
                    .is_some_and(|exe| state.app_keys.iter().any(|app| app == exe))
            })
        };
        world.video_window = world
            .windows
            .iter()
            .rev()
            .find(|w| w.active)
            .filter(matches)
            .or_else(|| world.windows.iter().rev().find(matches))
            .map(|w| w.id);
    }
}

impl Default for MediaMonitor {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for MediaMonitor {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        // A WinRT `.get()` can wait indefinitely on a stalled system media service.
        // Dropping JoinHandle detaches; the receiver observes the stop signal after the call returns.
        let _ = self.worker.take();
    }
}

fn media_worker(shared: Arc<Mutex<State>>, stop: Receiver<()>) {
    // New worker thread owns its WinRT apartment for its entire lifetime.
    let owns_apartment = loop {
        if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok() {
            break true;
        }
        // Retry once per normal poll interval; apartment setup errors do not
        // justify blocking or terminating the interactive mascot.
        if stopping(&stop, POLL_INTERVAL) {
            return;
        }
    };
    loop {
        if stopping(&stop, Duration::ZERO) {
            break;
        }
        let keys = query_playing_video_app_keys().unwrap_or_default();
        {
            let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
            if state.app_keys != keys {
                state.app_keys = keys;
            }
        }
        if stopping(&stop, POLL_INTERVAL) {
            break;
        }
    }
    if owns_apartment {
        unsafe {
            RoUninitialize();
        }
    }
}

fn stopping(receiver: &Receiver<()>, timeout: Duration) -> bool {
    match receiver.recv_timeout(timeout) {
        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => true,
        Err(mpsc::RecvTimeoutError::Timeout) => false,
    }
}

fn query_playing_video_app_keys() -> windows::core::Result<Vec<String>> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
    let sessions = manager.GetSessions()?;
    let mut keys = Vec::new();
    for i in 0..sessions.Size()? {
        let session = match sessions.GetAt(i) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .ok()
            == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
        if !playing {
            continue;
        }
        // Audio-only sessions do not identify a video window. Require explicit
        // playback type metadata; missing/unknown metadata is not treated as video.
        let is_video = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|info| info.PlaybackType().ok())
            .and_then(|kind| kind.Value().ok())
            == Some(MediaPlaybackType::Video);
        if !is_video {
            continue;
        }
        if let Ok(app_id) = session.SourceAppUserModelId() {
            let key = canonical_app_id(&app_id.to_string());
            if !key.is_empty() && !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    Ok(keys)
}

fn canonical_app_id(app_id: &str) -> String {
    // AppUserModelIDs may include a package family and an `!application` suffix.
    let app = app_id
        .split('!')
        .next()
        .unwrap_or(app_id)
        .to_ascii_lowercase();
    if app.contains("firefox") || app.contains("mozilla.firefox") {
        return "firefox".into();
    }
    if app.contains("spotify") {
        return "spotify".into();
    }
    if app.contains("videolan") || app == "vlc" || app.ends_with(".vlc") {
        return "vlc".into();
    }
    if app.contains("microsoft.edge") {
        return "edge".into();
    }
    if app.ends_with(".exe") {
        return app.trim_end_matches(".exe").to_string();
    }
    // Packaged IDs often have `PackageName_version_publisher`; the package name
    // itself is the best executable hint after aliases above.
    let package = app.split('_').next().unwrap_or(&app);
    package.rsplit('.').next().unwrap_or(package).to_string()
}

fn canonical_exe(exe: &str) -> String {
    let name = exe
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(exe)
        .to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    if stem.contains("firefox") {
        "firefox".into()
    } else if stem.contains("spotify") {
        "spotify".into()
    } else if stem.contains("vlc") {
        "vlc".into()
    } else if stem == "msedge" || stem == "edge" {
        "edge".into()
    } else {
        stem.to_string()
    }
}

fn window_process_id(hwnd_value: usize) -> u32 {
    let hwnd = hwnd_value as windows_sys::Win32::Foundation::HWND;
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}

fn process_executable(pid: u32) -> Option<String> {
    let process: HANDLE = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buf = vec![0u16; 32768];
    let mut length = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut length) } != 0;
    unsafe {
        CloseHandle(process);
    }
    if !ok {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..length as usize]))
}

#[cfg(test)]
mod tests {
    use super::{canonical_app_id, canonical_exe};

    #[test]
    fn normalizes_firefox_source_ids_and_image_names() {
        assert_eq!(canonical_app_id("org.mozilla.firefox!Firefox"), "firefox");
        assert_eq!(canonical_app_id("Mozilla.Firefox"), "firefox");
        assert_eq!(
            canonical_exe(r"C:\Program Files\Mozilla Firefox\firefox.exe"),
            "firefox"
        );
    }

    #[test]
    fn normalizes_common_browser_and_vlc_aliases() {
        assert_eq!(
            canonical_app_id("Microsoft.Edge_1.2.3!Microsoft.Edge"),
            "edge"
        );
        assert_eq!(canonical_exe("msedge.exe"), "edge");
        assert_eq!(canonical_app_id("VideoLAN.VLC"), "vlc");
        assert_eq!(
            canonical_exe(r"C:\Program Files\VideoLAN\VLC\vlc.exe"),
            "vlc"
        );
    }
}
