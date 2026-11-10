//! Native Windows host. Coordinates are physical virtual-desktop pixels and the
//! pixel surface passed to `present` is premultiplied BGRA (ARGB `u32` values).
use crate::{Rect, Vec2, WindowInfo, World};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Mutex, OnceLock,
};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    CreateMutexW, CreateWaitableTimerExW, GetCurrentProcessId, SetWaitableTimer, INFINITE,
    SYNCHRONIZATION_SYNCHRONIZE, TIMER_MODIFY_STATE,
};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: &[u16] = &[
    99, 108, 97, 117, 100, 101, 45, 109, 97, 115, 99, 111, 116, 45, 110, 97, 116, 105, 118, 101, 0,
];
const TRAY_MESSAGE: u32 = WM_APP + 23;
const TRAY_ICON: &[u8] = include_bytes!("../assets/claude-icon.ico");
// Always return to the frame loop even if mouse input keeps arriving faster
// than it can be dispatched. The next iteration resumes draining the queue.
const MAX_MESSAGES_PER_PUMP: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    PointerDown(Vec2),
    PointerMove(Vec2),
    PointerUp(Vec2),
    DoubleClick,
    Command(u16),
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub struct MenuState {
    pub selected_action: u16,
    pub paused: bool,
    pub hide_fullscreen: bool,
    pub autostart: bool,
    pub auto_updates: bool,
    pub chase: bool,
    pub scale: f64,
    pub target_fps: u32,
    pub show_bubbles: bool,
    pub show_mood: bool,
    pub social_enabled: bool,
    pub auto_activities: bool,
    pub auto_reactions: bool,
    pub desktop_perches: bool,
    pub personal_space_min: f64,
    pub personal_space_max: f64,
    pub body_color: u32,
    pub satisfaction: u8,
}
impl Default for MenuState {
    fn default() -> Self {
        Self {
            selected_action: 1,
            paused: false,
            hide_fullscreen: false,
            autostart: false,
            auto_updates: true,
            chase: false,
            scale: 1.0,
            target_fps: crate::timing::DEFAULT_FPS,
            show_bubbles: true,
            show_mood: true,
            social_enabled: true,
            auto_activities: true,
            auto_reactions: true,
            desktop_perches: true,
            personal_space_min: 24.0,
            personal_space_max: 96.0,
            body_color: crate::palette::DEFAULT_BODY_COLOR,
            satisfaction: 75,
        }
    }
}

/// Own this guard for the lifetime of the process. `Ok(None)` means another copy is running.
pub struct InstanceGuard(HANDLE);
impl InstanceGuard {
    pub fn acquire(name: &str) -> Result<Option<Self>, String> {
        let name = wide(name);
        let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(last_error("CreateMutexW"));
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(handle);
            }
            Ok(None)
        } else {
            Ok(Some(Self(handle)))
        }
    }
}
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                CloseHandle(self.0);
            }
        }
    }
}

/// Establish per-monitor physical-pixel coordinates before creating any windows.
pub fn initialize() -> Result<(), String> {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            let e = GetLastError();
            // ERROR_ACCESS_DENIED usually means the executable manifest already
            // selected awareness. Accept it only when it selected the exact
            // context this host requires for physical-pixel coordinates.
            let already_per_monitor_v2 = e == ERROR_ACCESS_DENIED
                && AreDpiAwarenessContextsEqual(
                    GetThreadDpiAwarenessContext(),
                    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                ) != 0;
            if !already_per_monitor_v2 {
                return Err(format!("SetProcessDpiAwarenessContext failed ({e})"));
            }
        }
    }
    ensure_class()
}

struct HostState {
    events: Mutex<Vec<InputEvent>>,
    menu_state: Mutex<MenuState>,
    dragging: AtomicBool,
    visible: AtomicBool,
    hwnd: HWND,
    tray: bool,
    tray_id: u32,
    tray_icon: HICON,
}
static NEXT_TRAY_ID: AtomicU32 = AtomicU32::new(1);
static CLASS: OnceLock<Result<(), String>> = OnceLock::new();

pub struct NativeHost {
    state: Box<HostState>,
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    dib: *mut c_void,
    dib_size: (i32, i32),
}

