//! Modeless native preferences window. It owns only controls and a small event
//! queue; the application frame loop remains the sole message pump.
use crate::activities::CATALOG;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    OnceLock,
};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetActiveWindow, GetFocus, SetFocus,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS: &[u16] = &[
    67, 108, 97, 117, 100, 101, 77, 97, 115, 99, 111, 116, 80, 114, 101, 102, 115, 0,
];
const TITLE: &[u16] = &[
    67, 108, 97, 117, 100, 101, 32, 77, 97, 115, 99, 111, 116, 32, 8212, 32, 1053, 1072, 1089,
    1090, 1088, 1086, 1081, 1082, 1080, 0,
];
const ID_NAV_BASE: i32 = 3000;
const FIXED: usize = usize::MAX;
const ID_PET: i32 = 100;
const ID_FPS: i32 = 101;
const ID_SCALE: i32 = 102;
const ID_INTENSITY: i32 = 103;
const ID_MIN_GAP: i32 = 104;
const ID_MAX_GAP: i32 = 105;
const ID_MOOD: i32 = 110;
const ID_BUBBLES: i32 = 111;
const ID_REACTIONS: i32 = 112;
const ID_ACTIVITIES: i32 = 113;
const ID_SOCIAL: i32 = 114;
const ID_DESKTOP: i32 = 115;
const ID_PAUSED: i32 = 116;
const ID_AUTO_UPDATES: i32 = 117;
const ID_AUTO_INSTALL: i32 = 118;
const ID_CHOOSE_COLOR: i32 = 120;
const ID_COLOR_BUTTON: i32 = 140;
const ID_RESET_COLOR: i32 = 121;
const ID_ACTIVITY: i32 = 130;
const ID_RUN_ACTIVITY: i32 = 131;
const ID_ACTIVITY_SEARCH: i32 = 132;
const ID_PAUSE_ALL: i32 = 3100;
const ID_QUICK_BASE: i32 = 140;
const DARK_RGB: u32 = 0x252220;
const PANEL_RGB: u32 = 0x302B27;
const CREAM_RGB: u32 = 0xF3EDE2;
const ACCENT_RGB: u32 = 0xD97757;
static PREFERENCES_HWND: AtomicIsize = AtomicIsize::new(0);
const DEFAULT_CHARSET: u32 = 1;
const OUT_DEFAULT_PRECIS: u32 = 0;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const DEFAULT_QUALITY: u32 = 0;
const DEFAULT_PITCH: u32 = 0;
const FW_NORMAL: i32 = 400;

#[derive(Clone, Debug, PartialEq)]
pub struct PreferencesState {
    pub selected_pet_id: u64,
    pub pet_ids: Vec<u64>,
    /// RGB in the low 24 bits, matching the application's color setting.
    pub body_color: u32,
    pub fps: u32,
    pub scale: f64,
    pub intensity: u8,
    pub min_gap: f64,
    pub max_gap: f64,
    pub show_mood: bool,
    pub show_bubbles: bool,
    pub auto_reactions: bool,
    pub auto_activities: bool,
    pub social_enabled: bool,
    pub desktop_perches: bool,
    pub paused: bool,
    pub auto_updates: bool,
    pub auto_install_updates: bool,
}
impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            selected_pet_id: 1,
            pet_ids: vec![1],
            body_color: 0xD97757,
            fps: 60,
            scale: 1.0,
            intensity: 50,
            min_gap: 24.0,
            max_gap: 96.0,
            show_mood: true,
            show_bubbles: true,
            auto_reactions: true,
            auto_activities: true,
            social_enabled: true,
            desktop_perches: true,
            paused: false,
            auto_updates: true,
            auto_install_updates: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PreferencesEvent {
    SelectPet(u64),
    SetFps(u32),
    SetScale(f64),
    SetIntensity(u8),
    SetSpacing(f64, f64),
    ToggleMood(bool),
    ToggleBubbles(bool),
    ToggleReactions(bool),
    ToggleActivities(bool),
    ToggleSocial(bool),
    ToggleDesktop(bool),
    TogglePaused(bool),
    ToggleAutoUpdates(bool),
    ToggleAutoInstall(bool),
    ChooseColor,
    ResetColor,
    RunAction(u16),
}

