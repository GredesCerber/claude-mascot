//! Read-only, bounded UI Automation geometry collection.
use crate::{Rect, UiSurface, World};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
const SNAPSHOT_TTL: Duration = Duration::from_millis(1500);

#[derive(Default)]
struct Cache {
    revision: usize,
    time: Option<Instant>,
    surfaces: Vec<UiSurface>,
}
pub struct UiPerchMonitor {
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    owner: Arc<AtomicUsize>,
    cache: Arc<Mutex<Cache>>,
    revision: Arc<AtomicUsize>,
}
impl Default for UiPerchMonitor {
    fn default() -> Self {
        Self::new()
    }
}
impl UiPerchMonitor {
    pub fn new() -> Self {
        let enabled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let owner = Arc::new(AtomicUsize::new(0));
        let cache = Arc::new(Mutex::new(Cache::default()));
        let revision = Arc::new(AtomicUsize::new(0));
        let (e, s, o, c, r) = (
            enabled.clone(),
            stop.clone(),
            owner.clone(),
            cache.clone(),
            revision.clone(),
        );
        let _ = thread::Builder::new()
            .name("claude-ui-geometry".into())
            .spawn(move || worker(e, s, o, c, r));
        Self {
            enabled,
            stop,
            owner,
            cache,
            revision,
        }
    }
    pub fn set_enabled(&self, enabled: bool) {
        if self.enabled.swap(enabled, Ordering::AcqRel) != enabled {
            self.revision.fetch_add(1, Ordering::AcqRel);
        }
        if !enabled {
            *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = Cache::default();
        }
    }
    pub fn snapshot(&self, world: &World) -> Vec<UiSurface> {
        let owner = world
            .windows
            .iter()
            .rev()
            .find(|w| w.active && !w.is_desktop_icon && !w.fullscreen)
            .map_or(0, |w| w.id);
        if self.owner.swap(owner, Ordering::AcqRel) != owner {
            self.revision.fetch_add(1, Ordering::AcqRel);
        }
        if owner == 0 || !self.enabled.load(Ordering::Acquire) {
            return vec![];
        }
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.revision != self.revision.load(Ordering::Acquire)
            || !cache.time.is_some_and(|t| t.elapsed() <= SNAPSHOT_TTL)
        {
            return vec![];
        }
        cache
            .surfaces
            .iter()
            .filter(|s| visible(s, world, owner))
            .cloned()
            .collect()
    }
}
impl Drop for UiPerchMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
fn inside(a: Rect, b: Rect) -> bool {
    a.is_valid()
        && b.is_valid()
        && a.x >= b.x
        && a.y >= b.y
        && a.right() <= b.right()
        && a.bottom() <= b.bottom()
}
fn overlap(a: Rect, b: Rect) -> bool {
    a.is_valid()
        && b.is_valid()
        && a.x < b.right()
        && b.x < a.right()
        && a.y < b.bottom()
        && b.y < a.bottom()
}
fn visible(s: &UiSurface, w: &World, owner: usize) -> bool {
    let Some(i) = w
        .windows
        .iter()
        .position(|v| v.id == owner && v.active && !v.fullscreen)
    else {
        return false;
    };
    s.owner == owner
        && inside(s.rect, w.windows[i].rect)
        && !w.windows[i + 1..]
            .iter()
            .any(|v| !v.is_desktop_icon && overlap(v.rect, s.rect))
}
#[derive(Default)]
struct Tracker {
    previous: Vec<(UiSurface, u64)>,
    next: u64,
    last_confirmed: Option<Instant>,
}
impl Tracker {
    fn clear(&mut self) {
        self.previous.clear();
        self.last_confirmed = None;
    }
    fn finish_collection(
        &mut self,
        current: Vec<UiSurface>,
        started: Instant,
        finished: Instant,
    ) -> Option<Vec<UiSurface>> {
        if finished.saturating_duration_since(started) > SNAPSHOT_TTL {
            self.clear();
            return None;
        }
        if self
            .last_confirmed
            .is_some_and(|at| finished.saturating_duration_since(at) > SNAPSHOT_TTL)
        {
            self.clear();
        }
        self.last_confirmed = Some(finished);
        Some(self.observe(current))
    }
    fn observe(&mut self, current: Vec<UiSurface>) -> Vec<UiSurface> {
        let mut next_previous = Vec::new();
        let mut stable = Vec::new();
        for surface in current {
            let old = self.previous.iter().find(|(p, _)| {
                p.owner == surface.owner
                    && p.id == surface.id
                    && (p.rect.x - surface.rect.x).abs() <= 20.0
                    && (p.rect.y - surface.rect.y).abs() <= 20.0
                    && (p.rect.w - surface.rect.w).abs() <= 20.0
                    && (p.rect.h - surface.rect.h).abs() <= 20.0
            });
            let generation = if let Some((_, generation)) = old {
                *generation
            } else {
                self.next = self.next.wrapping_add(1);
                self.next
            };
            if old.is_some() {
                let mut accepted = surface.clone();
                accepted.id = generation;
                stable.push(accepted);
            }
            next_previous.push((surface, generation));
        }
        self.previous = next_previous;
        stable
    }
}
fn worker(
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    owner: Arc<AtomicUsize>,
    cache: Arc<Mutex<Cache>>,
    revision: Arc<AtomicUsize>,
) {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
        return;
    }
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
    let mut tracker = Tracker::default();
    let mut last_revision = revision.load(Ordering::Acquire);
    while !stop.load(Ordering::Acquire) {
        let rev = revision.load(Ordering::Acquire);
        if rev != last_revision {
            tracker.clear();
            last_revision = rev;
        }
        let target = owner.load(Ordering::Acquire);
        if enabled.load(Ordering::Acquire) && target != 0 {
            let started = Instant::now();
            let current = automation
                .as_ref()
                .and_then(|a| unsafe { collect(a, target).ok() })
                .unwrap_or_default();
            if enabled.load(Ordering::Acquire)
                && revision.load(Ordering::Acquire) == rev
                && owner.load(Ordering::Acquire) == target
            {
                let finished = Instant::now();
                let result = tracker.finish_collection(current, started, finished);
                *cache.lock().unwrap_or_else(|e| e.into_inner()) = match result {
                    Some(stable) => Cache {
                        revision: rev,
                        time: Some(finished),
                        surfaces: stable,
                    },
                    None => Cache::default(),
                };
            }
        } else {
            tracker.clear();
            *cache.lock().unwrap_or_else(|e| e.into_inner()) = Cache::default();
        }
        thread::sleep(Duration::from_millis(500));
    }
    drop(automation);
    unsafe {
        CoUninitialize();
    }
}
unsafe fn collect(
    a: &windows::Win32::UI::Accessibility::IUIAutomation,
    owner: usize,
) -> windows::core::Result<Vec<UiSurface>> {
    use windows::Win32::{
        Foundation::HWND,
        System::Ole::{
            SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
        },
        UI::Accessibility::{UIA_EditControlTypeId, UIA_SeparatorControlTypeId},
    };
    let root = a.ElementFromHandle(HWND(owner as *mut _))?;
    let walker = a.ControlViewWalker()?;
    let mut stack = vec![(root, 0usize, None::<Rect>)];
    let mut out = vec![];
    let mut seen = 0;
    while let Some((element, depth, clip)) = stack.pop() {
        seen += 1;
        if seen > 256 {
            break;
        }
        if element.CurrentIsOffscreen().map_or(true, |x| x.as_bool()) {
            continue;
        }
        let Ok(r) = element.CurrentBoundingRectangle() else {
            continue;
        };
        let rect = Rect::new(
            r.left as f64,
            r.top as f64,
            f64::from(r.right) - f64::from(r.left),
            f64::from(r.bottom) - f64::from(r.top),
        );
        if !rect.is_valid() || clip.is_some_and(|p| !inside(rect, p)) {
            continue;
        }
        let kind = element.CurrentControlType().ok();
        if out.len() < 32
            && rect.w >= 80.
            && rect.h >= 2.
            && rect.h <= 400.
            && (kind == Some(UIA_SeparatorControlTypeId)
                || (kind == Some(UIA_EditControlTypeId)
                    && element.CurrentIsPassword().is_ok_and(|v| !v.as_bool())))
        {
            if let Ok(array) = element.GetRuntimeId() {
                if !array.is_null() {
                    let bounds = SafeArrayGetLBound(array, 1)
                        .ok()
                        .zip(SafeArrayGetUBound(array, 1).ok());
                    let mut hash = 0xcbf29ce484222325u64;
                    let mut valid = false;
                    if let Some((lo, hi)) = bounds {
                        if hi >= lo && hi - lo < 64 {
                            valid = true;
                            for i in lo..=hi {
                                let mut v = 0i32;
                                if SafeArrayGetElement(array, &i, &mut v as *mut _ as *mut _)
                                    .is_err()
                                {
                                    valid = false;
                                    break;
                                }
                                for b in v.to_le_bytes() {
                                    hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
                                }
                            }
                        }
                    }
                    let _ = SafeArrayDestroy(array);
                    if valid {
                        out.push(UiSurface {
                            owner,
                            id: hash,
                            rect,
                        });
                    }
                }
            }
        }
        if depth < 8 {
            let mut child = walker.GetFirstChildElement(&element).ok();
            let mut siblings = 0;
            while let Some(node) = child {
                siblings += 1;
                if siblings > 256 || stack.len() >= 256 {
                    break;
                }
                child = walker.GetNextSiblingElement(&node).ok();
                stack.push((node, depth + 1, Some(rect)));
            }
        }
    }
    // Ambiguous runtime IDs are not safe identities.
    let mut counts = std::collections::HashMap::new();
    for item in &out {
        *counts.entry(item.id).or_insert(0usize) += 1;
    }
    out.retain(|item| counts.get(&item.id) == Some(&1));
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowInfo;
    fn win(id: usize, rect: Rect, active: bool) -> WindowInfo {
        WindowInfo {
            id,
            rect,
            title: String::new(),
            active,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: false,
        }
    }
    #[test]
    fn rejects_hidden_and_occluded() {
        let s = UiSurface {
            owner: 1,
            id: 1,
            rect: Rect::new(10., 10., 100., 20.),
        };
        let mut w = World {
            windows: vec![win(1, Rect::new(0., 0., 300., 300.), true)],
            ..World::default()
        };
        assert!(visible(&s, &w, 1));
        w.windows
            .push(win(2, Rect::new(50., 0., 100., 100.), false));
        assert!(!visible(&s, &w, 1));
        w.windows.pop();
        w.windows[0].active = false;
        assert!(!visible(&s, &w, 1));
    }
    #[test]
    fn rejects_clipped_rectangles() {
        assert!(!inside(
            Rect::new(-1., 0., 20., 20.),
            Rect::new(0., 0., 100., 100.)
        ));
    }
    #[test]
    fn identity_expires_on_disappearance_and_large_motion() {
        let mut tracker = Tracker::default();
        let surface = UiSurface {
            owner: 1,
            id: 8,
            rect: Rect::new(20., 20., 120., 30.),
        };
        assert!(tracker.observe(vec![surface.clone()]).is_empty());
        let original = tracker.observe(vec![surface.clone()])[0].id;
        let mut moved = surface.clone();
        moved.rect.x += 5.;
        assert_eq!(tracker.observe(vec![moved.clone()])[0].id, original);
        tracker.observe(vec![]);
        assert!(tracker.observe(vec![moved.clone()]).is_empty());
        assert_ne!(tracker.observe(vec![moved.clone()])[0].id, original);
        moved.rect.y += 100.;
        assert!(tracker.observe(vec![moved]).is_empty());
    }
    #[test]
    fn stale_or_slow_collection_never_revives_old_identity() {
        let mut tracker = Tracker::default();
        let surface = UiSurface {
            owner: 1,
            id: 8,
            rect: Rect::new(20., 20., 120., 30.),
        };
        let t = Instant::now();
        assert!(tracker
            .finish_collection(vec![surface.clone()], t, t)
            .unwrap()
            .is_empty());
        let old = tracker
            .finish_collection(
                vec![surface.clone()],
                t + Duration::from_millis(500),
                t + Duration::from_millis(500),
            )
            .unwrap()[0]
            .id;
        let expired = t + Duration::from_millis(2100);
        assert!(tracker
            .finish_collection(vec![surface.clone()], expired, expired)
            .unwrap()
            .is_empty());
        let new_id = tracker
            .finish_collection(vec![surface.clone()], expired, expired)
            .unwrap()[0]
            .id;
        assert_ne!(old, new_id);
        let started = expired + Duration::from_millis(500);
        let finished = started + SNAPSHOT_TTL + Duration::from_millis(1);
        assert!(tracker
            .finish_collection(vec![surface.clone()], started, finished)
            .is_none());
        assert!(tracker
            .finish_collection(vec![surface.clone()], finished, finished)
            .unwrap()
            .is_empty());
        assert_ne!(
            tracker
                .finish_collection(vec![surface], finished, finished)
                .unwrap()[0]
                .id,
            new_id
        );
    }
}
