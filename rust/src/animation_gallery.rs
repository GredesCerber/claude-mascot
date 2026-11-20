//! Native, virtualized animation-card gallery. Previews never update the pets'
//! simulation: only visible cards render, on the 12Hz pose clock.
use crate::{
    activities::CATALOG,
    engine::{Pet, Skin, State},
    render::{Renderer, HEIGHT, WIDTH},
    Vec2,
};
use std::{
    cell::RefCell,
    ffi::c_void,
    mem::size_of,
    ptr::{null, null_mut},
    sync::OnceLock,
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{SetScrollInfo, SetWindowTheme},
        Input::KeyboardAndMouse::{GetFocus, SetFocus},
        WindowsAndMessaging::*,
    },
};
const CLASS: &[u16] = &[
    67, 108, 97, 117, 100, 101, 65, 110, 105, 109, 97, 116, 105, 111, 110, 71, 97, 108, 108, 101,
    114, 121, 0,
];
const TIMER: usize = 1;
pub(crate) const CHANGED: u32 = 0x400;
const BG: u32 = 0x302B27;
const CARD: u32 = 0x252220;
const TEXT: u32 = 0xF3EDE2;
const MUTED: u32 = 0xB8AFA4;
const ACCENT: u32 = 0xD97757;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rgb(c: u32) -> u32 {
    ((c >> 16) & 255) | (c & 0xFF00) | ((c & 255) << 16)
}
#[derive(Clone, Copy, Debug)]
struct Layout {
    columns: usize,
    card_w: i32,
    card_h: i32,
    gap: i32,
}
impl Layout {
    fn new(width: i32, dpi: f64) -> Self {
        let gap = (12.0 * dpi).round() as i32;
        let columns = ((width - gap).max(1) / (250.0 * dpi).round() as i32).max(1) as usize;
        let card_w = ((width - gap * (columns as i32 + 1)) / columns as i32).max(1);
        Self {
            columns,
            card_w,
            card_h: (208.0 * dpi).round() as i32,
            gap,
        }
    }
    fn bounds(self, row: usize, scroll: i32) -> RECT {
        let x = self.gap + (row % self.columns) as i32 * (self.card_w + self.gap);
        let y = self.gap + (row / self.columns) as i32 * (self.card_h + self.gap) - scroll;
        RECT {
            left: x,
            top: y,
            right: x + self.card_w,
            bottom: y + self.card_h,
        }
    }
    fn total(self, count: usize) -> i32 {
        count.div_ceil(self.columns) as i32 * (self.card_h + self.gap) + self.gap
    }
    fn hit(self, x: i32, y: i32, scroll: i32, count: usize) -> Option<usize> {
        let x = x - self.gap;
        let y = y + scroll - self.gap;
        if x < 0 || y < 0 {
            return None;
        }
        let column = x / (self.card_w + self.gap);
        let row = y / (self.card_h + self.gap);
        if column as usize >= self.columns
            || x % (self.card_w + self.gap) >= self.card_w
            || y % (self.card_h + self.gap) >= self.card_h
        {
            return None;
        }
        let index = row as usize * self.columns + column as usize;
        (index < count).then_some(index)
    }
    fn visible(self, count: usize, scroll: i32, height: i32) -> std::ops::Range<usize> {
        let first = (scroll / (self.card_h + self.gap)).max(0) as usize * self.columns;
        let end = ((scroll + height) / (self.card_h + self.gap) + 1).max(0) as usize * self.columns;
        first.min(count)..end.min(count)
    }
}
#[derive(Default)]
struct Buffer {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    w: i32,
    h: i32,
}
impl Buffer {
    unsafe fn resize(&mut self, dc: HDC, w: i32, h: i32) -> bool {
        if self.w == w && self.h == h && !self.dc.is_null() {
            return true;
        }
        self.release();
        if w <= 0 || h <= 0 {
            return false;
        }
        self.dc = CreateCompatibleDC(dc);
        if self.dc.is_null() {
            return false;
        }
        self.bitmap = CreateCompatibleBitmap(dc, w, h);
        if self.bitmap.is_null() {
            self.release();
            return false;
        }
        self.old = SelectObject(self.dc, self.bitmap as HGDIOBJ);
        self.w = w;
        self.h = h;
        true
    }
    unsafe fn release(&mut self) {
        if !self.dc.is_null() {
            if !self.old.is_null() {
                SelectObject(self.dc, self.old);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap as HGDIOBJ);
            }
            DeleteDC(self.dc);
        }
        self.dc = null_mut();
        self.bitmap = null_mut();
        self.old = null_mut();
        self.w = 0;
        self.h = 0;
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.release();
        }
    }
}
struct GalleryState {
    hwnd: HWND,
    parent: HWND,
    id: i32,
    dpi: f64,
    font: HFONT,
    renderer: Renderer,
    pets: Vec<Pet>,
    items: Vec<usize>,
    selected: Option<usize>,
    scroll: i32,
    width: i32,
    height: i32,
    color: u32,
    start: Instant,
    selected_at: Instant,
    runs: Vec<usize>,
    active: bool,
    buffer: Buffer,
    pixels: Vec<u32>,
}
pub(crate) struct AnimationGallery {
    hwnd: HWND,
    state: Box<RefCell<GalleryState>>,
}
impl AnimationGallery {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parent: HWND,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        dpi: f64,
        font: HFONT,
    ) -> Result<Self, String> {
        ensure_class()?;
        let mut renderer = Renderer::new()?;
        renderer.show_mood = false;
        let pets = CATALOG
            .iter()
            .map(|spec| {
                let mut p = Pet::new(spec.id.index() as u64 + 1, Skin::Claude, Vec2::default());
                p.state = State::Activity;
                p.activity = Some(spec.id);
                p
            })
            .collect();
        let mut state = Box::new(RefCell::new(GalleryState {
            hwnd: null_mut(),
            parent,
            id,
            dpi,
            font,
            renderer,
            pets,
            items: (0..CATALOG.len()).collect(),
            selected: Some(0),
            scroll: 0,
            width: w,
            height: h,
            color: 0xD97757,
            start: Instant::now(),
            selected_at: Instant::now(),
            runs: Vec::new(),
            active: false,
            buffer: Buffer::default(),
            pixels: Vec::with_capacity(WIDTH * HEIGHT),
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                CLASS.as_ptr(),
                wide("Галерея анимаций").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL,
                x,
                y,
                w,
                h,
                parent,
                id as usize as HMENU,
                GetModuleHandleW(null()),
                (&mut *state as *mut RefCell<GalleryState>).cast::<c_void>(),
            )
        };
        if hwnd.is_null() {
            return Err("Не удалось открыть галерею анимаций".into());
        }
        state.borrow_mut().hwnd = hwnd;
        unsafe {
            SetWindowTheme(hwnd, wide("DarkMode_Explorer").as_ptr(), null());
        }
        Ok(Self { hwnd, state })
    }
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
    pub fn selected(&self) -> Option<usize> {
        self.state.borrow().selected
    }
    pub fn item_count(&self) -> usize {
        self.state.borrow().items.len()
    }
    #[cfg(test)]
    pub fn select(&mut self, id: usize) {
        unsafe {
            self.state.borrow_mut().select(id, true);
        }
    }
    pub fn set_items(&mut self, items: Vec<usize>) {
        let mut state = self.state.borrow_mut();
        state.items = items.into_iter().filter(|i| *i < CATALOG.len()).collect();
        if !state.selected.is_some_and(|id| state.items.contains(&id)) {
            state.selected = state.items.first().copied();
            state.selected_at = Instant::now();
        }
        state.scroll = 0;
        unsafe {
            state.update_scroll();
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    pub fn set_color(&mut self, color: u32) {
        let mut state = self.state.borrow_mut();
        if state.color != color {
            state.color = color;
            unsafe {
                InvalidateRect(self.hwnd, null(), 0);
            }
        }
    }
    pub fn set_active(&mut self, active: bool) {
        unsafe {
            self.state.borrow_mut().set_active(active);
        }
    }
    pub fn take_runs(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.state.borrow_mut().runs)
    }
}
impl Drop for AnimationGallery {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}
impl GalleryState {
    fn layout(&self) -> Layout {
        Layout::new(self.width, self.dpi)
    }
    unsafe fn set_active(&mut self, active: bool) {
        self.active = active;
        if active {
            SetTimer(self.hwnd, TIMER, 83, None);
            InvalidateRect(self.hwnd, null(), 0);
        } else {
            KillTimer(self.hwnd, TIMER);
        }
    }
    unsafe fn update_scroll(&mut self) {
        // A native scrollbar can change client width synchronously. Reconcile
        // that geometry here because reentrant callbacks are borrow-guarded.
        for _ in 0..2 {
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut rect);
            self.width = rect.right;
            self.height = rect.bottom;
            let total = self.layout().total(self.items.len());
            self.scroll = self.scroll.clamp(0, (total - self.height).max(0));
            let info = SCROLLINFO {
                cbSize: size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: (total - 1).max(0),
                nPage: self.height.max(1) as u32,
                nPos: self.scroll,
                nTrackPos: 0,
            };
            SetScrollInfo(self.hwnd, SB_VERT, &info, 1);
        }
    }
    unsafe fn scroll_to(&mut self, to: i32) {
        self.scroll = to;
        self.update_scroll();
        InvalidateRect(self.hwnd, null(), 0);
    }
    unsafe fn select(&mut self, id: usize, reset: bool) {
        if !self.items.contains(&id) {
            return;
        }
        self.selected = Some(id);
        if reset {
            self.selected_at = Instant::now();
        }
        let spec = &CATALOG[id];
        SetWindowTextW(
            self.hwnd,
            wide(&format!(
                "{} · {} · {:.1} с",
                spec.title_ru,
                spec.group.label_ru(),
                spec.duration
            ))
            .as_ptr(),
        );
        InvalidateRect(self.hwnd, null(), 0);
        PostMessageW(
            self.parent,
            WM_COMMAND,
            self.id as usize | ((CHANGED as usize) << 16),
            self.hwnd as LPARAM,
        );
    }
    unsafe fn ensure_selected_visible(&mut self) {
        if let Some(row) = self
            .selected
            .and_then(|id| self.items.iter().position(|i| *i == id))
        {
            let rect = self.layout().bounds(row, self.scroll);
            if rect.top < 0 {
                self.scroll_to(self.scroll + rect.top);
            } else if rect.bottom > self.height {
                self.scroll_to(self.scroll + rect.bottom - self.height);
            }
        }
    }
    unsafe fn key(&mut self, key: u32) {
        if key == 0x0D || key == 0x20 {
            if let Some(id) = self.selected {
                self.runs.push(id);
            }
            return;
        }
        let Some(row) = self
            .selected
            .and_then(|id| self.items.iter().position(|i| *i == id))
        else {
            return;
        };
        let cols = self.layout().columns;
        let page =
            (self.height / (self.layout().card_h + self.layout().gap)).max(1) as usize * cols;
        let next = match key {
            0x25 => row.saturating_sub(1),
            0x27 => row + 1,
            0x26 => row.saturating_sub(cols),
            0x28 => row + cols,
            0x24 => 0,
            0x23 => self.items.len() - 1,
            0x21 => row.saturating_sub(page),
            0x22 => row + page,
            _ => return,
        };
        let id = self.items[next.min(self.items.len() - 1)];
        self.select(id, true);
        self.ensure_selected_visible();
    }
    unsafe fn paint(&mut self, dc: HDC) {
        if !self.buffer.resize(dc, self.width, self.height) {
            return;
        }
        let target = self.buffer.dc;
        fill(
            target,
            RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            },
            BG,
        );
        let previous_font = SelectObject(target, self.font as HGDIOBJ);
        SetBkMode(target, TRANSPARENT as i32);
        let layout = self.layout();
        if self.items.is_empty() {
            text(
                target,
                "Ничего не найдено. Попробуйте другое название.",
                RECT {
                    left: 16,
                    top: 24,
                    right: self.width - 16,
                    bottom: 100,
                },
                MUTED,
                DT_CENTER | DT_WORDBREAK,
            );
        }
        let elapsed = self.start.elapsed().as_secs_f64();
        for row in layout.visible(self.items.len(), self.scroll, self.height) {
            let id = self.items[row];
            let spec = &CATALOG[id];
            let rect = layout.bounds(row, self.scroll);
            fill(target, rect, CARD);
            let border = CreateSolidBrush(rgb(if self.selected == Some(id) {
                ACCENT
            } else {
                0x514940
            }));
            FrameRect(target, &rect, border);
            DeleteObject(border as HGDIOBJ);
            let phase = if self.selected == Some(id) {
                self.selected_at.elapsed().as_secs_f64()
            } else {
                elapsed + id as f64 * 0.37
            };
            let phase = phase % (spec.duration + 0.65);
            let pet = &mut self.pets[id];
            pet.body_color = self.color;
            pet.time = phase;
            pet.state_time = phase;
            let frame = self.renderer.frame(pet, false);
            self.pixels.clear();
            for &pixel in &frame.pixels {
                let a = pixel >> 24;
                let inv = 255 - a;
                let r = ((pixel >> 16) & 255) + ((CARD >> 16) & 255) * inv / 255;
                let g = ((pixel >> 8) & 255) + ((CARD >> 8) & 255) * inv / 255;
                let b = (pixel & 255) + (CARD & 255) * inv / 255;
                self.pixels.push(0xFF000000 | (r << 16) | (g << 8) | b);
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = WIDTH as i32;
            info.bmiHeader.biHeight = -(HEIGHT as i32);
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let pad = (12.0 * self.dpi).round() as i32;
            let picture_h = (148.0 * self.dpi).round() as i32;
            SetStretchBltMode(target, COLORONCOLOR);
            let picture_w = (picture_h * WIDTH as i32 / HEIGHT as i32).min(layout.card_w - pad * 2);
            StretchDIBits(
                target,
                rect.left + (layout.card_w - picture_w) / 2,
                rect.top + pad,
                picture_w,
                picture_h,
                0,
                0,
                WIDTH as i32,
                HEIGHT as i32,
                self.pixels.as_ptr().cast::<c_void>(),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            text(
                target,
                spec.title_ru,
                RECT {
                    left: rect.left + pad,
                    top: rect.top + picture_h + pad,
                    right: rect.right - pad,
                    bottom: rect.top + picture_h + pad + (23.0 * self.dpi) as i32,
                },
                TEXT,
                DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            text(
                target,
                &format!("{} · {:.1} с", spec.group.label_ru(), spec.duration),
                RECT {
                    left: rect.left + pad,
                    top: rect.top + picture_h + pad + (25.0 * self.dpi) as i32,
                    right: rect.right - pad,
                    bottom: rect.bottom - pad / 2,
                },
                MUTED,
                DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            if self.selected == Some(id) && GetFocus() == self.hwnd {
                let focus = RECT {
                    left: rect.left + 3,
                    top: rect.top + 3,
                    right: rect.right - 3,
                    bottom: rect.bottom - 3,
                };
                DrawFocusRect(target, &focus);
            }
        }
        SelectObject(target, previous_font);
        BitBlt(dc, 0, 0, self.width, self.height, target, 0, 0, SRCCOPY);
    }
}
unsafe fn fill(dc: HDC, rect: RECT, color: u32) {
    let brush = CreateSolidBrush(rgb(color));
    FillRect(dc, &rect, brush);
    DeleteObject(brush as HGDIOBJ);
}
unsafe fn text(dc: HDC, value: &str, mut rect: RECT, color: u32, flags: u32) {
    let value = wide(value);
    SetTextColor(dc, rgb(color));
    DrawTextW(dc, value.as_ptr(), -1, &mut rect, flags);
}
fn ensure_class() -> Result<(), String> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| unsafe {
            let class = WNDCLASSW {
                style: CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(null()),
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: CLASS.as_ptr(),
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err("Не удалось зарегистрировать галерею".into());
            }
            Ok(())
        })
        .clone()
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<GalleryState>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    // Win32 can re-enter while focus or non-client scrollbar geometry changes.
    // Keep those nested callbacks from aliasing the outer mutable state borrow.
    let Ok(mut s) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    s.hwnd = hwnd;
    match msg {
        WM_SIZE => {
            s.width = (lparam as u32 & 65535) as i32;
            s.height = (lparam as u32 >> 16) as i32;
            s.update_scroll();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_SHOWWINDOW => {
            let parent_visible = IsWindowVisible(s.parent) != 0;
            s.set_active(wparam != 0 && parent_visible);
            0
        }
        WM_TIMER => {
            if s.active && IsWindowVisible(hwnd) != 0 {
                InvalidateRect(hwnd, null(), 0);
            } else {
                s.set_active(false);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            s.paint(dc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            SetFocus(hwnd);
            let x = (lparam as u32 & 65535) as i16 as i32;
            let y = (lparam as u32 >> 16) as i16 as i32;
            if let Some(row) = s.layout().hit(x, y, s.scroll, s.items.len()) {
                let id = s.items[row];
                s.select(id, true);
                if msg == WM_LBUTTONDBLCLK {
                    s.runs.push(id);
                }
            }
            0
        }
        WM_CHAR if wparam == 0x0D || wparam == 0x20 => 0,
        WM_KEYDOWN => {
            s.key(wparam as u32);
            0
        }
        WM_GETDLGCODE => {
            let key = if lparam != 0 {
                (*(lparam as *const MSG)).wParam as u32
            } else {
                0
            };
            if key == 0x0D || key == 0x20 {
                DLGC_WANTMESSAGE as LRESULT
            } else {
                DLGC_WANTARROWS as LRESULT
            }
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16) as i32;
            let next = s.scroll - (delta as f64 * s.dpi * 0.75).round() as i32;
            s.scroll_to(next);
            0
        }
        WM_VSCROLL => {
            let mut info: SCROLLINFO = std::mem::zeroed();
            info.cbSize = size_of::<SCROLLINFO>() as u32;
            info.fMask = SIF_TRACKPOS;
            GetScrollInfo(hwnd, SB_VERT, &mut info);
            let next = match (wparam & 65535) as i32 {
                SB_LINEUP => s.scroll - (36.0 * s.dpi) as i32,
                SB_LINEDOWN => s.scroll + (36.0 * s.dpi) as i32,
                SB_PAGEUP => s.scroll - s.height,
                SB_PAGEDOWN => s.scroll + s.height,
                SB_TOP => 0,
                SB_BOTTOM => s.layout().total(s.items.len()),
                SB_THUMBTRACK | SB_THUMBPOSITION => info.nTrackPos,
                _ => s.scroll,
            };
            s.scroll_to(next);
            0
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_DESTROY => {
            s.set_active(false);
            0
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn card_hit_testing_respects_gaps_and_scroll() {
        let l = Layout::new(570, 1.0);
        assert_eq!(l.columns, 2);
        assert_eq!(l.hit(20, 20, 0, 78), Some(0));
        assert_eq!(l.hit(l.card_w + 25, 20, 0, 78), Some(1));
        assert_eq!(l.hit(2, 20, 0, 78), None);
        assert_eq!(l.hit(20, l.card_h + 15, 0, 78), None);
        assert_eq!(l.hit(20, 20, l.card_h + l.gap, 78), Some(2));
    }
    #[test]
    fn rendering_window_is_bounded_even_for_a_large_catalog() {
        for dpi in [1.0, 1.5, 2.0] {
            let l = Layout::new((570.0 * dpi) as i32, dpi);
            let visible = l.visible(78, 1800, (360.0 * dpi) as i32);
            assert!(visible.len() <= 8);
            assert!(visible.end <= 78);
            assert_eq!(l.hit(-1, -1, 0, 78), None);
        }
    }
}