impl NativeHost {
    pub fn choose_body_color(&self, initial: u32) -> Option<u32> {
        use windows_sys::Win32::UI::Controls::Dialogs::*;
        let convert = |rgb: u32| ((rgb & 255) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 255);
        let mut custom = [0xFFFFFFu32; 16];
        for (slot, (_, _, rgb)) in custom.iter_mut().zip(crate::palette::PRESETS.iter()) {
            *slot = convert(*rgb);
        }
        let mut dialog = CHOOSECOLORW {
            lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: self.state.hwnd,
            rgbResult: convert(initial),
            lpCustColors: custom.as_mut_ptr(),
            Flags: CC_FULLOPEN | CC_RGBINIT | CC_ANYCOLOR,
            ..unsafe { std::mem::zeroed() }
        };
        // SAFETY: owner is a live native window; custom colors and dialog
        // storage remain valid until this synchronous OS dialog returns.
        (unsafe { ChooseColorW(&mut dialog) } != 0).then(|| convert(dialog.rgbResult))
    }
    pub fn new(title: &str, width: i32, height: i32, tray: bool) -> Result<Self, String> {
        ensure_class()?;
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".into());
        }
        let mut state = Box::new(HostState {
            events: Mutex::new(Vec::new()),
            menu_state: Mutex::new(MenuState::default()),
            dragging: AtomicBool::new(false),
            visible: AtomicBool::new(false),
            hwnd: null_mut(),
            tray,
            tray_id: NEXT_TRAY_ID.fetch_add(1, Ordering::Relaxed),
            tray_icon: null_mut(),
        });
        let hinst = unsafe { GetModuleHandleW(null()) };
        if hinst.is_null() {
            return Err(last_error("GetModuleHandleW"));
        }
        let title = wide(title);
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS_NAME.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                width,
                height,
                null_mut(),
                null_mut(),
                hinst,
                (&mut *state as *mut HostState).cast(),
            )
        };
        if hwnd.is_null() {
            return Err(last_error("CreateWindowExW"));
        }
        state.hwnd = hwnd;
        let mut host = Self {
            state,
            dc: null_mut(),
            bitmap: null_mut(),
            old_bitmap: null_mut(),
            dib: null_mut(),
            dib_size: (0, 0),
        };
        if tray {
            if let Err(e) = host.add_tray() {
                drop(host);
                return Err(e);
            }
        }
        Ok(host)
    }

    pub fn hwnd(&self) -> HWND {
        self.state.hwnd
    }

    /// Make this host visible to UI inspection tools without activating it.
    /// The default host remains a tool window; callers must opt in explicitly.
    pub fn expose_for_inspection(&self) -> Result<(), String> {
        unsafe {
            let mut style = GetWindowLongPtrW(self.state.hwnd, GWL_EXSTYLE);
            style &= !(WS_EX_TOOLWINDOW as isize);
            style |= WS_EX_APPWINDOW as isize;
            SetLastError(0);
            let previous = SetWindowLongPtrW(self.state.hwnd, GWL_EXSTYLE, style);
            let error = GetLastError();
            if previous == 0 && error != 0 {
                return Err(format!("SetWindowLongPtrW(GWL_EXSTYLE) failed ({error})"));
            }
            if SetWindowPos(
                self.state.hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            ) == 0
            {
                return Err(last_error("SetWindowPos(SWP_FRAMECHANGED)"));
            }
        }
        Ok(())
    }

    /// Make this overlay ignore pointer input while remaining visible and topmost.
    /// Intended for cosmetic overlays that must never block the foreign window.
    pub fn set_input_transparent(&self) -> Result<(), String> {
        unsafe {
            let mut style = GetWindowLongPtrW(self.state.hwnd, GWL_EXSTYLE);
            style |= (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) as isize;
            SetLastError(0);
            let previous = SetWindowLongPtrW(self.state.hwnd, GWL_EXSTYLE, style);
            let error = GetLastError();
            if previous == 0 && error != 0 {
                return Err(format!("SetWindowLongPtrW(GWL_EXSTYLE) failed ({error})"));
            }
            if SetWindowPos(
                self.state.hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            ) == 0
            {
                return Err(last_error("SetWindowPos(SWP_FRAMECHANGED)"));
            }
        }
        Ok(())
    }

    pub fn take_events(&self) -> Vec<InputEvent> {
        self.state
            .events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }
    pub fn set_visible(&self, visible: bool) {
        if !change_visibility(&self.state.visible, visible) {
            return;
        }
        unsafe {
            ShowWindow(
                self.state.hwnd,
                if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
            );
        }
    }
    pub fn set_size(&self, width: i32, height: i32) -> Result<(), String> {
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".into());
        }
        if unsafe {
            SetWindowPos(
                self.state.hwnd,
                null_mut(),
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        } == 0
        {
            Err(last_error("SetWindowPos"))
        } else {
            Ok(())
        }
    }
    pub fn set_menu_state(&self, menu_state: MenuState) {
        *self
            .state
            .menu_state
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = menu_state;
    }

    /// Present one premultiplied BGRA canvas at physical desktop coordinate `(x,y)`.
    pub fn present(&mut self, pixels: &[u32], x: i32, y: i32) -> Result<(), String> {
        let mut size = unsafe { zeroed::<RECT>() };
        if unsafe { GetWindowRect(self.state.hwnd, &mut size) } == 0 {
            return Err(last_error("GetWindowRect"));
        }
        let width = size.right - size.left;
        let height = size.bottom - size.top;
        if width <= 0 || height <= 0 || pixels.len() < width as usize * height as usize {
            return Err("pixel buffer is smaller than the host window".into());
        }
        if self.dib_size != (width, height) {
            self.resize_dib(width, height)?;
        }
        // SAFETY: resize_dib created a writable 32-bit top-down DIB of exactly this
        // size, and the length check above guarantees the source contains all pixels.
        unsafe {
            std::ptr::copy_nonoverlapping(
                pixels.as_ptr(),
                self.dib.cast::<u32>(),
                (width as usize) * (height as usize),
            );
        }
        let dst = POINT { x, y };
        let src = POINT { x: 0, y: 0 };
        let sz = SIZE {
            cx: width,
            cy: height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        if unsafe {
            UpdateLayeredWindow(
                self.state.hwnd,
                null_mut(),
                &dst,
                &sz,
                self.dc,
                &src,
                0,
                &blend,
                ULW_ALPHA,
            )
        } == 0
        {
            return Err(last_error("UpdateLayeredWindow"));
        }
        Ok(())
    }

    fn resize_dib(&mut self, width: i32, height: i32) -> Result<(), String> {
        unsafe {
            if !self.dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.dc, self.old_bitmap);
                    self.old_bitmap = null_mut();
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                    self.bitmap = null_mut();
                }
                DeleteDC(self.dc);
                self.dc = null_mut();
                self.dib = null_mut();
                self.dib_size = (0, 0);
            }
            let screen = GetDC(null_mut());
            if screen.is_null() {
                return Err(last_error("GetDC"));
            }
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(null_mut(), screen);
            if dc.is_null() {
                return Err(last_error("CreateCompatibleDC"));
            }
            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(dc);
                return Err(last_error("CreateDIBSection"));
            }
            let old = SelectObject(dc, bitmap);
            if old.is_null() || old as isize == GDI_ERROR as isize {
                DeleteObject(bitmap);
                DeleteDC(dc);
                return Err(last_error("SelectObject"));
            }
            self.dc = dc;
            self.bitmap = bitmap;
            self.old_bitmap = old;
            self.dib = bits;
            self.dib_size = (width, height);
        }
        Ok(())
    }

    fn add_tray(&mut self) -> Result<(), String> {
        let mut data: NOTIFYICONDATAW = unsafe { zeroed() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.state.hwnd;
        data.uID = self.state.tray_id;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        let icon = unsafe { tray_icon_from_ico(TRAY_ICON) };
        if icon.is_null() {
            return Err("embedded tray icon could not be decoded".into());
        }
        data.hIcon = icon;
        let tip = wide("Клод — питомец рабочего стола");
        let n = tip.len().min(data.szTip.len());
        data.szTip[..n].copy_from_slice(&tip[..n]);
        if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
            unsafe { DestroyIcon(icon) };
            Err(last_error("Shell_NotifyIconW(NIM_ADD)"))
        } else {
            // The host owns this icon until it removes the tray entry.
            self.state.tray_icon = icon;
            Ok(())
        }
    }

    pub fn pump() -> bool {
        unsafe {
            let mut msg: MSG = zeroed();
            let mut running = true;
            for _ in 0..MAX_MESSAGES_PER_PUMP {
                if PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                if msg.message == WM_QUIT {
                    running = false;
                    break;
                }
                if crate::preferences::preprocess_message(&msg) {
                    continue;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            running
        }
    }

    pub fn wait(milliseconds: u32) {
        unsafe {
            MsgWaitForMultipleObjects(0, null(), 0, milliseconds, QS_ALLINPUT);
        }
    }

    /// Wait for a frame deadline while still waking for native input. Uses a
    /// per-call high-resolution waitable timer when Windows supports it.
    pub fn wait_precise(duration: std::time::Duration) {
        let ticks = duration.as_nanos().saturating_add(99) / 100;
        if ticks == 0 {
            return;
        }
        unsafe {
            // CREATE_WAITABLE_TIMER_HIGH_RESOLUTION is available on newer
            // Windows builds; older builds reject the flag and use the legacy wait.
            let timer = CreateWaitableTimerExW(
                null(),
                null(),
                0x2,
                TIMER_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
            );
            if timer.is_null() {
                Self::wait(wait_millis_ceil(duration));
                return;
            }
            struct TimerGuard(HANDLE);
            impl Drop for TimerGuard {
                fn drop(&mut self) {
                    unsafe {
                        CloseHandle(self.0);
                    }
                }
            }
            let _guard = TimerGuard(timer);
            let due = -(ticks.min(i64::MAX as u128) as i64);
            if SetWaitableTimer(timer, &due, 0, None, null(), 0) == 0 {
                Self::wait(wait_millis_ceil(duration));
                return;
            }
            MsgWaitForMultipleObjects(1, &timer, 0, INFINITE, QS_ALLINPUT);
        }
    }
}

fn change_visibility(current: &AtomicBool, visible: bool) -> bool {
    current.swap(visible, Ordering::Relaxed) != visible
}

fn enqueue_pointer_move(events: &mut Vec<InputEvent>, point: Vec2) {
    if let Some(InputEvent::PointerMove(last)) = events.last_mut() {
        *last = point;
    } else {
        events.push(InputEvent::PointerMove(point));
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        unsafe {
            if self.state.tray {
                let mut data: NOTIFYICONDATAW = zeroed();
                data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                data.hWnd = self.state.hwnd;
                data.uID = self.state.tray_id;
                Shell_NotifyIconW(NIM_DELETE, &data);
            }
            if !self.state.tray_icon.is_null() {
                DestroyIcon(self.state.tray_icon);
                self.state.tray_icon = null_mut();
            }
            if !self.dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                DeleteDC(self.dc);
            }
            if !self.state.hwnd.is_null() && IsWindow(self.state.hwnd) != 0 {
                DestroyWindow(self.state.hwnd);
            }
        }
    }
}

