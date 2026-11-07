//! Read-only desktop shortcut geometry from Explorer's public UI Automation tree.
//!
//! UI Automation can stall while Explorer is busy, so all COM work happens on a
//! detached MTA worker. The game loop only clones a small, last-known snapshot.

use crate::{Rect, WindowInfo, World};
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const MAX_DESKTOP_ICONS: usize = 128;
const MIN_ICON_EDGE: f64 = 32.0;
// UIA returns physical pixels in our DPI-aware process. A normal icon with
// a two-line label is already 150 x 157 at 200% scaling. Keep a finite bound
// that also permits Windows' larger accessibility scales.
const MAX_ICON_EDGE: f64 = 512.0;
const ICON_ID_BIT: usize = 1usize << (usize::BITS - 1);
const SNAPSHOT_TTL: Duration = Duration::from_secs(3);

#[derive(Default)]
struct CachedIcons {
    icons: Vec<WindowInfo>,
    updated: Option<Instant>,
}
impl CachedIcons {
    fn replace(&mut self, icons: Vec<WindowInfo>) {
        self.icons = icons;
        self.updated = Some(Instant::now());
    }

    fn clear(&mut self) {
        self.icons.clear();
        self.updated = None;
    }

    fn read(&self, now: Instant) -> Vec<WindowInfo> {
        if self
            .updated
            .is_some_and(|at| now.saturating_duration_since(at) <= SNAPSHOT_TTL)
        {
            self.icons.clone()
        } else {
            Vec::new()
        }
    }
}

/// Worker-backed, best-effort desktop icon geometry provider.
///
/// `snapshot` contains rectangles and opaque synthetic IDs only. It never
/// reads or retains desktop labels, paths, or shortcut contents.
pub struct DesktopMonitor {
    enabled: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    cache: Arc<Mutex<CachedIcons>>,
    _worker: Option<JoinHandle<()>>,
}

impl DesktopMonitor {
    pub fn new() -> Self {
        let enabled = Arc::new(AtomicBool::new(true));
        let stopped = Arc::new(AtomicBool::new(false));
        let cache = Arc::new(Mutex::new(CachedIcons::default()));
        let worker_enabled = Arc::clone(&enabled);
        let worker_stopped = Arc::clone(&stopped);
        let worker_cache = Arc::clone(&cache);
        let worker = thread::Builder::new()
            .name("claude-desktop-icons".into())
            .spawn(move || worker_loop(worker_enabled, worker_stopped, worker_cache))
            .ok();
        Self {
            enabled,
            stopped,
            cache,
            _worker: worker,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
        }
    }

    /// Return a cheap clone of the latest complete snapshot (at most 128 items).
    pub fn snapshot(&self) -> Vec<WindowInfo> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .read(Instant::now())
    }

    /// Append icons below ordinary windows in the world's bottom-to-top order.
    pub fn apply(&self, world: &mut World) {
        let icons = if self.enabled.load(Ordering::Acquire) {
            clip_icons_to_work_areas(self.snapshot(), &world.work_areas)
        } else {
            Vec::new()
        };
        let mut windows = icons;
        windows.extend(world.windows.drain(..).filter(|w| !w.is_desktop_icon));
        world.windows = windows;
    }

    /// Privacy-safe inspection data for QA: only rectangles, never icon names.
    pub fn icon_rectangles(&self) -> Vec<Rect> {
        self.snapshot().into_iter().map(|icon| icon.rect).collect()
    }
}

impl Default for DesktopMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DesktopMonitor {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // Do not join: an in-flight UIA/Explorer COM call has no safe bounded
        // cancellation API. The detached worker owns its state via Arc and exits
        // as soon as that call returns.
    }
}

fn clip_icons_to_work_areas(icons: Vec<WindowInfo>, areas: &[Rect]) -> Vec<WindowInfo> {
    let mut out = Vec::with_capacity(icons.len().min(MAX_DESKTOP_ICONS));
    for mut icon in icons.into_iter().take(MAX_DESKTOP_ICONS) {
        if !icon.rect.is_valid() {
            continue;
        }
        if areas.is_empty() {
            out.push(icon);
            continue;
        }
        let clipped = areas.iter().filter(|a| a.is_valid()).find_map(|a| {
            let x1 = icon.rect.x.max(a.x);
            let y1 = icon.rect.y.max(a.y);
            let x2 = icon.rect.right().min(a.right());
            let y2 = icon.rect.bottom().min(a.bottom());
            (x2 > x1 && y2 > y1).then_some(Rect::new(x1, y1, x2 - x1, y2 - y1))
        });
        let Some(rect) = clipped else {
            continue;
        };
        if rect.w < MIN_ICON_EDGE || rect.h < MIN_ICON_EDGE {
            continue;
        }
        icon.rect = rect;
        out.push(icon);
    }
    out
}

