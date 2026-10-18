"""Windows: окна узнаём через WinAPI (ctypes), видео — через системную панель медиа
(та, что всплывает при нажатии клавиш громкости; пакеты winrt-*).

Без пакетов winrt маскот просто не подглядывает за видео, остальное работает.
"""
import asyncio
import ctypes
import ctypes.wintypes as wt
import os
import threading

from PyQt6.QtCore import QDir, QLockFile, QTimer

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
try:
    dwmapi = ctypes.WinDLL("dwmapi")
except OSError:
    dwmapi = None

GWL_EXSTYLE = -20
WS_EX_TOOLWINDOW = 0x00000080
GW_OWNER = 4
DWMWA_EXTENDED_FRAME_BOUNDS = 9
DWMWA_CLOAKED = 14
MONITOR_DEFAULTTONEAREST = 2
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000

# служебные окна рабочего стола и панели задач — не настоящие окна
SKIP_CLASSES = {"Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd",
                "Windows.UI.Core.CoreWindow", "NotifyIconOverflowWindow"}
AUDIO_PLAYERS = {"spotify", "music.ui", "zunemusic", "applemusic", "yandex.music",
                 "foobar2000", "aimp", "musicbee", "itunes", "deezer", "tidal"}
# Firefox регистрируется в панели медиа под хешем вместо имени
FIREFOX_IDS = {"308046b0af4a39cb", "6f193ccc56814779", "e7cf176e110c211b"}


class LASTINPUTINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.UINT), ("dwTime", wt.DWORD)]


def idle_seconds():
    """Сколько секунд пользователь не трогал мышь и клавиатуру (GetLastInputInfo)."""
    info = LASTINPUTINFO(ctypes.sizeof(LASTINPUTINFO), 0)
    if not user32.GetLastInputInfo(ctypes.byref(info)):
        return 0.0
    return ((kernel32.GetTickCount() - info.dwTime) & 0xFFFFFFFF) / 1000.0


class MONITORINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.DWORD), ("rcMonitor", wt.RECT),
                ("rcWork", wt.RECT), ("dwFlags", wt.DWORD)]


EnumWindowsProc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
user32.EnumWindows.argtypes = [EnumWindowsProc, wt.LPARAM]
user32.GetWindowLongW.argtypes = [wt.HWND, ctypes.c_int]
user32.GetWindow.argtypes = [wt.HWND, wt.UINT]
user32.GetWindow.restype = wt.HWND
user32.IsWindowVisible.argtypes = [wt.HWND]
user32.IsIconic.argtypes = [wt.HWND]
user32.GetForegroundWindow.restype = wt.HWND
user32.GetWindowTextLengthW.argtypes = [wt.HWND]
user32.GetWindowTextW.argtypes = [wt.HWND, wt.LPWSTR, ctypes.c_int]
user32.GetClassNameW.argtypes = [wt.HWND, wt.LPWSTR, ctypes.c_int]
user32.GetWindowRect.argtypes = [wt.HWND, ctypes.POINTER(wt.RECT)]
user32.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
user32.MonitorFromWindow.argtypes = [wt.HWND, wt.DWORD]
user32.MonitorFromWindow.restype = wt.HANDLE
user32.GetMonitorInfoW.argtypes = [wt.HANDLE, ctypes.POINTER(MONITORINFO)]
kernel32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
kernel32.OpenProcess.restype = wt.HANDLE
kernel32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
kernel32.CloseHandle.argtypes = [wt.HANDLE]
kernel32.GetTickCount.restype = wt.DWORD
if dwmapi:
    dwmapi.DwmGetWindowAttribute.argtypes = [wt.HWND, wt.DWORD, ctypes.c_void_p, wt.DWORD]

_keep = []
_exe_cache = {}
OWN_PID = os.getpid()


def setup_env():
    # WinAPI отдаёт координаты в физических пикселях — пусть Qt работает в них же.
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "0")


def prepare_cursor():
    return True


def move_cursor(x, y):
    from PyQt6.QtGui import QCursor
    QCursor.setPos(int(x), int(y))  # SetCursorPos


def window_flags():
    from PyQt6.QtCore import Qt
    return Qt.WindowType(0)