fn wait_millis_ceil(duration: std::time::Duration) -> u32 {
    duration
        .as_millis()
        .saturating_add(u128::from(
            !duration.subsec_nanos().is_multiple_of(1_000_000),
        ))
        .min(u32::MAX as u128) as u32
}

#[cfg(windows)]
unsafe fn tray_icon_from_ico(ico: &[u8]) -> HICON {
    // ICO directory entries point at RT_ICON-compatible payloads. The checked-in
    // icon includes PNG-compressed frames, supported by Windows Vista and newer.
    if ico.len() < 6 || ico[0..4] != [0, 0, 1, 0] {
        return null_mut();
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let directory_end = match 6usize.checked_add(count.saturating_mul(16)) {
        Some(end) if end <= ico.len() => end,
        _ => return null_mut(),
    };
    let mut selected: Option<(usize, usize, usize)> = None;
    for entry in ico[6..directory_end].as_chunks::<16>().0 {
        let width = if entry[0] == 0 {
            256
        } else {
            entry[0] as usize
        };
        let height = if entry[1] == 0 {
            256
        } else {
            entry[1] as usize
        };
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        if width >= 32
            && height >= 32
            && offset >= directory_end
            && offset.checked_add(size).is_some_and(|end| end <= ico.len())
            && selected.is_none_or(|(_, _, best): (usize, usize, usize)| width * height > best)
        {
            selected = Some((offset, size, width * height));
        }
    }
    let Some((offset, size, _)) = selected else {
        return null_mut();
    };
    CreateIconFromResourceEx(
        ico[offset..offset + size].as_ptr(),
        size as u32,
        1,
        0x0003_0000,
        32,
        32,
        LR_DEFAULTCOLOR,
    )
}

const FPS_ITEMS: &[(u16, u32, &str)] = &[
    (80, 30, "30 кадров/с"),
    (81, 60, "60 кадров/с"),
    (82, 120, "120 кадров/с"),
    (83, 144, "144 кадра/с"),
    (84, 240, "240 кадров/с"),
];

/// Owns a menu tree until it is attached to its parent (or explicitly destroyed).
/// DestroyMenu recursively frees any submenus already attached to the root.
struct MenuGuard(HMENU);
impl MenuGuard {
    unsafe fn popup() -> Option<Self> {
        let menu = CreatePopupMenu();
        (!menu.is_null()).then_some(Self(menu))
    }
    fn raw(&self) -> HMENU {
        self.0
    }
    fn into_raw(self) -> HMENU {
        let menu = self.0;
        std::mem::forget(self);
        menu
    }
}
impl Drop for MenuGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                DestroyMenu(self.0);
            }
        }
    }
}

