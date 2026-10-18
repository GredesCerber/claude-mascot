"""Linux: окна узнаём через KWin-скрипт (только KDE Plasma), видео — через MPRIS.

Без KDE список окон просто пустой — маскот гуляет по низу экрана.
"""
import fcntl
import json
import os
import struct

from PyQt6.QtCore import QObject, QTimer, pyqtClassInfo, pyqtSlot
from PyQt6.QtDBus import (QDBusConnection, QDBusInterface, QDBusMessage,
                          QDBusVariant)

HERE = os.path.dirname(os.path.abspath(__file__))
SERVICE = "org.claude.Mascot"
KWIN_PLUGIN = "claude-mascot-windows"

AUDIO_PLAYERS = {"spotify", "elisa", "rhythmbox", "strawberry", "audacious", "amarok",
                 "clementine", "lollypop", "cmus", "deadbeef", "tidal-hifi", "kdeconnect",
                 "yandex-music", "quodlibet", "gnome-music"}

_keep = []  # держим ссылки, чтобы сборщик мусора не убил мост и таймер
_pointer = None  # виртуальный указатель для удара по курсору (None — ещё не создан, False — нельзя)


def setup_env():
    # На Wayland окна не могут сами себя двигать — идём через XWayland.
    os.environ.setdefault("QT_QPA_PLATFORM", "xcb")


def window_flags():
    from PyQt6.QtCore import Qt
    # override-redirect: окно поверх всего, в т.ч. полноэкранного видео
    return Qt.WindowType.X11BypassWindowManagerHint


def start(app, world):
    """Подключает источники данных о мире. False — маскот уже запущен."""
    bus = QDBusConnection.sessionBus()
    if not bus.registerService(SERVICE):
        return False

    @pyqtClassInfo("D-Bus Interface", SERVICE)
    class Bridge(QObject):
        @pyqtSlot(str)
        def update(self, payload):
            try:
                world.set_state(json.loads(payload))
            except (ValueError, KeyError, TypeError):
                pass

        @pyqtSlot(str)
        def cursor(self, payload):
            try:
                x, y = payload.split(",")
                world.cursor = (float(x), float(y))
            except ValueError:
                pass

    bridge = Bridge()
    bus.registerObject("/", bridge, QDBusConnection.RegisterOption.ExportAllSlots)
    start_kwin_script()
    app.aboutToQuit.connect(stop_kwin_script)
    app.aboutToQuit.connect(release_cursor)

    media = QTimer()
    media.timeout.connect(lambda: poll_media(world))
    media.start(1500)
    poll_media(world)
    _keep.extend([bridge, media])
    return True


class VirtualPointer:
    """Виртуальное «абсолютное» устройство-указатель через /dev/uinput — как планшет в виртуалке.

    Wayland не даёт приложениям двигать курсор, а событиям от устройства ввода KWin подчиняется:
    координата устройства 0..ширина-1 ложится на всю ширину всех мониторов вместе.
    Нужен доступ на запись к /dev/uinput (группа input; в Nobara/Fedora со Steam он обычно есть).
    """
    UI_SET_EVBIT, UI_SET_KEYBIT, UI_SET_ABSBIT = 0x40045564, 0x40045565, 0x40045567
    UI_DEV_CREATE, UI_DEV_DESTROY = 0x5501, 0x5502
    EV_SYN, EV_KEY, EV_ABS, BTN_LEFT = 0, 1, 3, 0x110

    def __init__(self):
        from PyQt6.QtCore import QRect
        from PyQt6.QtWidgets import QApplication
        area = QRect()
        for s in QApplication.screens():
            area = area.united(s.geometry())
        self.left, self.top = area.left(), area.top()
        self.w, self.h = area.width(), area.height()
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        try:
            for ev in (self.EV_KEY, self.EV_ABS, self.EV_SYN):
                fcntl.ioctl(self.fd, self.UI_SET_EVBIT, ev)
            fcntl.ioctl(self.fd, self.UI_SET_KEYBIT, self.BTN_LEFT)  # иначе это не «мышь»
            for axis in (0, 1):
                fcntl.ioctl(self.fd, self.UI_SET_ABSBIT, axis)
            absmax = [0] * 64
            absmax[0], absmax[1] = self.w - 1, self.h - 1
            os.write(self.fd, struct.pack("80sHHHHi", b"claude-mascot pointer", 0x06, 0x1234, 0x5678, 1, 0)
                     + struct.pack("64i", *absmax) + struct.pack("64i", *[0] * 64) * 3)
            fcntl.ioctl(self.fd, self.UI_DEV_CREATE)
        except OSError:
            os.close(self.fd)
            raise

    def move(self, x, y):
        x = max(0, min(self.w - 1, int(x - self.left)))
        y = max(0, min(self.h - 1, int(y - self.top)))
        ev = "llHHi"
        os.write(self.fd, struct.pack(ev, 0, 0, self.EV_ABS, 0, x) + struct.pack(ev, 0, 0, self.EV_ABS, 1, y)
                 + struct.pack(ev, 0, 0, self.EV_SYN, 0, 0))

    def close(self):
        try:
            fcntl.ioctl(self.fd, self.UI_DEV_DESTROY)
        finally:
            os.close(self.fd)


def prepare_cursor():
    """Заранее создать виртуальный указатель (системе нужно ~0.1–0.3 с, чтобы его подхватить)."""
    global _pointer
    if _pointer is None:
        try:
            _pointer = VirtualPointer()
        except OSError:
            _pointer = False  # нет доступа к /dev/uinput — курсор двигать не получится
    return bool(_pointer)


def move_cursor(x, y):
    if prepare_cursor():
        _pointer.move(x, y)
    else:
        from PyQt6.QtGui import QCursor
        QCursor.setPos(int(x), int(y))  # на Wayland сработает, только если курсор над нашим окном


def release_cursor():
    global _pointer
    if _pointer:
        _pointer.close()
    _pointer = None


def kwin_scripting():
    return QDBusInterface("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
                          QDBusConnection.sessionBus())


def start_kwin_script():
    k = kwin_scripting()
    if not k.isValid():
        return
    k.call("unloadScript", KWIN_PLUGIN)
    k.call("loadScript", os.path.join(HERE, "kwin_windows.js"), KWIN_PLUGIN)
    k.call("start")


def stop_kwin_script():
    k = kwin_scripting()
    if k.isValid():
        k.call("unloadScript", KWIN_PLUGIN)


def dbus_prop(iface, interface, name):
    msg = iface.call("Get", interface, name)
    if msg.type() != QDBusMessage.MessageType.ReplyMessage or not msg.arguments():
        return None
    v = msg.arguments()[0]
    return v.variant() if isinstance(v, QDBusVariant) else v


def poll_media(world):
    bus = QDBusConnection.sessionBus()
    reply = bus.interface().registeredServiceNames()
    names = reply.value() if reply.isValid() else []
    keys = []
    for n in names:
        if not n.startswith("org.mpris.MediaPlayer2."):
            continue
        key = n.split(".")[3].lower()
        if key in AUDIO_PLAYERS:
            continue
        props = QDBusInterface(n, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", bus)
        props.setTimeout(300)
        if dbus_prop(props, "org.mpris.MediaPlayer2.Player", "PlaybackStatus") != "Playing":
            continue
        keys.append(key)
        for prop in ("DesktopEntry", "Identity"):
            val = dbus_prop(props, "org.mpris.MediaPlayer2", prop)
            if val and str(val).lower() not in AUDIO_PLAYERS:
                keys.append(str(val).lower())
    world.playing = keys