def start(app, world):
    """Подключает источники данных о мире. False — маскот уже запущен."""
    lock = QLockFile(QDir.temp().filePath("claude-mascot.lock"))
    if not lock.tryLock(100):
        return False

    def refresh():
        try:
            world.set_state({"windows": list_windows(), "areas": []})
        except OSError:
            pass

    timer = QTimer()
    timer.timeout.connect(refresh)
    timer.start(16)  # часто: по скорости окна маскот понимает, что его трясут
    refresh()
    threading.Thread(target=media_thread, args=(world,), daemon=True).start()
    _keep.extend([lock, timer])
    return True


# ---------------------------------------------------------------- окна

def exe_name(hwnd):
    pid = wt.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    if pid.value in _exe_cache:
        return pid.value, _exe_cache[pid.value]
    name = ""
    h = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid.value)
    if h:
        buf = ctypes.create_unicode_buffer(1024)
        size = wt.DWORD(1024)
        if kernel32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(size)):
            name = os.path.basename(buf.value).lower()
        kernel32.CloseHandle(h)
    if len(_exe_cache) > 500:
        _exe_cache.clear()
    _exe_cache[pid.value] = name
    return pid.value, name


def window_rect(hwnd):
    rect = wt.RECT()
    # без невидимых рамок для растягивания, которые есть у окон Windows 10/11
    if not dwmapi or dwmapi.DwmGetWindowAttribute(
            hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, ctypes.byref(rect), ctypes.sizeof(rect)) != 0:
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
    return rect


def is_cloaked(hwnd):
    # «спрятанные» окна: на другом виртуальном столе, свёрнутые UWP и т.п.
    if not dwmapi:
        return False
    cloaked = ctypes.c_int(0)
    dwmapi.DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, ctypes.byref(cloaked), ctypes.sizeof(cloaked))
    return cloaked.value != 0


def is_fullscreen(hwnd, rect):
    mi = MONITORINFO()
    mi.cbSize = ctypes.sizeof(MONITORINFO)
    if not user32.GetMonitorInfoW(user32.MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), ctypes.byref(mi)):
        return False
    m = mi.rcMonitor
    return rect.left <= m.left and rect.top <= m.top and rect.right >= m.right and rect.bottom >= m.bottom


def list_windows():
    """Видимые окна снизу вверх по стопке — в том же формате, что шлёт KWin-скрипт."""
    fg = user32.GetForegroundWindow()
    out = []
    buf = ctypes.create_unicode_buffer(512)

    def visit(hwnd, _):
        if not user32.IsWindowVisible(hwnd) or user32.IsIconic(hwnd):
            return True
        if user32.GetWindow(hwnd, GW_OWNER) or user32.GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW:
            return True
        if user32.GetWindowTextLengthW(hwnd) == 0 or is_cloaked(hwnd):
            return True
        user32.GetClassNameW(hwnd, buf, 512)
        if buf.value in SKIP_CLASSES:
            return True
        pid, exe = exe_name(hwnd)
        if pid == OWN_PID:
            return True
        user32.GetWindowTextW(hwnd, buf, 512)
        title = buf.value
        r = window_rect(hwnd)
        out.append({"id": str(hwnd), "x": r.left, "y": r.top,
                    "w": r.right - r.left, "h": r.bottom - r.top,
                    "cls": exe, "df": "", "cap": title,
                    "fs": is_fullscreen(hwnd, r), "act": hwnd == fg})
        return True

    user32.EnumWindows(EnumWindowsProc(visit), 0)
    out.reverse()  # EnumWindows идёт сверху вниз, а нам нужно снизу вверх
    return out


# ---------------------------------------------------------------- видео

def media_thread(world):
    try:
        from winrt.windows.media.control import (
            GlobalSystemMediaTransportControlsSessionManager as Manager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status)
    except ImportError:
        return  # пакеты winrt не стоят — просто без подглядывания

    async def loop():
        manager = await Manager.request_async()
        while True:
            keys = []
            try:
                for s in manager.get_sessions():
                    if s.get_playback_info().playback_status != Status.PLAYING:
                        continue
                    app_id = (s.source_app_user_model_id or "").lower()
                    key = app_id.split("!")[0].removesuffix(".exe")
                    if any(fid in key for fid in FIREFOX_IDS):
                        key = "firefox"
                    if key and not any(a in key for a in AUDIO_PLAYERS):
                        keys.append(key)
            except OSError:
                pass
            world.playing = keys
            await asyncio.sleep(1.5)

    try:
        asyncio.run(loop())
    except Exception:  # noqa: BLE001 — без панели медиа (старая Windows, Wine) живём без видео
        world.playing = []