#[derive(Clone, Copy)]
struct Child {
    page: usize,
    hwnd: HWND,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
struct WindowState {
    page: usize,
    build_page: usize,
    page_heights: [i32; 7],
    page_scroll: [i32; 7],
    scope_label: HWND,
    activity_items: Vec<usize>,
    activity_status: HWND,
    spacing_status: HWND,
    friends_status: HWND,
    hwnd: HWND,
    children: Vec<Child>,
    events: Vec<PreferencesEvent>,
    data: PreferencesState,
    font: HFONT,
    bold_font: HFONT,
    dark_brush: HBRUSH,
    panel_brush: HBRUSH,
    accent_brush: HBRUSH,
    section_headers: Vec<HWND>,
    color_brush: HBRUSH,
    small_icon: HICON,
    big_icon: HICON,
    color_value: u32,
    dpi: f64,
    scroll: i32,
    content_height: i32,
    client_height: i32,
    pet_combo: HWND,
    pet_items: Vec<u64>,
    fps_combo: HWND,
    scale_combo: HWND,
    intensity: HWND,
    intensity_value: HWND,
    min_edit: HWND,
    max_edit: HWND,
    activity_combo: HWND,
    color_swatch: HWND,
    built: bool,
}
impl WindowState {
    fn new() -> Self {
        Self {
            page: 0,
            build_page: FIXED,
            page_heights: [0; 7],
            page_scroll: [0; 7],
            scope_label: null_mut(),
            activity_items: Vec::new(),
            activity_status: null_mut(),
            spacing_status: null_mut(),
            friends_status: null_mut(),
            hwnd: null_mut(),
            children: Vec::new(),
            events: Vec::new(),
            data: PreferencesState::default(),
            font: null_mut(),
            bold_font: null_mut(),
            dark_brush: null_mut(),
            panel_brush: null_mut(),
            accent_brush: null_mut(),
            section_headers: Vec::new(),
            color_brush: null_mut(),
            small_icon: null_mut(),
            big_icon: null_mut(),
            color_value: u32::MAX,
            dpi: 1.0,
            scroll: 0,
            content_height: 0,
            client_height: 0,
            pet_combo: null_mut(),
            pet_items: Vec::new(),
            fps_combo: null_mut(),
            scale_combo: null_mut(),
            intensity: null_mut(),
            intensity_value: null_mut(),
            min_edit: null_mut(),
            max_edit: null_mut(),
            activity_combo: null_mut(),
            color_swatch: null_mut(),
            built: false,
        }
    }
}

pub struct PreferencesWindow {
    hwnd: HWND,
    state: Box<WindowState>,
}
impl PreferencesWindow {
    pub fn new() -> Result<Self, String> {
        ensure_class()?;
        let mut state = Box::new(WindowState::new());
        let ptr = (&mut *state) as *mut WindowState;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW | WS_EX_CONTROLPARENT,
                CLASS.as_ptr(),
                TITLE.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                900,
                820,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                ptr.cast::<c_void>(),
            )
        };
        if hwnd.is_null() {
            return Err(last_error("CreateWindowExW preferences"));
        }
        state.hwnd = hwnd;
        state.dpi = unsafe { GetDpiForWindow(hwnd) as f64 / 96.0 }.max(1.0);
        state.font = unsafe { make_font(state.dpi, FW_NORMAL) };
        state.bold_font = unsafe { make_font(state.dpi, FW_BOLD as i32) };
        state.dark_brush = unsafe { CreateSolidBrush(colorref(DARK_RGB)) };
        state.panel_brush = unsafe { CreateSolidBrush(colorref(PANEL_RGB)) };
        state.accent_brush = unsafe { CreateSolidBrush(colorref(ACCENT_RGB)) };
        if state.font.is_null()
            || state.bold_font.is_null()
            || state.dark_brush.is_null()
            || state.panel_brush.is_null()
            || state.accent_brush.is_null()
        {
            unsafe {
                DestroyWindow(hwnd);
                release_gdi_resources(&state);
            }
            return Err(last_error("CreateFontW/CreateSolidBrush preferences"));
        }
        unsafe {
            state.small_icon = crate::platform::application_icon(scale(16, state.dpi));
            state.big_icon = crate::platform::application_icon(scale(32, state.dpi));
        }
        if state.small_icon.is_null() || state.big_icon.is_null() {
            unsafe {
                DestroyWindow(hwnd);
                release_gdi_resources(&state);
            }
            return Err("Cannot load embedded Claude window icons".into());
        }
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETICON,
                ICON_SMALL as usize,
                state.small_icon as LPARAM,
            );
            SendMessageW(
                hwnd,
                WM_SETICON,
                ICON_BIG as usize,
                state.big_icon as LPARAM,
            );
        }
        if let Err(error) = unsafe { build_controls(&mut state) } {
            unsafe {
                DestroyWindow(hwnd);
                release_gdi_resources(&state);
            }
            return Err(error);
        }
        PREFERENCES_HWND.store(hwnd as isize, Ordering::Release);
        let mut window = Self { hwnd, state };
        window.resize_to_work_area();
        Ok(window)
    }
    /// Show or reopen the existing modeless window and synchronize values.
    pub fn show(&mut self, state: PreferencesState) {
        self.state.data = state;
        unsafe {
            sync_controls(&mut self.state, true);
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
            // A process launched with STARTUPINFO.wShowWindow=SW_HIDE can have
            // its first normal-show request suppressed. Follow with SW_SHOW.
            ShowWindow(self.hwnd, SW_SHOW);
            SetForegroundWindow(self.hwnd);
        }
    }
    /// Drain actions emitted by native controls since the previous frame.
    pub fn poll(&mut self) -> Vec<PreferencesEvent> {
        std::mem::take(&mut self.state.events)
    }
    /// Update values while preserving actively edited spacing fields.
    pub fn sync(&mut self, state: PreferencesState) {
        self.state.data = state;
        unsafe {
            sync_controls(&mut self.state, false);
        }
    }
    pub fn hide(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }
    fn resize_to_work_area(&mut self) {
        unsafe {
            let mut area = RECT {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 900,
            };
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut area as *mut RECT).cast(), 0);
            let avail_w = (area.right - area.left - scale(48, self.state.dpi))
                .max(scale(120, self.state.dpi));
            let avail_h = (area.bottom - area.top - scale(48, self.state.dpi))
                .max(scale(100, self.state.dpi));
            let width = scale(850, self.state.dpi).min(avail_w);
            let height = scale(640, self.state.dpi).min(avail_h);
            SetWindowPos(
                self.hwnd,
                null_mut(),
                area.left + ((area.right - area.left - width) / 2),
                area.top + ((area.bottom - area.top - height) / 2),
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
impl Drop for PreferencesWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
            }
            let _ = PREFERENCES_HWND.compare_exchange(
                self.hwnd as isize,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            release_gdi_resources(&self.state);
        }
    }
}

fn ensure_class() -> Result<(), String> {
    static CLASS_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    CLASS_RESULT
        .get_or_init(|| unsafe {
            let init = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_BAR_CLASSES,
            };
            if InitCommonControlsEx(&init) == 0 {
                return Err(last_error("InitCommonControlsEx"));
            }
            let instance = GetModuleHandleW(null());
            if instance.is_null() {
                return Err(last_error("GetModuleHandleW"));
            }
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: CLASS.as_ptr(),
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(last_error("RegisterClassW preferences"));
            }
            Ok(())
        })
        .clone()
}

