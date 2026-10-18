"""Linux: окна узнаём через KWin-скрипт (только KDE Plasma), видео — через MPRIS.

Без KDE список окон просто пустой — маскот гуляет по низу экрана.
"""
import json
import os

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

    media = QTimer()
    media.timeout.connect(lambda: poll_media(world))
    media.start(1500)
    poll_media(world)
    _keep.extend([bridge, media])
    return True


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
