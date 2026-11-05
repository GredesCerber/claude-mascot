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
use windows_sys::core::PCWSTR;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: &[u16] = &[
    99, 108, 97, 117, 100, 101, 45, 109, 97, 115, 99, 111, 116, 45, 110, 97, 116, 105, 118, 101, 0,
];
const TRAY_MESSAGE: u32 = WM_APP + 23;
const IDI_APP: PCWSTR = 32512usize as PCWSTR;

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
    pub rage: bool,
    pub tagilla: bool,
    pub scale: f64,
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
            rage: false,
            tagilla: false,
            scale: 1.0,
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
    hwnd: HWND,
    tray: bool,
    tray_id: u32,
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
    pub fn new(title: &str, width: i32, height: i32, tray: bool) -> Result<Self, String> {
        ensure_class()?;
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".into());
        }
        let mut state = Box::new(HostState {
            events: Mutex::new(Vec::new()),
            menu_state: Mutex::new(MenuState::default()),
            dragging: AtomicBool::new(false),
            hwnd: null_mut(),
            tray,
            tray_id: NEXT_TRAY_ID.fetch_add(1, Ordering::Relaxed),
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
        let host = Self {
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

    fn add_tray(&self) -> Result<(), String> {
        let mut data: NOTIFYICONDATAW = unsafe { zeroed() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.state.hwnd;
        data.uID = self.state.tray_id;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = unsafe { LoadIconW(null_mut(), IDI_APP) };
        let tip = wide("Клод — питомец рабочего стола");
        let n = tip.len().min(data.szTip.len());
        data.szTip[..n].copy_from_slice(&tip[..n]);
        if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
            Err(last_error("Shell_NotifyIconW(NIM_ADD)"))
        } else {
            Ok(())
        }
    }

    pub fn pump() -> bool {
        unsafe {
            let mut msg: MSG = zeroed();
            let mut running = true;
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    running = false;
                    break;
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

const MENU_ITEMS: &[(u16, &str)] = &[
    (1, "Гулять"),
    (2, "Сидеть"),
    (3, "Спать"),
    (4, "Карабкаться"),
    (5, "Гоняться"),
    (6, "Крушить"),
    (7, "Ярость"),
    (8, "Переключить ярость"),
    (0, ""),
    (10, "Claude"),
    (11, "Тагилла"),
    (20, "Позвать Claude"),
    (21, "Позвать Тагиллу"),
    (0, ""),
    (30, "Убрать этого питомца"),
    (31, "Закрыть всех питомцев"),
    (0, ""),
    (40, "Пауза"),
    (41, "Скрыть в полноэкранном режиме"),
    (42, "Запускать вместе с Windows"),
    (60, "Проверить обновления"),
    (61, "Автопроверка обновлений"),
    (0, ""),
    (50, "Маленький размер"),
    (51, "Обычный размер"),
    (52, "Большой размер"),
];

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
            state
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(InputEvent::PointerMove(cursor_point()));
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

fn show_menu(hwnd: HWND, state: &HostState) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        for &(id, label) in MENU_ITEMS {
            let appended = if id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, null())
            } else {
                let label = wide(label);
                AppendMenuW(menu, MF_STRING, id as usize, label.as_ptr())
            };
            if appended == 0 {
                DestroyMenu(menu);
                return;
            }
        }
        let s = *state.menu_state.lock().unwrap_or_else(|e| e.into_inner());
        let check = |id: u16, selected: bool| {
            CheckMenuItem(
                menu,
                id as u32,
                MF_BYCOMMAND | if selected { MF_CHECKED } else { MF_UNCHECKED },
            );
        };
        for id in 1..=7 {
            check(id, s.selected_action == id || (id == 5 && s.chase));
        }
        check(8, s.rage);
        check(10, !s.tagilla);
        check(11, s.tagilla);
        check(40, s.paused);
        check(41, s.hide_fullscreen);
        check(42, s.autostart);
        check(61, s.auto_updates);
        check(50, s.scale < 0.9);
        check(51, (0.9..=1.25).contains(&s.scale));
        check(52, s.scale > 1.25);
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
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
        DestroyMenu(menu);
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
    if active {
        let mut mons = Vec::<Monitor>::new();
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(enum_monitor),
            (&mut mons as *mut Vec<Monitor>) as LPARAM,
        );
        fullscreen = mons.iter().any(|m| {
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