unsafe fn append_item(menu: HMENU, id: u16, label: &str, flags: u32) -> bool {
    let label = wide(label);
    AppendMenuW(menu, flags | MF_STRING, id as usize, label.as_ptr()) != 0
}

unsafe fn append_submenu(parent: HMENU, child: MenuGuard, label: &str) -> bool {
    let label = wide(label);
    if AppendMenuW(parent, MF_POPUP, child.raw() as usize, label.as_ptr()) == 0 {
        return false;
    }
    child.into_raw(); // ownership moved to parent
    true
}

unsafe fn build_menu_tree(s: MenuState) -> Option<MenuGuard> {
    let root = MenuGuard::popup()?;
    let activities = MenuGuard::popup()?;
    for (id, label) in [
        (70, "Прыгнуть"),
        (71, "Погладить"),
        (72, "Потанцевать"),
        (73, "Помахать"),
        (74, "Печатать на ноутбуке"),
        (75, "Потянуться"),
        (76, "Осмотреться"),
    ] {
        if !append_item(activities.raw(), id, label, 0) {
            return None;
        }
    }
    for id in 74..=76 {
        check_menu(activities.raw(), id, s.selected_action == id);
    }
    let scenes = MenuGuard::popup()?;
    for (index, spec) in crate::activities::CATALOG.iter().enumerate() {
        if crate::activities::CATALOG[..index]
            .iter()
            .any(|previous| previous.group == spec.group)
        {
            continue;
        }
        let group = MenuGuard::popup()?;
        for scene in crate::activities::CATALOG
            .iter()
            .filter(|scene| scene.group == spec.group)
        {
            let command = 100 + scene.id.0 as u16;
            if !append_item(group.raw(), command, scene.title_ru, 0) {
                return None;
            }
            check_menu(group.raw(), command, s.selected_action == command);
        }
        if !append_submenu(scenes.raw(), group, spec.group.label_ru()) {
            return None;
        }
    }
    if !append_submenu(activities.raw(), scenes, "Все занятия (75)") {
        return None;
    }
    if !append_submenu(root.raw(), activities, "Заняться")
        || !append_item(
            root.raw(),
            0,
            &format!(
                "Настроение: {} · {}%",
                if s.satisfaction >= 70 {
                    "доволен"
                } else if s.satisfaction >= 35 {
                    "спокоен"
                } else {
                    "сердится"
                },
                s.satisfaction.min(100)
            ),
            MF_GRAYED,
        )
        || AppendMenuW(root.raw(), MF_SEPARATOR, 0, null()) == 0
    {
        return None;
    }

    let actions = MenuGuard::popup()?;
    for (id, label) in [
        (1, "Гулять"),
        (2, "Сидеть"),
        (3, "Спать"),
        (4, "Карабкаться"),
        (5, "Бегать за курсором"),
    ] {
        if !append_item(actions.raw(), id, label, 0) {
            return None;
        }
    }
    if (1..=4).contains(&s.selected_action) {
        CheckMenuRadioItem(actions.raw(), 1, 4, s.selected_action as u32, MF_BYCOMMAND);
    } else {
        for id in 1..=4 {
            check_menu(actions.raw(), id, false);
        }
    }
    check_menu(actions.raw(), 5, s.chase);
    if !append_submenu(root.raw(), actions, "Поведение") {
        return None;
    }

    let friends = MenuGuard::popup()?;
    let together = MenuGuard::popup()?;
    for (id, label) in [
        (90, "Выбрать вместе"),
        (91, "Поздороваться"),
        (92, "Дать пять"),
        (93, "Потанцевать вместе"),
        (94, "Посидеть за ноутбуком"),
        (95, "Подбодрить друга"),
        (96, "Поиграть в догонялки"),
        (97, "Построить башню всем вместе"),
        (98, "Поиграть в чехарду"),
        (99, "Устроить дружескую потасовку"),
    ] {
        if !append_item(together.raw(), id, label, 0) {
            return None;
        }
    }
    if !append_item(friends.raw(), 20, "Позвать Claude", 0)
        || !append_submenu(friends.raw(), together, "Вместе с другом")
        || !append_item(friends.raw(), 45, "Взаимодействовать с друзьями", 0)
    {
        return None;
    }
    check_menu(friends.raw(), 45, s.social_enabled);
    if !append_item(friends.raw(), 30, "Убрать этого питомца", 0)
        || !append_submenu(root.raw(), friends, "Друзья")
    {
        return None;
    }

    let settings = MenuGuard::popup()?;
    if !append_item(settings.raw(), 49, "Открыть настройки…", 0)
        || AppendMenuW(settings.raw(), MF_SEPARATOR, 0, null()) == 0
    {
        return None;
    }
    for (id, label, checked) in [
        (40, "Пауза", s.paused),
        (41, "Скрывать в полноэкранном режиме", s.hide_fullscreen),
        (42, "Запускать вместе с Windows", s.autostart),
        (43, "Показывать реплики", s.show_bubbles),
        (44, "Показывать настроение", s.show_mood),
        (46, "Случайные занятия", s.auto_activities),
        (47, "Реагировать на пользователя", s.auto_reactions),
        (48, "Прыгать на значки рабочего стола", s.desktop_perches),
    ] {
        if !append_item(settings.raw(), id, label, 0) {
            return None;
        }
        check_menu(settings.raw(), id, checked);
    }
    let size = MenuGuard::popup()?;
    for (id, label, checked) in [
        (50, "Маленький", s.scale < 0.9),
        (51, "Обычный", (0.9..=1.25).contains(&s.scale)),
        (52, "Большой", s.scale > 1.25),
    ] {
        if !append_item(size.raw(), id, label, 0) {
            return None;
        }
        check_menu(size.raw(), id, checked);
    }
    CheckMenuRadioItem(
        size.raw(),
        50,
        52,
        if s.scale < 0.9 {
            50
        } else if s.scale > 1.25 {
            52
        } else {
            51
        },
        MF_BYCOMMAND,
    );
    if !append_submenu(settings.raw(), size, "Размер") {
        return None;
    }
    let palette = MenuGuard::popup()?;
    for &(id, label, color) in crate::palette::PRESETS {
        if !append_item(palette.raw(), id, label, 0) {
            return None;
        }
        check_menu(palette.raw(), id, s.body_color == color);
    }
    if !append_item(palette.raw(), 188, "Свой цвет…", 0)
        || !append_submenu(settings.raw(), palette, "Цвет этого Claude")
    {
        return None;
    }

    let spacing = MenuGuard::popup()?;
    for (id, min, max, label) in [
        (85, 12.0, 48.0, "Поближе: 12–48 px"),
        (86, 24.0, 96.0, "Обычно: 24–96 px"),
        (87, 48.0, 144.0, "Свободнее: 48–144 px"),
    ] {
        if !append_item(spacing.raw(), id, label, 0) {
            return None;
        }
        check_menu(
            spacing.raw(),
            id,
            s.personal_space_min == min && s.personal_space_max == max,
        );
    }
    if !append_item(
        spacing.raw(),
        0,
        &format!(
            "Сейчас: {:.0}–{:.0} px между краями",
            s.personal_space_min, s.personal_space_max
        ),
        MF_GRAYED,
    ) || !append_submenu(settings.raw(), spacing, "Личное пространство")
    {
        return None;
    }

    let fps = MenuGuard::popup()?;
    for &(id, value, label) in FPS_ITEMS {
        if !append_item(fps.raw(), id, label, 0) {
            return None;
        }
        check_menu(fps.raw(), id, s.target_fps == value);
    }
    if !FPS_ITEMS.iter().any(|&(_, value, _)| value == s.target_fps)
        && !append_item(
            fps.raw(),
            0,
            &format!("{} кадров/с (текущий режим)", s.target_fps),
            MF_GRAYED,
        )
    {
        return None;
    }
    if let Some(&(id, _, _)) = FPS_ITEMS
        .iter()
        .find(|&&(_, value, _)| value == s.target_fps)
    {
        CheckMenuRadioItem(fps.raw(), 80, 84, id as u32, MF_BYCOMMAND);
    }
    if !append_submenu(settings.raw(), fps, "Частота кадров")
        || AppendMenuW(settings.raw(), MF_SEPARATOR, 0, null()) == 0
        || !append_item(settings.raw(), 60, "Проверить обновления", 0)
        || !append_item(settings.raw(), 61, "Автопроверка обновлений", 0)
    {
        return None;
    }
    check_menu(settings.raw(), 61, s.auto_updates);
    if !append_submenu(root.raw(), settings, "Настройки")
        || AppendMenuW(root.raw(), MF_SEPARATOR, 0, null()) == 0
        || !append_item(root.raw(), 31, "Выход", 0)
    {
        return None;
    }
    Some(root)
}