#[cfg(windows)]
fn worker_loop(enabled: Arc<AtomicBool>, stopped: Arc<AtomicBool>, cache: Arc<Mutex<CachedIcons>>) {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        return;
    }
    let mut automation = initialized
        .then(|| unsafe { create_automation() })
        .flatten();
    while !stopped.load(Ordering::Acquire) {
        if !enabled.load(Ordering::Acquire) {
            cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        if let Some(uia) = automation.as_ref() {
            let result = unsafe { collect_icons(uia) };
            if enabled.load(Ordering::Acquire) {
                if let Ok(icons) = result {
                    cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .replace(icons);
                } else {
                    cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
                }
            } else {
                cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
            }
        } else {
            // COM/UIA is unavailable. Retry infrequently in case Explorer starts.
            automation = unsafe { create_automation() };
        }
        thread::sleep(Duration::from_secs(1));
    }
    drop(automation);
    if initialized {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(not(windows))]
fn worker_loop(_: Arc<AtomicBool>, stopped: Arc<AtomicBool>, _: Arc<Mutex<CachedIcons>>) {
    while !stopped.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(windows)]
unsafe fn create_automation() -> Option<windows::Win32::UI::Accessibility::IUIAutomation> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        .ok()
        .map(|a: IUIAutomation| a)
}

#[cfg(windows)]
unsafe fn collect_icons(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
) -> windows::core::Result<Vec<WindowInfo>> {
    use windows::Win32::System::Ole::SafeArrayDestroy;
    use windows::Win32::UI::Accessibility::{
        TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_ListItemControlTypeId,
    };
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, GetClassNameW, IsWindowVisible,
    };

    // Search Progman and WorkerW desktop hosts; the DefView/ListView hierarchy
    // is how Windows Explorer hosts the icon surface on current Windows builds.
    let mut roots = Vec::<HWND>::new();
    unsafe extern "system" fn enum_top(hwnd: HWND, data: isize) -> i32 {
        let roots = &mut *(data as *mut Vec<HWND>);
        roots.push(hwnd);
        1
    }
    EnumWindows(Some(enum_top), &mut roots as *mut _ as isize);
    let mut list_view = std::ptr::null_mut();
    for root in roots {
        let mut class_buf = [0u16; 128];
        let class_len = GetClassNameW(root, class_buf.as_mut_ptr(), class_buf.len() as i32);
        if class_len <= 0
            || !is_desktop_host_class(&String::from_utf16_lossy(&class_buf[..class_len as usize]))
        {
            continue;
        }
        let def_view = FindWindowExW(
            root,
            std::ptr::null_mut(),
            wide_null("SHELLDLL_DefView").as_ptr(),
            std::ptr::null(),
        );
        if !def_view.is_null() {
            list_view = FindWindowExW(
                def_view,
                std::ptr::null_mut(),
                wide_null("SysListView32").as_ptr(),
                std::ptr::null(),
            );
            if !list_view.is_null() {
                break;
            }
        }
    }
    if list_view.is_null() || IsWindowVisible(list_view) == 0 {
        return Ok(Vec::new());
    }

    let root = automation.ElementFromHandle(windows::Win32::Foundation::HWND(list_view))?;
    let list_item_type = windows::core::VARIANT::from(UIA_ListItemControlTypeId.0);
    let condition =
        automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &list_item_type)?;
    let elements = root.FindAll(TreeScope_Descendants, &condition)?;
    let length = elements.Length()?.min(MAX_DESKTOP_ICONS as i32);
    let mut icons = Vec::with_capacity(length as usize);
    let mut ids = HashSet::with_capacity(length as usize);
    for index in 0..length {
        let Ok(element) = elements.GetElement(index) else {
            continue;
        };
        let Ok(rect) = element.CurrentBoundingRectangle() else {
            continue;
        };
        let rect = Rect::new(
            rect.left as f64,
            rect.top as f64,
            (rect.right - rect.left) as f64,
            (rect.bottom - rect.top) as f64,
        );
        if !acceptable_rect(rect) {
            continue;
        }
        let Ok(array) = element.GetRuntimeId() else {
            continue;
        };
        if array.is_null() {
            continue;
        }
        let id = runtime_id(array).unwrap_or_else(|| fallback_id(rect));
        let _ = SafeArrayDestroy(array);
        let mut unique = id;
        while !ids.insert(unique) {
            unique = (unique.wrapping_add(1) & !ICON_ID_BIT) | ICON_ID_BIT;
        }
        icons.push(WindowInfo {
            id: unique,
            rect,
            title: String::new(),
            active: false,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: true,
        });
    }
    Ok(icons)
}