unsafe fn make_font(dpi: f64, weight: i32) -> HFONT {
    let face: Vec<u16> = "Segoe UI"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    CreateFontW(
        -scale(14, dpi),
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        DEFAULT_PITCH,
        face.as_ptr(),
    )
}
fn scale(value: i32, dpi: f64) -> i32 {
    (value as f64 * dpi).round() as i32
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn last_error(api: &str) -> String {
    format!("{api} failed ({})", unsafe { GetLastError() })
}
unsafe fn release_gdi_resources(s: &WindowState) {
    for icon in [s.small_icon, s.big_icon] {
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
    for object in [
        s.font as HGDIOBJ,
        s.bold_font as HGDIOBJ,
        s.dark_brush as HGDIOBJ,
        s.panel_brush as HGDIOBJ,
        s.accent_brush as HGDIOBJ,
        s.color_brush as HGDIOBJ,
    ] {
        if !object.is_null() {
            DeleteObject(object);
        }
    }
}

/// Let the app's existing pump handle Tab and Shift+Tab navigation for this dialog-like window.
pub fn preprocess_message(msg: &MSG) -> bool {
    let hwnd = PREFERENCES_HWND.load(Ordering::Acquire) as HWND;
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        IsWindowVisible(hwnd) != 0 && GetActiveWindow() == hwnd && IsDialogMessageW(hwnd, msg) != 0
    }
}

// Mirrors Win32 child-control creation with explicit layout coordinates.
#[allow(clippy::too_many_arguments)]
unsafe fn add_child(
    s: &mut WindowState,
    class: &str,
    text: &str,
    id: i32,
    style: u32,
    ex: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    let class = wide(class);
    let text = wide(text);
    let hwnd = CreateWindowExW(
        ex,
        class.as_ptr(),
        text.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        scale(x + if s.build_page == FIXED { 0 } else { 200 }, s.dpi),
        scale(y, s.dpi),
        scale(w, s.dpi),
        scale(h, s.dpi),
        s.hwnd,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null_mut(),
    );
    if !hwnd.is_null() {
        SendMessageW(hwnd, WM_SETFONT, s.font as usize, 1);
        if s.build_page != FIXED {
            s.page_heights[s.build_page] = s.page_heights[s.build_page].max(scale(
                y + if class == wide("COMBOBOX") { 40 } else { h } + 24,
                s.dpi,
            ));
        }
        s.children.push(Child {
            page: s.build_page,
            hwnd,
            x: scale(x + if s.build_page == FIXED { 0 } else { 200 }, s.dpi),
            y: scale(y, s.dpi),
            w: scale(w, s.dpi),
            h: scale(h, s.dpi),
        });
    }
    hwnd
}
unsafe fn label(s: &mut WindowState, text: &str, x: i32, y: i32, w: i32) -> Result<(), String> {
    if add_child(s, "STATIC", text, 0, 0, 0, x, y, w, 22).is_null() {
        Err(last_error("CreateWindowExW label"))
    } else {
        Ok(())
    }
}
unsafe fn heading(s: &mut WindowState, text: &str, x: i32, y: i32, w: i32) -> Result<(), String> {
    let hwnd = add_child(s, "STATIC", text, 0, 0, 0, x, y, w, 24);
    if hwnd.is_null() {
        return Err(last_error("CreateWindowExW section header"));
    }
    SendMessageW(hwnd, WM_SETFONT, s.bold_font as usize, 1);
    s.section_headers.push(hwnd);
    Ok(())
}
unsafe fn button(
    s: &mut WindowState,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
) -> Result<HWND, String> {
    let h = add_child(
        s,
        "BUTTON",
        text,
        id,
        BS_OWNERDRAW as u32 | BS_NOTIFY as u32 | WS_TABSTOP,
        0,
        x,
        y,
        w,
        30,
    );
    if h.is_null() {
        Err(last_error("CreateWindowExW button"))
    } else {
        Ok(h)
    }
}
unsafe fn checkbox(s: &mut WindowState, text: &str, id: i32, x: i32, y: i32) -> Result<(), String> {
    if add_child(
        s,
        "BUTTON",
        text,
        id,
        BS_AUTOCHECKBOX as u32 | BS_NOTIFY as u32 | WS_TABSTOP,
        0,
        x,
        y,
        560,
        28,
    )
    .is_null()
    {
        Err(last_error("CreateWindowExW checkbox"))
    } else {
        Ok(())
    }
}
unsafe fn combo(s: &mut WindowState, id: i32, x: i32, y: i32, w: i32) -> Result<HWND, String> {
    // Keep insertion order stable: combo sorting would break FPS/activity IDs.
    let h = add_child(
        s,
        "COMBOBOX",
        "",
        id,
        CBS_DROPDOWNLIST as u32 | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        x,
        y,
        w,
        250,
    );
    if h.is_null() {
        Err(last_error("CreateWindowExW combo"))
    } else {
        Ok(h)
    }
}
unsafe fn combo_item(hwnd: HWND, text: &str) {
    let text = wide(text);
    SendMessageW(hwnd, CB_ADDSTRING, 0, text.as_ptr() as LPARAM);
}

unsafe fn build_controls(s: &mut WindowState) -> Result<(), String> {
    s.build_page = FIXED;
    heading(s, "Claude Mascot", 20, 18, 200)?;
    label(s, "Питомец", 210, 22, 100)?;
    s.pet_combo = combo(s, ID_PET, 320, 18, 300)?;
    button(s, "Пауза всем", ID_PAUSE_ALL, 650, 18, 150)?;
    s.scope_label = add_child(s, "STATIC", "", 0, 0, 0, 200, 62, 600, 22);
    for (index, title) in [
        "Внешний вид",
        "Поведение",
        "Опоры",
        "Друзья",
        "Производительность",
        "Занятия",
        "Обновления",
    ]
    .iter()
    .enumerate()
    {
        button(
            s,
            title,
            ID_NAV_BASE + index as i32,
            14,
            108 + index as i32 * 42,
            174,
        )?;
    }
    s.build_page = 0;
    let mut y = 112;
    heading(s, "Внешний вид", 20, y, 400)?;
    y += 38;
    label(s, "Цвет тела", 20, y, 150)?;
    s.color_swatch = add_child(
        s,
        "STATIC",
        "",
        ID_CHOOSE_COLOR,
        WS_TABSTOP | WS_BORDER,
        0,
        190,
        y,
        54,
        32,
    );
    if s.color_swatch.is_null() {
        return Err(last_error("CreateWindowExW color swatch"));
    }
    button(s, "Выбрать цвет…", ID_COLOR_BUTTON, 255, y, 150)?;
    button(s, "Сбросить", ID_RESET_COLOR, 435, y, 120)?;
    y += 46;
    label(s, "Масштаб", 20, y, 150)?;
    s.scale_combo = combo(s, ID_SCALE, 190, y, 400)?;
    for item in ["75%", "100%", "150%"] {
        combo_item(s.scale_combo, item);
    }
    s.build_page = 1;
    y = 150;
    heading(s, "Поведение", 20, 112, 400)?;
    label(s, "Интенсивность", 20, y, 120)?;
    s.intensity_value = add_child(s, "STATIC", "50%", 0, 0, 0, 140, y, 40, 22);
    if s.intensity_value.is_null() {
        return Err(last_error("CreateWindowExW intensity label"));
    }
    s.intensity = add_child(
        s,
        "msctls_trackbar32",
        "",
        ID_INTENSITY,
        TBS_AUTOTICKS | TBS_HORZ | WS_TABSTOP,
        0,
        190,
        y,
        400,
        34,
    );
    if s.intensity.is_null() {
        return Err(last_error("CreateWindowExW trackbar"));
    }
    SendMessageW(s.intensity, TBM_SETRANGEMIN, 1, 0);
    SendMessageW(s.intensity, TBM_SETRANGEMAX, 1, 100);
    s.build_page = 4;
    y = 150;
    heading(s, "Производительность", 20, 112, 400)?;
    label(s, "Частота кадров", 20, y, 150)?;
    s.fps_combo = combo(s, ID_FPS, 190, y, 400)?;
    for item in ["30 FPS", "60 FPS", "120 FPS", "144 FPS", "240 FPS"] {
        combo_item(s.fps_combo, item);
    }
    label(
        s,
        "FPS меняет плавность движения, а не скорость питомцев.",
        20,
        200,
        590,
    )?;
    label(
        s,
        "60 — обычный режим; 120–240 — для быстрых мониторов.",
        20,
        232,
        590,
    )?;
    s.build_page = 3;
    y = 112;
    heading(s, "Расстояние между питомцами", 20, y, 400)?;
    y += 30;
    label(s, "Минимальный свободный зазор (px)", 20, y, 300)?;
    s.min_edit = add_child(
        s,
        "EDIT",
        "",
        ID_MIN_GAP,
        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
        WS_EX_CLIENTEDGE,
        360,
        y,
        100,
        28,
    );
    if s.min_edit.is_null() {
        return Err(last_error("CreateWindowExW min edit"));
    }
    y += 36;
    label(s, "Максимальный зазор (px)", 20, y, 300)?;
    s.max_edit = add_child(
        s,
        "EDIT",
        "",
        ID_MAX_GAP,
        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
        WS_EX_CLIENTEDGE,
        360,
        y,
        100,
        28,
    );
    if s.max_edit.is_null() {
        return Err(last_error("CreateWindowExW max edit"));
    }
    s.spacing_status = add_child(
        s,
        "STATIC",
        "8–256 px; максимум до 512 px, на 8 px больше минимума.",
        0,
        0,
        0,
        20,
        216,
        590,
        22,
    );
    s.build_page = 2;
    heading(s, "Окна и ярлыки", 20, 112, 400)?;
    label(
        s,
        "Пет исследует достижимые окна и видимые ярлыки.",
        20,
        150,
        570,
    )?;
    label(
        s,
        "Внутренние элементы интерфейса пока не поддерживаются.",
        20,
        182,
        590,
    )?;
    s.build_page = 1;
    label(
        s,
        "0 — спокойно; 100 — больше самостоятельных занятий",
        190,
        188,
        400,
    )?;
    s.build_page = 6;
    heading(s, "Обновления", 20, 112, 400)?;
    label(
        s,
        &format!("Установлена версия {}", env!("CARGO_PKG_VERSION")),
        20,
        150,
        500,
    )?;
    button(s, "Проверить обновления", ID_QUICK_BASE + 60, 20, 270, 260)?;
    button(s, "Последний результат", ID_QUICK_BASE + 63, 300, 270, 260)?;
    label(
        s,
        "Обычно установка начинается после «Обновить сейчас».",
        20,
        322,
        590,
    )?;
    label(
        s,
        "Включите автоустановку, только если подтверждение не нужно.",
        20,
        354,
        590,
    )?;
    for (id, text) in [
        (ID_MOOD, "Показывать настроение"),
        (ID_BUBBLES, "Показывать реплики"),
        (ID_REACTIONS, "Реакции на рабочий стол"),
        (ID_ACTIVITIES, "Автоматические занятия"),
        (ID_SOCIAL, "Взаимодействия между питомцами"),
        (ID_DESKTOP, "Прыжки на ярлыки"),
        (ID_PAUSED, "Пауза"),
        (ID_AUTO_UPDATES, "Проверять обновления автоматически"),
        (
            ID_AUTO_INSTALL,
            "Устанавливать обновления без подтверждения",
        ),
    ] {
        let (page, top) = match id {
            ID_MOOD => (0, 270),
            ID_BUBBLES => (0, 307),
            ID_REACTIONS => (1, 220),
            ID_ACTIVITIES => (1, 257),
            ID_PAUSED => (1, 294),
            ID_SOCIAL => (3, 250),
            ID_DESKTOP => (2, 230),
            ID_AUTO_UPDATES => (6, 190),
            _ => (6, 227),
        };
        s.build_page = page;
        checkbox(s, text, id, 25, top)?;
    }
    s.build_page = 5;
    y = 112;
    heading(s, "Выбрать занятие", 20, y, 400)?;
    label(s, "Поиск по названию или группе", 20, 150, 570)?;
    let search = add_child(
        s,
        "EDIT",
        "",
        ID_ACTIVITY_SEARCH,
        WS_TABSTOP | ES_AUTOHSCROLL as u32,
        WS_EX_CLIENTEDGE,
        20,
        180,
        570,
        30,
    );
    if search.is_null() {
        return Err(last_error("CreateWindowExW activity search"));
    }
    s.activity_combo = combo(s, ID_ACTIVITY, 20, 228, 570)?;
    button(s, "Запустить", ID_RUN_ACTIVITY, 20, 274, 160)?;
    s.activity_status = add_child(s, "STATIC", "", 0, 0, 0, 200, 280, 390, 22);
    filter_activities(s, "");
    label(
        s,
        "Занятие выполняет выбранный в шапке питомец.",
        20,
        330,
        590,
    )?;
    s.build_page = 3;
    s.friends_status = add_child(s, "STATIC", "", 0, 0, 0, 20, 285, 590, 22);
    y = 324;
    heading(s, "Совместные игры", 20, y, 400)?;
    y += 30;
    for (idx, (text, action)) in [
        ("Приветствие", 91u16),
        ("Дать пять", 92),
        ("Танец", 93),
        ("Общий ноутбук", 94),
        ("Утешить", 95),
        ("Догонялки", 96),
        ("Башня", 97),
        ("Чехарда", 98),
        ("Потасовка", 99),
    ]
    .into_iter()
    .enumerate()
    {
        let col = (idx % 3) as i32;
        let row = (idx / 3) as i32;
        button(
            s,
            text,
            ID_QUICK_BASE + action as i32,
            20 + col * 190,
            y + row * 38,
            175,
        )?;
    }
    s.build_page = 1;
    y = 350;
    heading(s, "Действия питомца", 20, y, 300)?;
    y += 30;
    for (idx, (text, action)) in [
        ("Погулять", 1u16),
        ("Сесть", 2),
        ("Спать", 3),
        ("Залезть", 4),
        ("Бежать за мышкой", 5),
        ("Прыжок", 70),
        ("Погладить", 71),
        ("Танец", 72),
        ("Помахать", 73),
        ("Ноутбук", 74),
        ("Потянуться", 75),
        ("Оглядеться", 76),
    ]
    .into_iter()
    .enumerate()
    {
        let col = (idx % 3) as i32;
        let row = (idx / 3) as i32;
        button(
            s,
            text,
            ID_QUICK_BASE + action as i32,
            20 + col * 190,
            y + row * 38,
            175,
        )?;
    }
    s.content_height = scale(y + row_count(12, 3) * 38 + 24, s.dpi);
    s.built = true;
    select_page(s, 0);
    Ok(())
}
fn row_count(n: usize, columns: usize) -> i32 {
    n.div_ceil(columns) as i32
}

fn activity_matches(index: usize, query: &str) -> bool {
    let spec = &CATALOG[index];
    let text = format!("{} {}", spec.group.label_ru(), spec.title_ru).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|term| text.contains(term))
}
unsafe fn filter_activities(s: &mut WindowState, query: &str) {
    let previous = s
        .activity_items
        .get(SendMessageW(s.activity_combo, CB_GETCURSEL, 0, 0) as usize)
        .copied();
    s.activity_items = (0..CATALOG.len())
        .filter(|&i| activity_matches(i, query))
        .collect();
    SendMessageW(s.activity_combo, CB_RESETCONTENT, 0, 0);
    for &index in &s.activity_items {
        let spec = &CATALOG[index];
        combo_item(
            s.activity_combo,
            &format!("{} · {}", spec.group.label_ru(), spec.title_ru),
        );
    }
    let selected = previous
        .and_then(|id| s.activity_items.iter().position(|&i| i == id))
        .unwrap_or(0);
    SendMessageW(s.activity_combo, CB_SETCURSEL, selected, 0);
    EnableWindow(
        GetDlgItem(s.hwnd, ID_RUN_ACTIVITY),
        i32::from(!s.activity_items.is_empty()),
    );
    set_text(
        s.activity_status,
        &if s.activity_items.is_empty() {
            "Ничего не найдено. Измените запрос.".to_owned()
        } else {
            format!("Найдено: {} из {}", s.activity_items.len(), CATALOG.len())
        },
    );
}