unsafe fn check_menu(menu: HMENU, id: u16, checked: bool) {
    CheckMenuItem(
        menu,
        id as u32,
        MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
    );
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = lparam as *const CREATESTRUCTW;
        // SAFETY: Windows sends WM_NCCREATE synchronously from CreateWindowExW with
        // its CREATESTRUCTW pointer in LPARAM; lpCreateParams is our still-live Box.
        if !cs.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
        }
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HostState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    // SAFETY: NativeHost keeps HostState in a Box at a stable address until after
    // DestroyWindow returns in Drop. The callback only mutates its Mutex event queue.
    let state = &*ptr;
    if msg == taskbar_created_message() {
        if state.tray && !state.tray_icon.is_null() {
            add_tray_icon(state);
        }
        return 0;
    }
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN => {
            state.dragging.store(true, Ordering::Relaxed);
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::PointerDown(cursor_point()));
            SetCapture(hwnd);
            0
        }
        WM_MOUSEMOVE if GetCapture() == hwnd => {
            enqueue_pointer_move(
                &mut state.events.lock().unwrap_or_else(|e| e.into_inner()),
                cursor_point(),
            );
            0
        }
        WM_LBUTTONUP => {
            if state.dragging.swap(false, Ordering::Relaxed) {
                state
                    .events
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(InputEvent::PointerUp(cursor_point()));
            }
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            0
        }
        WM_CANCELMODE | WM_CAPTURECHANGED => {
            if state.dragging.swap(false, Ordering::Relaxed) {
                state
                    .events
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(InputEvent::PointerUp(cursor_point()));
            }
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::DoubleClick);
            0
        }
        WM_CONTEXTMENU => {
            show_menu(hwnd, state);
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as u16;
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::Command(id));
            0
        }
        WM_CLOSE => {
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::Quit);
            0
        }
        TRAY_MESSAGE => {
            if lparam as u32 == WM_RBUTTONUP
                || lparam as u32 == WM_CONTEXTMENU
                || lparam as u32 == WM_LBUTTONUP
            {
                show_menu(hwnd, state);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn taskbar_created_message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) })
}