#[cfg(windows)]
unsafe fn runtime_id(array: *mut windows::Win32::System::Com::SAFEARRAY) -> Option<usize> {
    use windows::Win32::System::Ole::{
        SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
    };
    let low = SafeArrayGetLBound(array, 1).ok()?;
    let high = SafeArrayGetUBound(array, 1).ok()?;
    let count = high.checked_sub(low)?.checked_add(1)?;
    if !(1..=64).contains(&count) {
        return None;
    }
    let mut hash: u64 = 0xcbf29ce484222325;
    for i in low..=high {
        let mut value = 0i32;
        SafeArrayGetElement(array, &i, &mut value as *mut _ as *mut _).ok()?;
        for byte in value.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Some((hash as usize & !ICON_ID_BIT) | ICON_ID_BIT)
}

#[cfg(windows)]
fn fallback_id(rect: Rect) -> usize {
    let mut h: u64 = 0xcbf29ce484222325;
    for value in [rect.x as i32, rect.y as i32, rect.w as i32, rect.h as i32] {
        for byte in value.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    (h as usize & !ICON_ID_BIT) | ICON_ID_BIT
}

#[cfg(windows)]
fn acceptable_rect(rect: Rect) -> bool {
    rect.is_valid()
        && (MIN_ICON_EDGE..=MAX_ICON_EDGE).contains(&rect.w)
        && (MIN_ICON_EDGE..=MAX_ICON_EDGE).contains(&rect.h)
}

#[cfg(windows)]
fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn is_desktop_host_class(class: &str) -> bool {
    matches!(class, "Progman" | "WorkerW")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(id: usize, rect: Rect) -> WindowInfo {
        WindowInfo {
            id,
            rect,
            title: String::new(),
            active: false,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: true,
        }
    }

    #[test]
    fn clipping_rejects_invalid_and_out_of_bounds_geometry() {
        let icons = vec![
            icon(1, Rect::new(5.0, 5.0, 64.0, 64.0)),
            icon(2, Rect::new(500.0, 500.0, 64.0, 64.0)),
            icon(3, Rect::new(f64::NAN, 0.0, 64.0, 64.0)),
            icon(4, Rect::new(95.0, 10.0, 64.0, 64.0)),
        ];
        let clipped = clip_icons_to_work_areas(icons, &[Rect::new(0.0, 0.0, 128.0, 128.0)]);
        assert_eq!(clipped.len(), 2);
        assert_eq!(clipped[1].rect, Rect::new(95.0, 10.0, 33.0, 64.0));
    }

    #[test]
    fn apply_keeps_icons_below_real_windows_and_replaces_old_icons() {
        let desktop = DesktopMonitor {
            enabled: Arc::new(AtomicBool::new(true)),
            stopped: Arc::new(AtomicBool::new(false)),
            cache: Arc::new(Mutex::new(CachedIcons {
                icons: vec![icon(usize::MAX, Rect::new(0.0, 0.0, 64.0, 64.0))],
                updated: Some(Instant::now()),
            })),
            _worker: None,
        };
        let mut world = World {
            work_areas: vec![Rect::new(-100.0, -100.0, 500.0, 500.0)],
            ..World::default()
        };
        world.windows.push(WindowInfo {
            id: 9,
            rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            title: "window".into(),
            active: true,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: false,
        });
        world.windows.push(icon(7, Rect::new(1.0, 1.0, 48.0, 48.0)));
        desktop.apply(&mut world);
        assert_eq!(
            world.windows.iter().map(|w| w.id).collect::<Vec<_>>(),
            [usize::MAX, 9]
        );
        assert!(world.windows[0].is_desktop_icon);
    }

    #[test]
    fn synthetic_id_uses_high_bit_and_bounds_are_limited() {
        assert_ne!(ICON_ID_BIT, 0);
        assert!(!acceptable_rect(Rect::new(0.0, 0.0, 513.0, 64.0)));
        assert!(!acceptable_rect(Rect::new(0.0, 0.0, f64::INFINITY, 64.0)));
        assert!(acceptable_rect(Rect::new(0.0, 0.0, 64.0, 64.0)));
    }

    #[test]
    fn physical_icon_rectangles_survive_high_dpi_scaling() {
        for (width, height) in [(75.0, 79.0), (150.0, 157.0), (300.0, 314.0)] {
            assert!(acceptable_rect(Rect::new(0.0, 0.0, width, height)));
        }
        assert!(!acceptable_rect(Rect::new(0.0, 0.0, 25.0, 25.0)));
    }

    #[test]
    fn expired_cache_does_not_keep_ghost_perches() {
        let cache = CachedIcons {
            icons: vec![icon(1, Rect::new(0.0, 0.0, 64.0, 64.0))],
            updated: Some(Instant::now() - SNAPSHOT_TTL - Duration::from_millis(1)),
        };
        assert!(cache.read(Instant::now()).is_empty());
    }

    #[test]
    fn desktop_host_class_filter_excludes_folder_windows() {
        assert!(is_desktop_host_class("Progman"));
        assert!(is_desktop_host_class("WorkerW"));
        assert!(!is_desktop_host_class("CabinetWClass"));
        assert!(!is_desktop_host_class("ExploreWClass"));
    }
}