unsafe fn sync_controls(s: &mut WindowState, force: bool) {
    let focused = GetFocus();
    if force || (focused != s.pet_combo && s.pet_items != s.data.pet_ids) {
        SendMessageW(s.pet_combo, CB_RESETCONTENT, 0, 0);
        for id in &s.data.pet_ids {
            combo_item(s.pet_combo, &format!("Клод №{id}"));
        }
        s.pet_items.clone_from(&s.data.pet_ids);
    }
    if force || focused != s.pet_combo {
        let selected = s
            .data
            .pet_ids
            .iter()
            .position(|id| *id == s.data.selected_pet_id)
            .unwrap_or(0);
        SendMessageW(s.pet_combo, CB_SETCURSEL, selected, 0);
    }
    if force || focused != s.fps_combo {
        let fps_index = [30, 60, 120, 144, 240]
            .iter()
            .position(|&fps| fps == s.data.fps)
            .unwrap_or_else(|| {
                if SendMessageW(s.fps_combo, CB_GETCOUNT, 0, 0) > 5 {
                    SendMessageW(s.fps_combo, CB_DELETESTRING, 5, 0);
                }
                combo_item(s.fps_combo, &format!("{} FPS", s.data.fps));
                5
            });
        SendMessageW(s.fps_combo, CB_SETCURSEL, fps_index, 0);
    }
    let scale_index = if (s.data.scale - 0.75).abs() < 0.01 {
        0
    } else if (s.data.scale - 1.5).abs() < 0.01 {
        2
    } else {
        1
    };
    if force || focused != s.scale_combo {
        SendMessageW(s.scale_combo, CB_SETCURSEL, scale_index, 0);
    }
    if force || focused != s.intensity {
        SendMessageW(
            s.intensity,
            TBM_SETPOS,
            1,
            s.data.intensity.min(100) as LPARAM,
        );
    }
    set_text(
        s.intensity_value,
        &format!("{}%", s.data.intensity.min(100)),
    );
    for (id, value) in [
        (ID_MOOD, s.data.show_mood),
        (ID_BUBBLES, s.data.show_bubbles),
        (ID_REACTIONS, s.data.auto_reactions),
        (ID_ACTIVITIES, s.data.auto_activities),
        (ID_SOCIAL, s.data.social_enabled),
        (ID_DESKTOP, s.data.desktop_perches),
        (ID_PAUSED, s.data.paused),
        (ID_AUTO_UPDATES, s.data.auto_updates),
        (ID_AUTO_INSTALL, s.data.auto_install_updates),
    ] {
        let control = GetDlgItem(s.hwnd, id);
        SendMessageW(
            control,
            BM_SETCHECK,
            if value { BST_CHECKED as usize } else { 0 },
            0,
        );
    }
    if force || (focused != s.min_edit && focused != s.max_edit) {
        set_text(s.min_edit, &compact_number(s.data.min_gap));
    }
    if force || (focused != s.min_edit && focused != s.max_edit) {
        set_text(s.max_edit, &compact_number(s.data.max_gap));
    }
    let color = s.data.body_color & 0x00ff_ffff;
    if color != s.color_value {
        if !s.color_brush.is_null() {
            DeleteObject(s.color_brush as HGDIOBJ);
        }
        s.color_brush = CreateSolidBrush(colorref(color));
        s.color_value = color;
    }
    let has_friends = s.data.pet_ids.len() >= 2;
    for action in 91..=99 {
        let button = GetDlgItem(s.hwnd, ID_QUICK_BASE + action);
        let was_enabled = GetWindowLongW(button, GWL_STYLE) as u32 & WS_DISABLED == 0;
        if was_enabled != has_friends {
            EnableWindow(button, i32::from(has_friends));
            InvalidateRect(button, null(), 1);
        }
    }
    set_text(
        s.friends_status,
        if has_friends {
            "Выберите игру: питомцы подойдут к другу, когда будут свободны."
        } else {
            "Для совместных игр добавьте второго питомца через меню пета."
        },
    );
    let pause = GetDlgItem(s.hwnd, ID_PAUSE_ALL);
    let title = if s.data.paused {
        "Продолжить"
    } else {
        "Пауза всем"
    };
    let mut current = [0u16; 64];
    let n = GetWindowTextW(pause, current.as_mut_ptr(), 64);
    if String::from_utf16_lossy(&current[..n.max(0) as usize]) != title {
        set_text(pause, title);
        InvalidateRect(pause, null(), 1);
    }
    InvalidateRect(s.color_swatch, null(), 1);
}
unsafe fn set_text(hwnd: HWND, text: &str) {
    let t = wide(text);
    SetWindowTextW(hwnd, t.as_ptr());
}
fn compact_number(value: f64) -> String {
    if !value.is_finite() {
        return "24".into();
    }
    if value.fract().abs() < 0.001 {
        format!("{:.0}", value)
    } else {
        format!("{value:.1}")
    }
}
fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = lparam as *const CREATESTRUCTW;
        if !cs.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
        }
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let s = &mut *ptr;
    match msg {
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam as *mut MINMAXINFO);
            let mut monitor: MONITORINFO = std::mem::zeroed();
            monitor.cbSize = size_of::<MONITORINFO>() as u32;
            let handle = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if GetMonitorInfoW(handle, &mut monitor) != 0 {
                info.ptMinTrackSize.x =
                    scale(850, s.dpi).min(monitor.rcWork.right - monitor.rcWork.left);
                info.ptMinTrackSize.y =
                    scale(420, s.dpi).min(monitor.rcWork.bottom - monitor.rcWork.top);
            }
            0
        }
        WM_SIZE => {
            s.client_height = (lparam as u32 >> 16) as i32;
            update_scroll(s);
            0
        }
        WM_ERASEBKGND => {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut rect);
            FillRect(wparam as HDC, &rect, s.dark_brush);
            let panel = RECT {
                left: scale(202, s.dpi),
                top: scale(102, s.dpi),
                right: rect.right - scale(12, s.dpi),
                bottom: rect.bottom - scale(18, s.dpi),
            };
            FillRect(wparam as HDC, &panel, s.panel_brush);
            1
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut rect);
            FrameRect(dc, &rect, s.accent_brush);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let line = RECT {
                left: scale(12, s.dpi),
                top: height - scale(5, s.dpi),
                right: width - scale(12, s.dpi),
                bottom: height - scale(2, s.dpi),
            };
            FillRect(dc, &line, s.accent_brush);
            let mid = width / 2;
            for step in 0..4 {
                let inset = scale(step * 3, s.dpi);
                let tail = RECT {
                    left: mid - scale(9, s.dpi) + inset,
                    top: height - scale(9 - step, s.dpi),
                    right: mid + scale(9, s.dpi) - inset,
                    bottom: height - scale(5 - step, s.dpi),
                };
                FillRect(dc, &tail, s.accent_brush);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_VSCROLL => {
            scroll(
                s,
                (wparam & 0xffff) as i32,
                ((wparam >> 16) & 0xffff) as i32,
            );
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16) as i32;
            scroll(s, if delta > 0 { SB_PAGEUP } else { SB_PAGEDOWN }, 0);
            0
        }
        WM_COMMAND => {
            handle_command(s, wparam, lparam);
            0
        }
        WM_HSCROLL if lparam as HWND == s.intensity => {
            let value = SendMessageW(s.intensity, WM_USER, 0, 0).clamp(0, 100) as u8;
            s.data.intensity = value;
            set_text(s.intensity_value, &format!("{value}%"));
            s.events.push(PreferencesEvent::SetIntensity(value));
            0
        }
        WM_CTLCOLORSTATIC => {
            let child = lparam as HWND;
            if child == s.color_swatch && !s.color_brush.is_null() {
                SetBkMode(wparam as HDC, OPAQUE as i32);
                SetBkColor(wparam as HDC, colorref(s.color_value));
                s.color_brush as LRESULT
            } else {
                SetBkMode(wparam as HDC, OPAQUE as i32);
                SetTextColor(
                    wparam as HDC,
                    colorref(if s.section_headers.contains(&child) {
                        ACCENT_RGB
                    } else {
                        CREAM_RGB
                    }),
                );
                let content = s
                    .children
                    .iter()
                    .any(|c| c.hwnd == child && c.page != FIXED);
                SetBkColor(
                    wparam as HDC,
                    colorref(if content { PANEL_RGB } else { DARK_RGB }),
                );
                if content {
                    s.panel_brush as LRESULT
                } else {
                    s.dark_brush as LRESULT
                }
            }
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN | WM_CTLCOLORDLG
        | WM_CTLCOLORSCROLLBAR => {
            SetTextColor(wparam as HDC, colorref(CREAM_RGB));
            SetBkColor(wparam as HDC, colorref(DARK_RGB));
            SetBkMode(wparam as HDC, OPAQUE as i32);
            s.dark_brush as LRESULT
        }
        WM_DRAWITEM => {
            let item = lparam as *const DRAWITEMSTRUCT;
            if !item.is_null() && (*item).CtlType == ODT_BUTTON {
                draw_button(s, &*item);
                1
            } else {
                0
            }
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
unsafe fn handle_command(s: &mut WindowState, wparam: WPARAM, lparam: LPARAM) {
    let id = (wparam & 0xffff) as i32;
    let code = ((wparam >> 16) & 0xffff) as u32;
    let control = lparam as HWND;
    if matches!(code, EN_SETFOCUS | CBN_SETFOCUS | BN_SETFOCUS) && !control.is_null() {
        if let Some(child) = s
            .children
            .iter()
            .find(|c| c.hwnd == control && c.page == s.page)
            .copied()
        {
            let mut bounds: RECT = std::mem::zeroed();
            GetWindowRect(control, &mut bounds);
            let top = child.y - s.scroll;
            let bottom = top + bounds.bottom - bounds.top;
            let pos = if top < scale(110, s.dpi) {
                child.y - scale(110, s.dpi)
            } else if bottom > s.client_height - scale(20, s.dpi) {
                s.scroll + bottom - s.client_height + scale(20, s.dpi)
            } else {
                s.scroll
            };
            scroll_to(s, pos.clamp(0, (s.content_height - s.client_height).max(0)));
        }
    }
    match (id, code) {
        (ID_PAUSE_ALL, BN_CLICKED) => {
            s.data.paused = !s.data.paused;
            s.events.push(PreferencesEvent::TogglePaused(s.data.paused));
            sync_controls(s, false);
        }
        (ID_ACTIVITY_SEARCH, EN_CHANGE) => {
            let mut query = [0u16; 256];
            let n = GetWindowTextW(control, query.as_mut_ptr(), 256);
            filter_activities(s, &String::from_utf16_lossy(&query[..n.max(0) as usize]));
        }
        (id, BN_CLICKED) if (ID_NAV_BASE..ID_NAV_BASE + 7).contains(&id) => {
            select_page(s, (id - ID_NAV_BASE) as usize);
        }
        (id, BN_CLICKED) if id == ID_QUICK_BASE + 60 || id == ID_QUICK_BASE + 63 => {
            s.events
                .push(PreferencesEvent::RunAction((id - ID_QUICK_BASE) as u16));
        }
        (ID_PET, CBN_SELCHANGE) => {
            let index = SendMessageW(s.pet_combo, CB_GETCURSEL, 0, 0) as usize;
            if let Some(id) = s.data.pet_ids.get(index) {
                s.data.selected_pet_id = *id;
                s.events.push(PreferencesEvent::SelectPet(*id));
            }
        }
        (ID_FPS, CBN_SELCHANGE) => {
            let fps = [30, 60, 120, 144, 240]
                .get(SendMessageW(s.fps_combo, CB_GETCURSEL, 0, 0) as usize)
                .copied()
                .unwrap_or(s.data.fps);
            s.data.fps = fps;
            s.events.push(PreferencesEvent::SetFps(fps));
        }
        (ID_SCALE, CBN_SELCHANGE) => {
            let scale = [0.75, 1.0, 1.5]
                .get(SendMessageW(s.scale_combo, CB_GETCURSEL, 0, 0) as usize)
                .copied()
                .unwrap_or(1.0);
            s.data.scale = scale;
            s.events.push(PreferencesEvent::SetScale(scale));
        }
        (ID_COLOR_BUTTON, BN_CLICKED) => s.events.push(PreferencesEvent::ChooseColor),
        (ID_RESET_COLOR, BN_CLICKED) => s.events.push(PreferencesEvent::ResetColor),
        (ID_RUN_ACTIVITY, BN_CLICKED) => {
            let index = SendMessageW(s.activity_combo, CB_GETCURSEL, 0, 0);
            if let Some(&catalog_index) = s.activity_items.get(index as usize) {
                s.events
                    .push(PreferencesEvent::RunAction(100 + catalog_index as u16));
            }
        }
        (ID_MIN_GAP, EN_KILLFOCUS) | (ID_MAX_GAP, EN_KILLFOCUS) => {
            // Moving between the two fields is still one edit of the range.
            let focused = GetFocus();
            if focused == s.min_edit || focused == s.max_edit {
                return;
            }
            let min = parse_edit(s.min_edit, s.data.min_gap);
            let max = parse_edit(s.max_edit, s.data.max_gap);
            if crate::settings::valid_personal_space(min, max) {
                s.data.min_gap = min;
                s.data.max_gap = max;
                set_text(s.spacing_status, "Расстояние обновлено для всех питомцев.");
                s.events.push(PreferencesEvent::SetSpacing(min, max));
            } else {
                set_text(
                    s.spacing_status,
                    "Проверьте диапазон: минимум 8–256, максимум от min + 8 до 512.",
                );
            }
        }
        (_, BN_CLICKED)
            if id == ID_MOOD
                || id == ID_BUBBLES
                || id == ID_REACTIONS
                || id == ID_ACTIVITIES
                || id == ID_SOCIAL
                || id == ID_DESKTOP
                || id == ID_PAUSED
                || id == ID_AUTO_UPDATES
                || id == ID_AUTO_INSTALL =>
        {
            let value = SendMessageW(control, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            match id {
                ID_MOOD => {
                    s.data.show_mood = value;
                    s.events.push(PreferencesEvent::ToggleMood(value));
                }
                ID_BUBBLES => {
                    s.data.show_bubbles = value;
                    s.events.push(PreferencesEvent::ToggleBubbles(value));
                }
                ID_REACTIONS => {
                    s.data.auto_reactions = value;
                    s.events.push(PreferencesEvent::ToggleReactions(value));
                }
                ID_ACTIVITIES => {
                    s.data.auto_activities = value;
                    s.events.push(PreferencesEvent::ToggleActivities(value));
                }
                ID_SOCIAL => {
                    s.data.social_enabled = value;
                    s.events.push(PreferencesEvent::ToggleSocial(value));
                }
                ID_DESKTOP => {
                    s.data.desktop_perches = value;
                    s.events.push(PreferencesEvent::ToggleDesktop(value));
                }
                ID_AUTO_UPDATES => {
                    s.data.auto_updates = value;
                    s.events.push(PreferencesEvent::ToggleAutoUpdates(value));
                }
                ID_AUTO_INSTALL => {
                    s.data.auto_install_updates = value;
                    s.events.push(PreferencesEvent::ToggleAutoInstall(value));
                }
                _ => {
                    s.data.paused = value;
                    s.events.push(PreferencesEvent::TogglePaused(value));
                }
            }
        }
        _ if code == BN_CLICKED && (ID_QUICK_BASE + 1..=ID_QUICK_BASE + 5).contains(&id) => s
            .events
            .push(PreferencesEvent::RunAction((id - ID_QUICK_BASE) as u16)),
        _ if code == BN_CLICKED && (ID_QUICK_BASE + 70..=ID_QUICK_BASE + 76).contains(&id) => s
            .events
            .push(PreferencesEvent::RunAction((id - ID_QUICK_BASE) as u16)),
        _ if code == BN_CLICKED && (ID_QUICK_BASE + 90..=ID_QUICK_BASE + 99).contains(&id) => s
            .events
            .push(PreferencesEvent::RunAction((id - ID_QUICK_BASE) as u16)),
        _ => {}
    }
}
unsafe fn draw_button(s: &WindowState, item: &DRAWITEMSTRUCT) {
    let selected = item.itemState & ODS_SELECTED != 0
        || GetDlgCtrlID(item.hwndItem) == ID_NAV_BASE + s.page as i32;
    let fill = if selected {
        s.accent_brush
    } else {
        s.panel_brush
    };
    FillRect(item.hDC, &item.rcItem, fill);
    FrameRect(item.hDC, &item.rcItem, s.accent_brush);
    if item.itemState & ODS_FOCUS != 0 {
        let mut focus = item.rcItem;
        focus.left += scale(4, s.dpi);
        focus.top += scale(4, s.dpi);
        focus.right -= scale(4, s.dpi);
        focus.bottom -= scale(4, s.dpi);
        FrameRect(item.hDC, &focus, s.accent_brush);
    }
    SetBkMode(item.hDC, TRANSPARENT as i32);
    SetTextColor(
        item.hDC,
        colorref(if item.itemState & ODS_DISABLED != 0 {
            0xA59D94
        } else {
            CREAM_RGB
        }),
    );
    let mut text = [0u16; 128];
    let len = GetWindowTextW(item.hwndItem, text.as_mut_ptr(), text.len() as i32);
    let mut rect = item.rcItem;
    let pad = scale(5, s.dpi);
    rect.left += pad;
    rect.right -= pad;
    let nav = GetDlgCtrlID(item.hwndItem) - ID_NAV_BASE;
    if (0..7).contains(&nav) {
        // Tiny pixel glyphs use GDI rectangles: no emoji font or runtime SVG.
        let pieces: &[(i32, i32, i32, i32)] = match nav {
            0 => &[(1, 1, 6, 6), (9, 1, 6, 6), (1, 9, 6, 6), (9, 9, 6, 6)],
            1 => &[(2, 1, 5, 7), (9, 8, 5, 7)],
            2 => &[(0, 4, 16, 3), (3, 11, 13, 3)],
            3 => &[(1, 1, 5, 5), (10, 1, 5, 5), (0, 8, 7, 7), (9, 8, 7, 7)],
            4 => &[(1, 10, 3, 6), (6, 6, 3, 10), (11, 1, 3, 15)],
            5 => &[(3, 1, 3, 14), (6, 3, 3, 10), (9, 5, 3, 6), (12, 7, 3, 2)],
            _ => &[
                (6, 0, 4, 9),
                (2, 6, 4, 3),
                (10, 6, 4, 3),
                (4, 9, 8, 3),
                (0, 14, 16, 2),
            ],
        };
        for &(x, y, w, h) in pieces {
            let left = item.rcItem.left + scale(8 + x, s.dpi);
            let top = item.rcItem.top + scale(7 + y, s.dpi);
            FillRect(
                item.hDC,
                &RECT {
                    left,
                    top,
                    right: left + scale(w, s.dpi),
                    bottom: top + scale(h, s.dpi),
                },
                GetStockObject(WHITE_BRUSH) as HBRUSH,
            );
        }
        rect.left += scale(27, s.dpi);
    }
    DrawTextW(
        item.hDC,
        text.as_ptr(),
        len,
        &mut rect,
        (if (0..7).contains(&nav) {
            DT_LEFT
        } else {
            DT_CENTER
        }) | DT_VCENTER
            | DT_SINGLELINE
            | DT_END_ELLIPSIS,
    );
}
unsafe fn parse_edit(hwnd: HWND, fallback: f64) -> f64 {
    let mut buf = [0u16; 64];
    let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(fallback)
        .clamp(0.0, 512.0)
}
unsafe fn update_scroll(s: &mut WindowState) {
    s.content_height = s.page_heights[s.page];
    let visible = (s.content_height - s.client_height).max(0);
    let info = SCROLLINFO {
        cbSize: size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: s.content_height.max(0),
        nPage: s.client_height.max(1) as u32,
        nPos: s.scroll,
        nTrackPos: 0,
    };
    SetScrollInfo(s.hwnd, SB_VERT, &info, 1);
    if s.scroll > visible {
        scroll_to(s, visible);
    }
}
unsafe fn scroll(s: &mut WindowState, command: i32, track: i32) {
    let page = (s.client_height - scale(80, s.dpi)).max(scale(36, s.dpi));
    let pos = match command {
        SB_LINEUP => s.scroll - scale(32, s.dpi),
        SB_LINEDOWN => s.scroll + scale(32, s.dpi),
        SB_PAGEUP => s.scroll - page,
        SB_PAGEDOWN => s.scroll + page,
        SB_TOP => 0,
        SB_BOTTOM => s.content_height,
        SB_THUMBPOSITION | SB_THUMBTRACK => track,
        _ => s.scroll,
    };
    scroll_to(s, pos.clamp(0, (s.content_height - s.client_height).max(0)));
}
unsafe fn scroll_to(s: &mut WindowState, pos: i32) {
    if pos == s.scroll {
        return;
    }
    s.scroll = pos;
    for child in &s.children {
        let top = child.y - if child.page == FIXED { 0 } else { pos };
        SetWindowPos(
            child.hwnd,
            null_mut(),
            child.x,
            top,
            child.w,
            child.h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let mut bounds: RECT = std::mem::zeroed();
        GetWindowRect(child.hwnd, &mut bounds);
        let height = bounds.bottom - bounds.top;
        let clip = (scale(100, s.dpi) - top).max(0);
        let visible = child.page == FIXED || (child.page == s.page && clip < height);
        if child.page != FIXED && visible && clip > 0 {
            let region = CreateRectRgn(0, clip, child.w, height);
            if SetWindowRgn(child.hwnd, region, 1) == 0 {
                DeleteObject(region as HGDIOBJ);
            }
        } else {
            SetWindowRgn(child.hwnd, null_mut(), 1);
        }
        ShowWindow(child.hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
    }
    let info = SCROLLINFO {
        cbSize: size_of::<SCROLLINFO>() as u32,
        fMask: SIF_POS,
        nMin: 0,
        nMax: 0,
        nPage: 0,
        nPos: s.scroll,
        nTrackPos: 0,
    };
    SetScrollInfo(s.hwnd, SB_VERT, &info, 1);
}

unsafe fn select_page(s: &mut WindowState, page: usize) {
    if page >= s.page_heights.len() {
        return;
    }
    // Commit a spacing edit via its existing focus-loss handler before hiding it.
    SetFocus(GetDlgItem(s.hwnd, ID_NAV_BASE + page as i32));
    s.page_scroll[s.page] = s.scroll.max(0);
    s.page = page;
    let pos = s.page_scroll[page].min((s.page_heights[page] - s.client_height).max(0));
    s.scroll = -1;
    scroll_to(s, pos);
    EnableWindow(s.pet_combo, i32::from(matches!(page, 0 | 1 | 3 | 5)));
    set_text(
        s.scope_label,
        match page {
            0 => "Цвет — выбранному питомцу. Масштаб и видимость — всем.",
            1 => "Настройки — всем. Команды — выбранному питомцу.",
            3 => "Расстояние — всем. Игры — с участием выбранного питомца.",
            5 => "Занятия для выбранного питомца",
            _ => "Общие настройки для всех питомцев",
        },
    );
    update_scroll(s);
    InvalidateRect(s.hwnd, null(), 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tab_focus_scrolls_commands_into_view_without_running_them() {
        let mut panel = PreferencesWindow::new().unwrap();
        unsafe {
            SetWindowPos(
                panel.hwnd,
                null_mut(),
                0,
                0,
                scale(850, panel.state.dpi),
                scale(420, panel.state.dpi),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            select_page(&mut panel.state, 1);
            let button = GetDlgItem(panel.hwnd, ID_QUICK_BASE + 76);
            handle_command(
                &mut panel.state,
                (ID_QUICK_BASE + 76) as usize | ((BN_SETFOCUS as usize) << 16),
                button as isize,
            );
            assert!(panel.state.scroll > 0);
            assert!(panel.state.events.is_empty());
            handle_command(
                &mut panel.state,
                (ID_QUICK_BASE + 76) as usize,
                button as isize,
            );
            assert_eq!(
                panel.state.events.pop(),
                Some(PreferencesEvent::RunAction(76))
            );
        }
    }
    #[test]
    fn filtered_activity_runs_catalog_identity_and_empty_search_is_safe() {
        let mut panel = PreferencesWindow::new().unwrap();
        unsafe {
            let target = CATALOG.len() - 1;
            filter_activities(&mut panel.state, CATALOG[target].title_ru);
            let row = panel
                .state
                .activity_items
                .iter()
                .position(|&i| i == target)
                .unwrap();
            SendMessageW(panel.state.activity_combo, CB_SETCURSEL, row, 0);
            handle_command(&mut panel.state, ID_RUN_ACTIVITY as usize, 0);
            assert_eq!(
                panel.state.events.pop(),
                Some(PreferencesEvent::RunAction(100 + target as u16))
            );
            filter_activities(&mut panel.state, "zzzz-no-such-activity");
            assert!(panel.state.activity_items.is_empty());
            assert_ne!(
                GetWindowLongW(GetDlgItem(panel.hwnd, ID_RUN_ACTIVITY), GWL_STYLE) as u32
                    & WS_DISABLED,
                0
            );
            handle_command(&mut panel.state, ID_RUN_ACTIVITY as usize, 0);
            assert!(panel.state.events.is_empty());
            filter_activities(&mut panel.state, "");
            assert_eq!(panel.state.activity_items.len(), CATALOG.len());
        }
    }
    #[test]
    fn page_scroll_scope_and_global_pause_work_across_sections() {
        let mut panel = PreferencesWindow::new().unwrap();
        unsafe {
            SetWindowPos(
                panel.hwnd,
                null_mut(),
                0,
                0,
                scale(850, panel.state.dpi),
                scale(420, panel.state.dpi),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            select_page(&mut panel.state, 1);
            scroll_to(&mut panel.state, 60);
            select_page(&mut panel.state, 4);
            assert_ne!(
                GetWindowLongW(panel.state.pet_combo, GWL_STYLE) as u32 & WS_DISABLED,
                0
            );
            select_page(&mut panel.state, 1);
            assert_eq!(panel.state.scroll, 60);
            assert_eq!(
                GetWindowLongW(panel.state.pet_combo, GWL_STYLE) as u32 & WS_DISABLED,
                0
            );
            handle_command(&mut panel.state, ID_PAUSE_ALL as usize, 0);
            assert_eq!(
                panel.state.events.pop(),
                Some(PreferencesEvent::TogglePaused(true))
            );
            assert_eq!(
                SendMessageW(GetDlgItem(panel.hwnd, ID_PAUSED), BM_GETCHECK, 0, 0),
                BST_CHECKED as isize
            );
            handle_command(&mut panel.state, ID_PAUSE_ALL as usize, 0);
            assert_eq!(
                panel.state.events.pop(),
                Some(PreferencesEvent::TogglePaused(false))
            );
        }
    }
    #[test]
    fn invalid_spacing_does_not_dispatch_unusable_settings() {
        let mut panel = PreferencesWindow::new().unwrap();
        unsafe {
            set_text(panel.state.min_edit, "200");
            set_text(panel.state.max_edit, "100");
            handle_command(
                &mut panel.state,
                ID_MIN_GAP as usize | ((EN_KILLFOCUS as usize) << 16),
                0,
            );
            assert!(panel.state.events.is_empty());
            assert_eq!(panel.state.data.min_gap, 24.0);
            set_text(panel.state.max_edit, "240");
            handle_command(
                &mut panel.state,
                ID_MAX_GAP as usize | ((EN_KILLFOCUS as usize) << 16),
                0,
            );
            assert_eq!(
                panel.state.events.pop(),
                Some(PreferencesEvent::SetSpacing(200.0, 240.0))
            );
        }
    }
    #[test]
    fn preferences_has_embedded_small_and_big_window_icons() {
        let panel = PreferencesWindow::new().unwrap();
        unsafe {
            assert!(!panel.state.small_icon.is_null());
            assert!(!panel.state.big_icon.is_null());
            assert_eq!(
                SendMessageW(panel.hwnd, WM_GETICON, ICON_SMALL as usize, 0),
                panel.state.small_icon as isize
            );
            assert_eq!(
                SendMessageW(panel.hwnd, WM_GETICON, ICON_BIG as usize, 0),
                panel.state.big_icon as isize
            );
        }
    }
    #[test]
    fn pages_hide_other_controls_keep_header_fixed_and_preserve_values() {
        let mut panel = PreferencesWindow::new().unwrap();
        unsafe {
            let visible = |h| GetWindowLongW(h, GWL_STYLE) as u32 & WS_VISIBLE != 0;
            assert!(visible(panel.state.color_swatch));
            assert!(!visible(panel.state.fps_combo));
            let mut before: RECT = std::mem::zeroed();
            GetWindowRect(panel.state.pet_combo, &mut before);
            select_page(&mut panel.state, 4);
            assert!(visible(panel.state.fps_combo));
            assert!(!visible(panel.state.color_swatch));
            assert!(visible(panel.state.pet_combo));
            select_page(&mut panel.state, 3);
            set_text(panel.state.min_edit, "40");
            select_page(&mut panel.state, 6);
            select_page(&mut panel.state, 3);
            assert_eq!(parse_edit(panel.state.min_edit, 0.0), 40.0);
            scroll_to(&mut panel.state, 50);
            let mut after: RECT = std::mem::zeroed();
            GetWindowRect(panel.state.pet_combo, &mut after);
            assert_eq!(before.top, after.top);
        }
    }
    #[test]
    fn real_controls_represent_preferences_and_scroll_more_than_once() {
        let mut panel = PreferencesWindow::new().unwrap();
        panel.sync(PreferencesState {
            pet_ids: vec![1, 4, 7],
            selected_pet_id: 7,
            fps: 120,
            intensity: 80,
            show_mood: true,
            ..PreferencesState::default()
        });
        unsafe {
            assert_eq!(SendMessageW(panel.state.pet_combo, CB_GETCOUNT, 0, 0), 3);
            assert_eq!(SendMessageW(panel.state.pet_combo, CB_GETCURSEL, 0, 0), 2);
            assert_eq!(SendMessageW(panel.state.fps_combo, CB_GETCURSEL, 0, 0), 2);
            assert_eq!(
                SendMessageW(panel.state.activity_combo, CB_GETCOUNT, 0, 0),
                75
            );
            // TBM_GETPOS is WM_USER (not exported by this windows-sys version).
            assert_eq!(SendMessageW(panel.state.intensity, WM_USER, 0, 0), 80);
            assert_eq!(
                SendMessageW(GetDlgItem(panel.hwnd, ID_MOOD), BM_GETCHECK, 0, 0),
                BST_CHECKED as isize
            );
            panel.sync(PreferencesState {
                show_mood: false,
                ..PreferencesState::default()
            });
            assert_eq!(
                SendMessageW(GetDlgItem(panel.hwnd, ID_MOOD), BM_GETCHECK, 0, 0),
                0
            );
            panel.sync(PreferencesState {
                fps: 90,
                ..PreferencesState::default()
            });
            assert_eq!(SendMessageW(panel.state.fps_combo, CB_GETCURSEL, 0, 0), 5);
            let child = panel.state.color_swatch;
            scroll_to(&mut panel.state, 100);
            let mut first: RECT = std::mem::zeroed();
            GetWindowRect(child, &mut first);
            scroll_to(&mut panel.state, 200);
            let mut second: RECT = std::mem::zeroed();
            GetWindowRect(child, &mut second);
            assert_eq!(second.top - first.top, -100);
        }
    }
}