unsafe fn add_tray_icon(state: &HostState) {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = state.hwnd;
    data.uID = state.tray_id;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY_MESSAGE;
    data.hIcon = state.tray_icon;
    let tip = wide("Клод — питомец рабочего стола");
    let n = tip.len().min(data.szTip.len());
    data.szTip[..n].copy_from_slice(&tip[..n]);
    Shell_NotifyIconW(NIM_ADD, &data);
}

fn show_menu(hwnd: HWND, state: &HostState) {
    unsafe {
        let s = *state.menu_state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(menu) = build_menu_tree(s) else {
            return;
        };
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu.raw(),
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            p.x,
            p.y,
            0,
            hwnd,
            null(),
        );
        if chosen != 0 {
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::Command(chosen as u16));
        }
        // Required for notification-area menus so Explorer dismisses the menu
        // after TrackPopupMenu returns.
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
}

pub fn snapshot() -> World {
    unsafe {
        let mut world = World::default();
        world.windows.clear();
        world.work_areas.clear();
        let mut monitors: Vec<Monitor> = Vec::new();
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(enum_monitor),
            (&mut monitors as *mut Vec<Monitor>) as LPARAM,
        );
        world.work_areas = monitors
            .iter()
            .map(|m| {
                Rect::new(
                    m.work.left as f64,
                    m.work.top as f64,
                    (m.work.right - m.work.left) as f64,
                    (m.work.bottom - m.work.top) as f64,
                )
            })
            .collect();
        if world.work_areas.is_empty() {
            world.work_areas.push(Rect::new(0.0, 0.0, 1920.0, 1040.0));
        }
        let mut cursor = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut cursor) != 0 {
            world.cursor = Vec2::new(cursor.x as f64, cursor.y as f64);
        }
        let mut last: LASTINPUTINFO = zeroed();
        last.cbSize = size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(&mut last) != 0 {
            world.idle_seconds = (GetTickCount().wrapping_sub(last.dwTime) as f64) / 1000.0;
        }
        let mut list: Vec<WindowInfo> = Vec::new();
        EnumWindows(
            Some(enum_window),
            (&mut list as *mut Vec<WindowInfo>) as LPARAM,
        );
        list.reverse();
        world.windows = list;
        world.video_window = None;
        world
    }
}

struct Monitor {
    work: RECT,
    full: RECT,
}
unsafe extern "system" fn enum_monitor(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    // SAFETY: EnumDisplayMonitors invokes this callback synchronously with the
    // address of the caller-owned Vec<Monitor> supplied as dwData.
    if GetMonitorInfoW(h, &mut info) != 0 {
        (*(data as *mut Vec<Monitor>)).push(Monitor {
            work: info.rcWork,
            full: info.rcMonitor,
        });
    }
    1
}
unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
    // SAFETY: EnumWindows invokes this callback synchronously with the address of
    // the caller-owned Vec<WindowInfo> supplied as lParam.
    let vec = &mut *(data as *mut Vec<WindowInfo>);
    if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
        return 1;
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == GetCurrentProcessId() {
        return 1;
    }
    if !GetWindow(hwnd, GW_OWNER).is_null() {
        return 1;
    }
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    let mut class_buf = [0u16; 128];
    let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
    let class = String::from_utf16_lossy(&class_buf[..class_len.max(0) as usize]);
    let is_file_manager = matches!(class.as_str(), "CabinetWClass" | "ExploreWClass");
    if matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    ) {
        return 1;
    }
    let mut cloaked = 0u32;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED as u32,
        &mut cloaked as *mut _ as *mut c_void,
        size_of::<u32>() as u32,
    ) == S_OK
        && cloaked != 0
    {
        return 1;
    }
    let mut rect: RECT = zeroed();
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS as u32,
        &mut rect as *mut _ as *mut c_void,
        size_of::<RECT>() as u32,
    ) != S_OK
        && GetWindowRect(hwnd, &mut rect) == 0
    {
        return 1;
    }
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return 1;
    }
    let len = GetWindowTextLengthW(hwnd);
    let mut title_buf = vec![0u16; (len.max(0) as usize) + 1];
    GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
    let title = String::from_utf16_lossy(
        &title_buf[..title_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(title_buf.len())],
    );
    let active = GetForegroundWindow() == hwnd;
    let mut fullscreen = false;
    if active && unsafe { IsZoomed(hwnd) } == 0 {
        let mut mons = Vec::<Monitor>::new();
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(enum_monitor),
            (&mut mons as *mut Vec<Monitor>) as LPARAM,
        );
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
        // A maximized window may cover the monitor when taskbar auto-hide is
        // enabled. Treat only borderless popup windows as fullscreen candidates.
        let borderless =
            style & (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) == 0;
        fullscreen = borderless
            && mons.iter().any(|m| {
                (rect.left - m.full.left).abs() <= 2
                    && (rect.top - m.full.top).abs() <= 2
                    && (rect.right - m.full.right).abs() <= 2
                    && (rect.bottom - m.full.bottom).abs() <= 2
            });
    }
    vec.push(WindowInfo {
        id: hwnd as usize,
        rect: Rect::new(
            rect.left as f64,
            rect.top as f64,
            (rect.right - rect.left) as f64,
            (rect.bottom - rect.top) as f64,
        ),
        title,
        active,
        fullscreen,
        is_file_manager,
        is_desktop_icon: false,
    });
    1
}

fn ensure_class() -> Result<(), String> {
    CLASS
        .get_or_init(|| unsafe {
            let hinst = GetModuleHandleW(null());
            if hinst.is_null() {
                return Err(last_error("GetModuleHandleW"));
            }
            let wc = WNDCLASSW {
                style: CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: CLASS_NAME.as_ptr(),
            };
            if RegisterClassW(&wc) == 0 {
                let e = GetLastError();
                if e != ERROR_CLASS_ALREADY_EXISTS {
                    return Err(format!("RegisterClassW failed ({e})"));
                }
            }
            Ok(())
        })
        .clone()
}
fn cursor_point() -> Vec2 {
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) != 0 {
            Vec2::new(p.x as f64, p.y as f64)
        } else {
            Vec2::default()
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
fn last_error(api: &str) -> String {
    format!("{api} failed ({})", unsafe { GetLastError() })
}

#[cfg(test)]
mod tests {
    use super::{change_visibility, enqueue_pointer_move, wait_millis_ceil, InputEvent};
    use crate::Vec2;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn captured_pointer_move_flood_is_coalesced_without_crossing_event_barriers() {
        let mut events = vec![InputEvent::PointerDown(Vec2::new(1.0, 2.0))];
        for n in 0..100_000 {
            enqueue_pointer_move(&mut events, Vec2::new(n as f64, 2.0));
        }
        assert_eq!(events.len(), 2, "a move flood should occupy one queue slot");
        assert_eq!(
            events[1],
            InputEvent::PointerMove(Vec2::new(99_999.0, 2.0)),
            "the latest cursor point must be retained"
        );

        events.push(InputEvent::PointerUp(Vec2::new(100_000.0, 2.0)));
        enqueue_pointer_move(&mut events, Vec2::new(100_001.0, 2.0));
        assert_eq!(events.len(), 4, "up events must remain ordering barriers");
        assert_eq!(events[2], InputEvent::PointerUp(Vec2::new(100_000.0, 2.0)));
        assert_eq!(
            events[3],
            InputEvent::PointerMove(Vec2::new(100_001.0, 2.0))
        );
    }

    #[cfg(windows)]
    #[test]
    fn pump_returns_after_budget_with_thread_queue_messages_remaining() {
        use super::*;
        use std::sync::mpsc;
        use std::thread;

        let (thread_id_tx, thread_id_rx) = mpsc::channel();
        let (run_tx, run_rx) = mpsc::channel();
        let worker = thread::spawn(move || unsafe {
            let mut message: MSG = zeroed();
            // Force creation of this test thread's private message queue before
            // the parent thread posts any messages to it.
            PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
            thread_id_tx
                .send(windows_sys::Win32::System::Threading::GetCurrentThreadId())
                .unwrap();
            run_rx.recv().unwrap();

            let running = NativeHost::pump();
            let has_remaining = PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) != 0;
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {}
            (running, has_remaining)
        });

        let thread_id = thread_id_rx.recv().unwrap();
        for _ in 0..512 {
            assert_ne!(
                unsafe { PostThreadMessageW(thread_id, WM_NULL, 0, 0) },
                0,
                "posting a message to the test thread should succeed"
            );
        }
        run_tx.send(()).unwrap();
        let (running, has_remaining) = worker.join().unwrap();
        assert!(running);
        assert!(
            has_remaining,
            "one pump call must leave queued messages for the next frame"
        );
    }

    #[test]
    fn visibility_calls_are_needed_only_when_state_changes() {
        let visible = AtomicBool::new(false);
        assert!(!change_visibility(&visible, false));
        assert!(change_visibility(&visible, true));
        assert!(!change_visibility(&visible, true));
        assert!(change_visibility(&visible, false));
    }

    #[test]
    fn wait_fallback_rounds_positive_submillisecond_remainders_up() {
        assert_eq!(wait_millis_ceil(Duration::from_nanos(1)), 1);
        assert_eq!(wait_millis_ceil(Duration::from_millis(1)), 1);
        assert_eq!(wait_millis_ceil(Duration::from_micros(1_001)), 2);
        assert_eq!(wait_millis_ceil(Duration::MAX), u32::MAX);
    }

    #[cfg(windows)]
    #[test]
    fn context_menu_builds_claude_actions_and_valid_native_commands() {
        use super::*;

        unsafe fn find_command(menu: HMENU, id: u16) -> Option<HMENU> {
            for index in 0..GetMenuItemCount(menu).max(0) as u32 {
                let item_id = GetMenuItemID(menu, index as i32);
                if item_id == id as u32 {
                    return Some(menu);
                }
                let submenu = GetSubMenu(menu, index as i32);
                if !submenu.is_null() {
                    if let Some(found) = find_command(submenu, id) {
                        return Some(found);
                    }
                }
            }
            None
        }

        unsafe fn command_state(menu: HMENU, id: u16) -> u32 {
            let owner =
                find_command(menu, id).unwrap_or_else(|| panic!("menu command {id} must exist"));
            let state = GetMenuState(owner, id as u32, MF_BYCOMMAND);
            assert_ne!(state, u32::MAX, "native menu command {id} must be valid");
            state
        }

        unsafe {
            let menu = build_menu_tree(MenuState {
                selected_action: 74,
                chase: true,
                target_fps: 120,
                ..MenuState::default()
            })
            .expect("Windows should build the native menu tree");

            for id in [
                1, 2, 3, 4, 5, 20, 30, 31, 40, 41, 42, 43, 44, 50, 51, 52, 60, 61, 70, 71, 72, 73,
                45, 46, 47, 48, 74, 75, 76, 80, 81, 82, 83, 84, 90, 91, 92, 93, 94, 95, 96, 97, 98,
                99,
            ] {
                let _ = command_state(menu.raw(), id);
            }
            for id in [6, 7, 8, 10, 11, 21] {
                assert!(
                    find_command(menu.raw(), id).is_none(),
                    "removed command {id} must not exist"
                );
            }

            assert_eq!(command_state(menu.raw(), 2) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 5) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 74) & MF_CHECKED, 0);
            assert_eq!(command_state(menu.raw(), 75) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 82) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 45) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 46) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 47) & MF_CHECKED, 0);
            assert_ne!(command_state(menu.raw(), 48) & MF_CHECKED, 0);
            for scene in &crate::activities::CATALOG {
                assert_eq!(
                    command_state(menu.raw(), 100 + scene.id.0 as u16) & MF_CHECKED,
                    0
                );
            }
            let selected_scene = build_menu_tree(MenuState {
                selected_action: 121,
                social_enabled: false,
                auto_activities: false,
                ..MenuState::default()
            })
            .unwrap();
            assert_ne!(command_state(selected_scene.raw(), 121) & MF_CHECKED, 0);
            assert_eq!(command_state(selected_scene.raw(), 45) & MF_CHECKED, 0);
            assert_eq!(command_state(selected_scene.raw(), 46) & MF_CHECKED, 0);
        }
    }
}
