#!/usr/bin/env python3
"""Клод — маленький пиксельный маскот, который живёт на рабочем столе.

ЛКМ + тянуть — перетащить и швырнуть, клик — погладить,
двойной клик — прыжок, ПКМ — меню.

Гуляет по верху окон, карабкается по их краям, сидит на краю свесив ноги,
а когда играет видео — подглядывает из-за окна плеера.
Работает на Linux и Windows. Всё платформенное (откуда брать список окон и что
сейчас играет) — в platform_linux.py / platform_windows.py.
"""
import math
import os
import random
import re
import signal
import sys

if sys.platform == "win32":
    import platform_windows as platform_backend
else:
    import platform_linux as platform_backend
platform_backend.setup_env()  # до создания QApplication

from PyQt6.QtCore import QElapsedTimer, QPoint, QPointF, QRect, QRectF, QSettings, Qt, QTimer
from PyQt6.QtGui import (QAction, QColor, QCursor, QFont, QFontMetrics, QImage, QPainter,
                         QPainterPath, QPen, QRegion)
from PyQt6.QtWidgets import QApplication, QMenu, QWidget

P = 5  # размер "пикселя"
W, H = 280, 240  # размер окна
AX, AY = W // 2, H - 24  # где в окне стоят ноги

BODY = QColor("#D97757")
SHADE = QColor("#B85C3E")
LIGHT = QColor("#E8957A")
EYE = QColor("#1b1b1b")
BLUSH = QColor(255, 140, 150, 170)

# Тагилла: целиком из кадров assets/tagilla_frames.png (нарисованы GPT)
ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets")
T_SHOULDER = (11, -83)      # плечо относительно точки между ступнями (для удара по курсору)
T_HEAD_DIST = 72            # от плеча до центра бойка кувалды
T_STRIKE = (48, -12)        # куда приходится боёк в кадре «удар», от точки между ступнями

GRAVITY = 2200.0
WALK_SPEED = 55.0
RUN_SPEED = 150.0
BLOCK_CHANCE = 0.2  # шанс, что жертва поймает кувалду Тагиллы и отбросит его
IDLE_SLEEP = 60.0  # через сколько секунд бездействия пользователя маскоты засыпают
RAGE_PAUSE = (10.0, 120.0)  # Тагилла: сколько секунд спокоен между приступами буйства
RAGE_LENGTH = (15.0, 40.0)  # ...и сколько длится приступ
CLING_OFF = 26  # насколько центр маскота отстоит от края окна, когда он висит
SHAKE_OFF_HITS = 7  # сколько резких рывков туда-сюда за 1.5 с нужно, чтобы стряхнуть

IDLE_LINES = [
    "привет!", "что кодим сегодня?", "я тут просто гуляю", "*пип-пип*",
    "не забудь попить воды", "git push --force? смело", "мне скучно...",
    "погладь меня", "я Claude, кстати", "работаем!", "а тесты кто писать будет?",
    "хорошая погода на рабочем столе",
]
TOP_LINES = ["вид отсюда огонь", "я на окне!", "не закрывай это окно, ладно?", "высоко сижу"]
PICK_LINES = ["оп!", "эй!", "куда?!", "уиии!", "положи где взял", "аааа"]
OUCH_LINES = ["ой...", "больно же", "всё кружится", "за что?!"]
PET_LINES = ["хе-хе", "<3", "ещё!", "мурр", "приятно :)"]
HOLD_LINES = ["вишу!", "катаюсь на курсоре", "не отпущу", "уиии, быстрее!", "я твой брелок"]
GRIP_LINES = ["не стряхнёшь!", "держусь!", "хе-хе, неа"]
SLIP_LINES = ["аааа держусь!", "пальцы скользят!", "ну пожалуйста!"]
SHAKE_LINES = ["аааа!", "эй, не тряси!", "уаааа", "землетрясение!", "я падаю!"]
GRAB_LINES = ["хвать!", "держусь!", "фух, поймался"]
CLIMB_LINES = ["лезу-лезу", "я альпинист", "ух, высоко"]
PEEK_LINES = ["что смотрим?", "о, интересно...", "я тихонько", "ещё серию?",
              "не обращай на меня внимания", "ого", "тсс"]

LINES = {
    "idle": IDLE_LINES, "top": TOP_LINES, "pick": PICK_LINES, "ouch": OUCH_LINES,
    "pet": PET_LINES, "hold": HOLD_LINES, "grip": GRIP_LINES, "slip": SLIP_LINES,
    "shake": SHAKE_LINES, "grab": GRAB_LINES, "climb": CLIMB_LINES, "peek": PEEK_LINES,
    "catch": ["поймал!", "хвать!", "попался!"],
    "flung": ["аааааа!", "нееет!", "я ещё вернусь!"],
    "summit": ["залез!", "та-да!", "я на вершине"],
    "peek_end": ["ну всё, кончилось", "а дальше?", "эх"],
    "bonk": ["бум!", "ай!"],
    "smash": ["бах!"],
    # охота Тагиллы на других маскотов
    "spotted": ["ТАГИЛЛА!", "а?! это он!", "мама...", "опять он!"],
    "flee": ["ААА!", "помогите!", "только не молотом!", "бежим!", "я быстрый, я быстрый!"],
    "bonked": ["АЙ!", "за что?!", "ой-ой-ой", "больно же!", "ну ёлки..."],
    "dodge": ["мимо!", "не догонишь!", "хе-хе, промазал", "оп!"],
    "relief": ["фух...", "кажется, отстал", "живой!"],
    "rage": ["ГРАААА!", "стоять!"],
    "gloat": ["ха", "следующий"],
    "miss": ["мимо"],
    "taunt_up": ["слезай!"],
    "calm": ["устал"],
    "yawn": ["*зевает*", "пойду вздремну", "что-то тихо стало...", "zzz"],
    "wake": ["а? я не сплю!", "о, ты вернулся!", "*потягивается*", "доброе утро!"],
    "block": ["не в этот раз!", "ХА! поймал!", "а вот и нет!", "держи сдачи!"],
    "blocked": ["ГРА?!", "ЧТО?!", "как?!"],
}

TAGILLA_LINES = {
    "idle": ["ГРРР", "кто тут?", "*тяжело дышит*", "завод мой", "слышу шаги...",
             "молот наточен", "УБЕЙ", "где ЧВК?", "*скрежет кувалды*"],
    "top": ["сверху всё видно", "отсюда бить удобнее"],
    "pick": ["ГРА!", "отпусти!", "эй!"],
    "ouch": ["ГРРР!", "ну всё...", "больно!"],
    "pet": ["...гр", "ладно, живи", "хм", "*довольно пыхтит*"],
    "hold": ["не уйдёшь", "поймал ЧВКшника", "ГРР"],
    "grip": ["не стряхнёшь", "держу крепко"],
    "slip": ["ГРРРА!", "скользко!"],
    "shake": ["ГРАА!", "э-э-э!"],
    "grab": ["хвать", "держусь"],
    "climb": ["лезу", "доберусь"],
    "peek": ["кино...", "что смотрим?", "тсс"],
    "catch": ["попался!", "поймал!"],
    "flung": ["ГРАААА!", "я вернусь"],
    "summit": ["забрался", "теперь крушить"],
    "smash": ["БАХ!", "УБЕЙ!", "ГРРА!", "на!", "крушу!"],
    "rage": ["ГРАААА!", "УБЕЙ!", "вижу тебя!", "стоять!", "иди сюда!"],
    "gloat": ["ГРА-ХА-ХА", "ещё хочешь?", "лежать", "следующий", "*довольно пыхтит*"],
    "miss": ["ГРР! мимо", "стой!", "не уйдёшь!"],
    "taunt_up": ["слезай!", "я подожду", "ГРРР", "достану!"],
    "calm": ["*тяжело дышит*", "...устал", "ладно, живи пока", "перекур"],
    "yawn": ["*зевает в маску*", "...отдых", "ГРр... zzz"],
    "wake": ["ГРА! кто тут?", "*скрежет кувалды*", "проснулся злой"],
    "blocked": ["ГРА?!", "ЧТО?!", "как?!", "ГРРРР!!"],
}


class Skin:
    """Персонаж: как выглядит, что говорит и чем отличается поведение."""

    def __init__(self, name, title, hold_dy, mask, lines, speed=1.0, smashes=False, bubble_dy=92,
                 stride=9):
        self.name, self.title = name, title
        self.stride = stride  # сколько пикселей пути на один кадр шага
        self.bubble_dy = bubble_dy  # низ облачка с текстом над ногами; выше головы и курсора в hold
        self.hold_dy = hold_dy  # курсор держим чуть выше рук — чтобы клики проходили мимо
        self.mask = mask        # где персонаж ловит мышь (в координатах окна маскота)
        self.lines = lines
        self.speed = speed
        self.smashes = smashes  # крушит окна кувалдой


SKINS = {
    "claude": Skin("claude", "Клод", 64, QRect(AX - 60, AY - 95, 120, 118), {}),
    "tagilla": Skin("tagilla", "Тагилла", 128, QRect(AX - 100, AY - 135, 200, 158),
                    TAGILLA_LINES, speed=1.35, smashes=True, bubble_dy=146, stride=13),
}

BROWSERS = ("firefox", "chrom", "brave", "vivaldi", "opera", "edge", "librewolf", "zen",
            "yandex", "floorp", "waterfox")
VIDEO_HINT = re.compile(r"youtube|twitch|netflix|кинопоиск|rutube|vk видео|видео|video|anime|аниме", re.I)

GROUNDED = {"idle", "walk", "run", "chase", "sit", "sleep", "happy", "land", "dizzy",
            "ledge", "goto", "crouch", "smash", "hunt", "flee", "gloat", "block"}
MOVING = {"walk", "run", "chase", "goto", "hunt", "flee"}  # перебирает ногами
AIR = {"jump", "fall", "thrown", "leap"}
CALM = {"idle", "walk", "run", "sit", "land", "ledge", "sleep"}

pets = []
overlays = []  # трещины от ударов кувалдой
flings = []    # курсоры, которые сейчас летят от удара


_tagilla = None


def tagilla_frames():
    """Кадры Тагиллы: assets/tagilla_frames.png (полоса кадров, нарисовано GPT) и разметка
    assets/tagilla_frames.json: имя кадра -> [x, y, ширина, высота, x точки между ступнями]."""
    global _tagilla
    if _tagilla is None:
        import json
        img = QImage(os.path.join(ASSETS, "tagilla_frames.png")).convertToFormat(QImage.Format.Format_ARGB32)
        with open(os.path.join(ASSETS, "tagilla_frames.json"), encoding="utf-8") as f:
            frames = json.load(f)["frames"]
        walk = sorted((n for n in frames if n.startswith("walk")), key=lambda n: int(n[4:] or 0))
        _tagilla = {"img": img, "frames": frames, "walk": walk}
    return _tagilla


def saved_skin():
    name = QSettings("claude-mascot", "mascot").value("skin", "claude")
    return name if name in SKINS else "claude"


def rnd(a, b):
    return random.uniform(a, b)


def ui_font(size):
    f = QFont()
    f.setFamilies(["Segoe UI", "Noto Sans", "DejaVu Sans", "Sans"])  # Windows / Linux
    f.setPointSize(size)
    return f


# ---------------------------------------------------------------- мир вокруг

class Win:
    __slots__ = ("id", "x", "y", "w", "h", "right", "bottom", "cls", "df", "cap", "fs", "act")

    def __init__(self, d):
        self.id = d["id"]
        self.x, self.y = float(d["x"]), float(d["y"])
        self.w, self.h = float(d["w"]), float(d["h"])
        self.right, self.bottom = self.x + self.w, self.y + self.h
        self.cls, self.df, self.cap = d["cls"], d["df"], d["cap"]
        self.fs, self.act = d["fs"], d["act"]

    def contains(self, x, y):
        return self.x <= x < self.right and self.y <= y < self.bottom

    def rect(self):
        return QRect(int(self.x), int(self.y), int(self.w), int(self.h))


class World:
    windows = []  # снизу вверх по стопке
    playing = []  # ключи плееров, которые сейчас играют

    areas = []    # рабочие области мониторов без панелей (пусто — берём у Qt)
    cursor = None  # где курсор по данным платформы (None — спрашиваем у Qt)

    @classmethod
    def set_state(cls, data):
        cls.windows = [Win(d) for d in data["windows"] if d["w"] >= 120 and d["h"] >= 80]
        cls.areas = [QRect(int(a["x"]), int(a["y"]), int(a["w"]), int(a["h"])) for a in data["areas"]]

    @classmethod
    def work_area(cls, screen):
        g = screen.geometry()
        for a in cls.areas:
            if g.contains(a.center()):
                return a
        return screen.availableGeometry()

    @classmethod
    def get(cls, wid):
        for i, w in enumerate(cls.windows):
            if w.id == wid:
                return i, w
        return -1, None

    @classmethod
    def covered(cls, x, y, idx):
        return any(w.contains(x, y) for w in cls.windows[idx + 1:])

    @classmethod
    def fullscreen_active(cls):
        return next((w for w in cls.windows if w.fs and w.act), None)

    @classmethod
    def video_window(cls):
        best, best_score = None, -1
        for key in cls.playing:
            for i, w in enumerate(cls.windows):
                name = (w.cls + " " + w.df).lower()
                if key == "plasma-browser-integration" or any(b in key for b in BROWSERS):
                    hit = any(b in name for b in BROWSERS)
                else:
                    hit = key in name
                if not hit:
                    continue
                score = i + (1000 if w.act else 0) + (500 if VIDEO_HINT.search(w.cap) else 0)
                if score > best_score:
                    best, best_score = w, score
        return best


def cursor_pos():
    """Где сейчас курсор. На KDE Wayland Qt (через XWayland) видит его только над X11-окнами,
    поэтому там позицию присылает KWin-скрипт."""
    if World.cursor is not None:
        return QPointF(*World.cursor)
    return QPointF(QCursor.pos())


# ---------------------------------------------------------------- маскот

class Particle:
    def __init__(self, kind, x, y, vx=0.0, vy=0.0, life=1.0, size=1.0):
        self.kind, self.x, self.y = kind, x, y
        self.vx, self.vy = vx, vy
        self.life = self.max_life = life
        self.size = size


class CrackOverlay(QWidget):
    """Трещины на месте удара кувалдой. Прозрачны для мыши и сами тают через несколько секунд.

    Окна под ними никак не трогаем — это только картинка поверх.
    """
    SIZE = 320
    LIFE = 6.5

    def __init__(self, x, y, clip):
        super().__init__()
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint
                            | Qt.WindowType.WindowStaysOnTopHint
                            | Qt.WindowType.Tool
                            | Qt.WindowType.WindowTransparentForInput
                            | Qt.WindowType.WindowDoesNotAcceptFocus
                            | platform_backend.window_flags())
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        s = self.SIZE
        left, top = int(x - s / 2), int(y - s / 2)
        self.setGeometry(left, top, s, s)
        self.clip = clip.translated(-left, -top) if clip is not None else None
        self.cracks = self.make_cracks()
        self.life = 0.0
        self.clock = QElapsedTimer()
        self.clock.start()
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.step)
        self.timer.start(40)
        self.show()

    def make_cracks(self):
        c = self.SIZE / 2
        paths = []
        n = random.randint(8, 12)
        for i in range(n):
            a = i * 2 * math.pi / n + rnd(-0.25, 0.25)
            x, y = c, c
            pts = [QPointF(x, y)]
            walked, length = 0.0, rnd(50, 140)
            while walked < length:
                step = rnd(10, 22)
                a += rnd(-0.35, 0.35)
                x, y = x + math.cos(a) * step, y + math.sin(a) * step
                walked += step
                pts.append(QPointF(x, y))
                if random.random() < 0.18:  # ответвление
                    b = a + random.choice((-1, 1)) * rnd(0.5, 1.0)
                    r = rnd(12, 30)
                    paths.append([QPointF(x, y), QPointF(x + math.cos(b) * r, y + math.sin(b) * r)])
            paths.append(pts)
        ring = []  # кольцо вокруг точки удара
        r = rnd(14, 22)
        for k in range(n + 1):
            a = k * 2 * math.pi / n
            q = rnd(0.8, 1.2)
            ring.append(QPointF(c + math.cos(a) * r * q, c + math.sin(a) * r * q))
        paths.append(ring)
        return paths

    def step(self):
        self.life += self.clock.restart() / 1000.0
        if self.life >= self.LIFE:
            self.timer.stop()
            if self in overlays:
                overlays.remove(self)
            self.close()
        else:
            self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        if self.clip is not None:
            p.setClipRect(self.clip)
        p.setOpacity(min(1.0, (self.LIFE - self.life) / 2.0))
        c = self.SIZE / 2
        if self.life < 0.25:  # вспышка от удара
            g = 1 - self.life / 0.25
            p.setPen(Qt.PenStyle.NoPen)
            p.setBrush(QColor(255, 255, 255, int(170 * g)))
            r = 30 + self.life * 160
            p.drawEllipse(QPointF(c, c), r, r)
        for pen in (QPen(QColor(15, 15, 20, 150), 3.2), QPen(QColor(235, 245, 255, 235), 1.4)):
            p.setPen(pen)
            for pts in self.cracks:
                p.drawPolyline(pts)
        p.end()


class CursorFling(QWidget):
    """Удар кувалдой по курсору: курсор улетает по дуге и отскакивает от краёв экрана.

    Само окно — только след от курсора, прозрачное для мыши. Двигает курсор платформа:
    на Windows — SetCursorPos, на Linux — виртуальный указатель через /dev/uinput
    (Wayland не даёт приложениям двигать курсор иначе).
    """
    SIZE = 360
    FLIGHT = 0.65

    def __init__(self, start):
        super().__init__()
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint
                            | Qt.WindowType.WindowStaysOnTopHint
                            | Qt.WindowType.Tool
                            | Qt.WindowType.WindowTransparentForInput
                            | Qt.WindowType.WindowDoesNotAcceptFocus
                            | platform_backend.window_flags())
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        self.resize(self.SIZE, self.SIZE)
        platform_backend.prepare_cursor()
        self.p = [start.x(), start.y()]
        self.v = None       # None — ещё не ударили, просто стоим под курсором
        self.wait = 0.0     # пауза после удара (чтобы система успела подхватить устройство)
        self.life = 0.0
        self.flight = 0.0
        self.trail = []
        self.center_on()
        self.show()
        self.clock = QElapsedTimer()
        self.clock.start()
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.step)
        self.timer.start(16)
        flings.append(self)

    def center_on(self):
        self.move(int(self.p[0] - self.SIZE / 2), int(self.p[1] - self.SIZE / 2))

    def launch(self, vx, vy, wait=0.0):
        self.v = [vx, vy]
        self.wait = wait

    def finish(self):
        self.timer.stop()
        if self in flings:
            flings.remove(self)
        self.close()

    def step(self):
        dt = max(0.004, min(self.clock.restart() / 1000.0, 0.05))  # 0 бывает, когда кадры слиплись
        self.life += dt
        if self.v is None:  # ждём удара, держимся под курсором
            c = cursor_pos()
            self.p = [c.x(), c.y()]
            self.center_on()
            if self.life > 1.5:
                self.finish()
            return
        if self.wait > 0:
            self.wait -= dt
            return
        self.flight += dt
        self.v[1] += 2600 * dt
        self.p[0] += self.v[0] * dt
        self.p[1] += self.v[1] * dt
        area = QRect()
        for s in QApplication.screens():
            area = area.united(s.geometry())
        if not area.left() + 2 <= self.p[0] <= area.right() - 2:
            self.p[0] = max(area.left() + 2.0, min(area.right() - 2.0, self.p[0]))
            self.v[0] = -self.v[0] * 0.5
        if self.p[1] > area.bottom() - 2:
            self.p[1] = area.bottom() - 2.0
            self.v[1] = -self.v[1] * 0.35
            self.v[0] *= 0.7
        elif self.p[1] < area.top() + 2:
            self.p[1] = area.top() + 2.0
            self.v[1] = abs(self.v[1]) * 0.3
        self.trail.append((self.p[0], self.p[1], self.flight))
        self.trail = self.trail[-14:]
        platform_backend.move_cursor(self.p[0], self.p[1])
        World.cursor = (self.p[0], self.p[1])
        self.center_on()
        self.update()
        if self.flight >= self.FLIGHT:
            self.finish()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        ox, oy = self.x(), self.y()
        prev = None
        for x, y, t in self.trail:  # след от улетающего курсора
            k = max(0.0, 1 - (self.flight - t) / 0.25)
            if prev and k > 0:
                p.setPen(QPen(QColor(255, 255, 255, int(170 * k)), 3 * k + 0.5))
                p.drawLine(QPointF(prev[0] - ox, prev[1] - oy), QPointF(x - ox, y - oy))
            prev = (x, y)
        if self.v is not None and self.flight < 0.3:
            for i in range(4):  # звёздочки от удара
                a = self.flight * 9 + i * 1.57
                p.save()
                p.setOpacity(1 - self.flight / 0.3)
                self.parent_star(p, self.p[0] - ox + math.cos(a) * 18, self.p[1] - oy + math.sin(a) * 10)
                p.restore()
        p.end()

    @staticmethod
    def parent_star(p, x, y, r=4):
        path = QPainterPath()
        for i in range(10):
            a = -math.pi / 2 + i * math.pi / 5
            rr = r if i % 2 == 0 else r * 0.45
            pt = QPointF(x + math.cos(a) * rr * 1.6, y + math.sin(a) * rr * 1.6)
            path.moveTo(pt) if i == 0 else path.lineTo(pt)
        path.closeSubpath()
        p.fillPath(path, QColor("#FFD84A"))


class Pet(QWidget):
    def __init__(self, x=None, skin=None):
        super().__init__()
        self.skin = SKINS[skin or saved_skin()]
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint
                            | Qt.WindowType.WindowStaysOnTopHint
                            | Qt.WindowType.Tool
                            | Qt.WindowType.WindowDoesNotAcceptFocus
                            | platform_backend.window_flags())
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        self.setWindowTitle("Claude mascot")
        self.resize(W, H)

        geo = self.screen_geo(None)
        self.x = x if x is not None else rnd(geo.left() + 100, geo.right() - 100)
        self.y = geo.top() - 20.0  # падаем сверху
        self.vx, self.vy = rnd(-120, 120), 0.0
        self.facing = random.choice((-1, 1))
        self.state = "fall"
        self.state_t = 0.0
        self.state_len = 0.0
        self.t = rnd(0, 100)
        self.leg_phase = 0.0
        self.squash = 0.0
        self.squash_v = 0.0
        self.tilt = 0.0
        self.blink = 0.0
        self.next_blink = rnd(1, 4)
        self.next_talk = rnd(15, 40)
        self.eye_dx = 0.0
        self.eye_dy = 0.0
        self.chase = False
        self.dizzy_next = False
        self.particles = []
        self.bubble = None
        self.bubble_t = 0.0
        self.press_pos = None
        self.drag_off = QPointF()
        self.mouse_hist = []
        self.last_mask = None
        self.z_timer = 0.0
        self.hidden = False

        self.surface = None   # id окна, на котором стоим (None — пол)
        self.surf_pos = None  # где было это окно в прошлом кадре
        self.ignore = None    # (id окна, до какого t) — не приземляться обратно
        self.wv = (0.0, 0.0)  # сглаженная скорость окна под нами
        self.shake_dir = 0.0
        self.shake_hits = []
        self.lift = 0.0       # недавняя пиковая скорость подъёма окна
        self.chase_v = 0.0    # скорость бега за курсором
        self.chase_cd = 0.0   # пауза между прыжками в погоне
        self.hammer_a = -15.0  # угол руки с кувалдой (0 — вниз, 180 — вверх, 270 — вперёд)
        self.smash = None
        self.shake_t = 0.0
        self.prey = None       # Тагилла: за кем гонится
        self.hunt_cd = 4.0     # Тагилла: пауза перед следующей охотой
        self.hit_result = None  # Тагилла: чем кончился последний удар по маскоту
        self.spin = 0.0        # кувыркается в полёте после удара, град/с
        self.bounces = 0       # сколько раз ещё отскочит от пола
        self.cursor_fling = None      # Тагилла: окно-ловушка под курсором перед ударом по нему
        self.rage_until = 0.0  # Тагилла: до какого self.t длится приступ буйства
        self.next_rage = None  # ...и когда начнётся следующий
        self.rage_forever = False  # буйство без перерыва (галочка в меню)
        self.idle_sleep = False  # спит, потому что пользователь отошёл
        self.grab_cd = 0.0    # пауза, прежде чем снова схватить курсор
        self.hold_prev = None  # прошлая позиция курсора, когда висим на нём
        self.cv = (0.0, 0.0)  # сглаженная скорость курсора
        self.swing = 0.0      # угол раскачивания на курсоре
        self.swing_v = 0.0
        self.shake_sign = [0.0, 0.0]
        self.grip_hits = []
        self.grip_said = 0
        self.cling = None     # {"wid", "dir", "mode", "dy"}
        self.goal = None      # куда идём / прыгаем и что делать по прибытии
        self.leap = None      # (tx, ty, действие по прибытии)
        self.peek = None      # цель подглядывания
        self.peek_amt = 1.0
        self.peek_duck = 0.0
        self.peek_cd = 3.0

        self.clock = QElapsedTimer()
        self.clock.start()
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.tick)
        self.timer.start(16)
        self.place()
        self.show()

    # ---------- геометрия ----------
    def screen_of(self, x, y):
        return QApplication.screenAt(QPoint(int(x), int(y))) or QApplication.primaryScreen()

    def screen_geo(self, pt):
        if pt is None:
            return World.work_area(QApplication.primaryScreen())
        return World.work_area(self.screen_of(pt.x(), pt.y()))

    def virtual_geo(self):
        r = QRect()
        for s in QApplication.screens():
            r = r.united(World.work_area(s))
        return r

    def floor_y(self):
        probe = QPointF(self.x, min(self.y, self.virtual_geo().bottom() - 5))
        return self.screen_geo(probe).bottom() + 1.0

    def surface_win(self):
        if self.surface is None:
            return -1, None
        return World.get(self.surface)

    def ground_y(self):
        _, w = self.surface_win()
        return w.y if w else self.floor_y()

    def bounds(self):
        _, w = self.surface_win()
        if w:
            return w.x + 18, w.right - 18
        g = self.screen_geo(QPointF(self.x, self.floor_y() - 10))
        return g.left() + 45.0, g.right() - 45.0

    def platform_ok(self, idx, w, x):
        if w.fs or not (w.x + 4 <= x <= w.right - 4):
            return False
        if w.y < World.work_area(self.screen_of(x, w.y)).top() + 80:
            return False
        return not World.covered(x, w.y + 3, idx)

    def edge_x(self, w, d):
        return w.x if d == 1 else w.right

    # ---------- состояния ----------
    def set_state(self, st, length=0.0):
        self.state, self.state_t, self.state_len = st, 0.0, length

    def pick_next(self):
        if self.skin.smashes:
            result, self.hit_result = self.hit_result, None
            if result == "hit":
                self.set_state("gloat", 1.6)
                self.hunt_cd = 3.2  # даёт жертве очухаться и чуть-чуть форы
                self.say(self.line("gloat"), 1.6)
                return
            if self.raging() and self.hunt_cd <= 0 and self.hunt_target():
                self.start_hunt()
                return
        elif self.hunter_near():
            self.start_flee()
            return
        if self.chase:
            self.set_state("chase")
            return
        if self.skin.smashes and random.random() < 0.35 and self.plan_smash():
            return
        roll = random.random()
        if roll < 0.17 and self.plan_explore():
            return
        if self.surface is not None and roll < 0.25:
            self.jump_down()
            return
        if roll < 0.35:
            self.set_state("idle", rnd(1.5, 4))
        elif roll < 0.62:
            self.facing = random.choice((-1, 1))
            if self.skin.smashes and roll < 0.5:
                self.set_state("run", rnd(1.5, 3))  # Тагилла не гуляет, а носится
            else:
                self.set_state("walk", rnd(3, 8))
        elif roll < 0.72:
            self.facing = random.choice((-1, 1))
            self.set_state("run", rnd(1, 2.5))
        elif roll < 0.83:
            self.set_state("sit", rnd(4, 9))
        elif roll < 0.91:
            self.jump(rnd(500, 750), rnd(-80, 80))
        else:
            self.set_state("sit", rnd(5, 10))  # спят только когда пользователь отошёл

    def jump(self, power, vx=0.0):
        self.leave_surface()
        self.vy = -power
        self.vx = vx
        self.squash = 0.25
        self.set_state("jump")

    def leave_surface(self, ignore=0.0):
        if self.surface is not None and ignore:
            self.ignore = (self.surface, self.t + ignore)
        self.surface = None
        self.surf_pos = None

    def drop(self, push=0.0):
        self.leave_surface()
        self.cling = None
        self.peek = None
        self.vx, self.vy = push, 0.0
        self.set_state("fall")

    def step_off(self):
        _, w = self.surface_win()
        if w:
            self.x = (w.right + 6) if self.facing > 0 else (w.x - 6)
        self.leave_surface(0.5)
        self.vx, self.vy = self.facing * 100, -160
        self.set_state("fall")

    def jump_down(self):
        _, w = self.surface_win()
        if not w:
            return
        d = 1 if (w.right - self.x) < (self.x - w.x) else -1
        tx = (w.right + rnd(50, 150)) if d > 0 else (w.x - rnd(50, 150))
        self.facing = d
        self.crouch_then_leap(tx, self.floor_y(), None)

    def goto(self, x, action):
        self.goal = (x, action)
        self.set_state("goto")

    def crouch_then_leap(self, tx, ty, arrive):
        self.goal = (tx, ("leap", tx, ty, arrive))
        self.facing = 1 if tx > self.x else -1
        self.set_state("crouch", 0.22)

    def leap_to(self, tx, ty, arrive=None, extra=None):
        h = self.y - ty
        apex = max(h, 0) + (extra if extra is not None else rnd(50, 90))
        vy0 = -math.sqrt(2 * GRAVITY * apex)
        t_up = -vy0 / GRAVITY
        t_down = math.sqrt(2 * max(apex - h, 1) / GRAVITY)
        self.leave_surface(t_up + t_down + 0.1)  # не приземляться обратно, пока летим
        self.vx = (tx - self.x) / (t_up + t_down)
        self.vy = vy0
        self.leap = (tx, ty, arrive)
        self.squash = 0.3
        self.set_state("leap")

    def plan_explore(self):
        cands = []
        for idx, w in enumerate(World.windows):
            if w.id == self.surface or w.fs or abs((w.x + w.w / 2) - self.x) > 1500:
                continue
            xs = [w.x + w.w * f for f in (0.15, 0.3, 0.5, 0.7, 0.85)]
            xs = [x for x in xs if self.platform_ok(idx, w, x)]
            if xs:
                cands.append((idx, w, xs))
        if not cands:
            return False
        idx, w, xs = random.choice(cands)
        ground = self.ground_y()
        # с пола к высокому окну — лезем по боковому краю
        if self.surface is None and ground - w.y > 200 and w.bottom >= ground - 40 \
                and random.random() < 0.6:
            d = 1 if self.x < w.x + w.w / 2 else -1
            ex = self.edge_x(w, d)
            lo, hi = self.bounds()
            if lo < ex - d * CLING_OFF < hi and not World.covered(ex - d * 3, ground - 40, idx):
                self.goto(ex - d * CLING_OFF, ("climb", w.id, d))
                return True
        self.crouch_then_leap(random.choice(xs), w.y, None)
        return True

    def track_window(self, w, dt):
        """Следит за скоростью окна, на котором сидим; трясут или дёргают — слетаем."""
        prev = self.surf_pos
        self.surf_pos = (w.x, w.y)
        if prev is None or dt <= 0:
            self.reset_motion()
            return False
        ox, oy = self.wv
        nx = ox + ((w.x - prev[0]) / dt - ox) * 0.5
        ny = oy + ((w.y - prev[1]) / dt - oy) * 0.5
        self.wv = (nx, ny)
        self.lift = min(ny, self.lift + 2500 * dt)
        if abs(nx) > 300:
            if self.shake_dir and (nx > 0) != (self.shake_dir > 0):
                self.shake_hits.append(self.t)
            self.shake_dir = nx
        self.shake_hits = [t for t in self.shake_hits if self.t - t < 0.9]
        if len(self.shake_hits) >= 2 or abs(nx) > 1700:
            fling = (nx, min(ny, 0.0) - rnd(250, 450))   # трясут / резко дёрнули вбок
        elif ny > 1100:
            fling = (nx * 0.5, 0.0)                       # окно ушло вниз из-под ног
        elif self.lift < -650 and ny > -150:
            fling = (nx, self.lift)                       # резко подняли и остановили — подкинуло
        else:
            return False
        self.fling(*fling)
        return True

    def reset_motion(self):
        self.wv = (0.0, 0.0)
        self.shake_hits = []
        self.lift = 0.0

    def fling(self, vx, vy):
        self.leave_surface(0.5)
        self.cling = None
        self.vx = max(-2500, min(2500, vx))
        self.vy = max(-2500, min(2500, vy))
        self.squash = -0.2
        self.dizzy_next = math.hypot(self.vx, self.vy) > 1600
        self.set_state("thrown")
        self.say(self.line("shake"), 1.5)

    def start_cling(self, wid, d, mode):
        _, w = World.get(wid)
        if not w:
            return False
        self.leave_surface()
        self.cling = {"wid": wid, "dir": d, "mode": mode, "dy": min(self.y - w.y, w.h - 3)}
        self.surf_pos = (w.x, w.y)
        self.reset_motion()
        self.facing = d
        self.vx = self.vy = 0.0
        self.tilt = 0
        self.squash = -0.1
        self.set_state("cling", rnd(0.8, 2.5))
        return True

    def edge_grab_ok(self, idx, w, d, y):
        ex = self.edge_x(w, d)
        return (not w.fs and w.y + 55 < y < w.bottom - 2
                and not World.covered(ex - d * 3, y - 30, idx))

    def try_cling_near(self):
        for idx in range(len(World.windows) - 1, -1, -1):
            w = World.windows[idx]
            for d in (1, -1):
                anchor = self.edge_x(w, d) - d * CLING_OFF
                if abs(self.x - anchor) < 28 and self.edge_grab_ok(idx, w, d, self.y):
                    self.x = anchor
                    return self.start_cling(w.id, d, "hang")
        return False

    def try_grab_in_flight(self, prev_x):
        for idx in range(len(World.windows) - 1, -1, -1):
            w = World.windows[idx]
            if self.ignore and w.id == self.ignore[0] and self.t < self.ignore[1]:
                continue
            for d in (1, -1):
                anchor = self.edge_x(w, d) - d * CLING_OFF
                if (prev_x - anchor) * (self.x - anchor) > 0 or self.vx * d <= 0:
                    continue
                if not self.edge_grab_ok(idx, w, d, self.y):
                    continue
                if random.random() < 0.45:
                    self.x = anchor
                    self.start_cling(w.id, d, "hang")
                    self.say(self.line("grab"), 1.4)
                return

    def find_platform(self, y0, y1, x):
        best = None
        for idx, w in enumerate(World.windows):
            if self.ignore and w.id == self.ignore[0] and self.t < self.ignore[1]:
                continue
            if y0 <= w.y <= y1 and self.platform_ok(idx, w, x):
                if best is None or w.y < best.y:
                    best = w
        return best

    def land(self, ground, win):
        impact = self.vy
        self.y = ground
        self.leap = None
        if self.bounces > 0 and impact > 250:
            # отскакивает от пола, как мячик
            self.bounces -= 1
            self.vy = -impact * 0.55
            self.vx *= 0.7
            self.spin *= 0.6
            self.squash = 0.5
            self.dust(6)
            self.set_state("thrown")
            return
        self.tilt = 0
        self.spin = 0.0
        self.bounces = 0
        if impact > 1300 and self.state not in ("jump", "leap"):
            self.vy = -impact * 0.35
            self.vx *= 0.6
            self.squash = 0.45
            self.dust(12)
            self.set_state("thrown")
            if random.random() < 0.7:
                self.say(self.line("ouch"), 1.8)
            self.dizzy_next = True
            return
        self.vx = self.vy = 0
        self.surface = win.id if win else None
        self.surf_pos = (win.x, win.y) if win else None
        self.reset_motion()
        self.squash = min(0.45, impact / 2500)
        self.dust(6 if impact > 500 else 3)
        if self.dizzy_next:
            self.dizzy_next = False
            self.set_state("dizzy", 2.2)
        else:
            self.set_state("land", 0.12 if self.chase else 0.35)
            if win and random.random() < 0.25:
                self.say(self.line("top"), 2.2)

    def arrive(self, action):
        kind = action[0]
        if kind == "leap":
            _, tx, ty, then = action
            self.leap_to(tx, ty, then)
        elif kind == "climb":
            if self.start_cling(action[1], action[2], "climb") and random.random() < 0.5:
                self.say(self.line("climb"), 1.8)
            elif self.state != "cling":
                self.set_state("idle", 1)
        elif kind == "peek":
            self.enter_peek()
        elif kind == "smash":
            self.facing = action[2]
            self.start_smash("side", action[1])

    # ---------- подглядывание за видео ----------
    def peek_target(self, vw):
        scr = self.screen_of(vw.x + vw.w / 2, vw.y + vw.h / 2)
        full, avail = scr.geometry(), World.work_area(scr)
        others = [p for p in pets if p is not self and p.peek]
        if others:
            pref = -others[0].peek["dir"]
        else:
            pref = 1 if self.x < vw.x + vw.w / 2 else -1
        if vw.fs:
            edge = full.left() if pref == 1 else full.right() + 1
            return {"dir": pref, "edge": edge, "y": full.bottom() + 1.0, "hang": False,
                    "clip": None, "scr": full}
        floor = avail.bottom() + 1.0
        for d in (pref, -pref):
            ex = self.edge_x(vw, d)
            room = (ex - avail.left()) if d == 1 else (avail.right() + 1 - ex)
            if room < 50 or vw.h < 110:
                continue
            if vw.bottom >= floor - 12:
                y, hang = floor, False
            else:
                y, hang = vw.y + min(vw.h * 0.65, vw.h - 5), True
            return {"dir": d, "edge": ex, "y": y, "hang": hang, "clip": vw.rect(), "scr": avail}
        # места сбоку нет (развёрнуто) — выглядываем из-за края экрана
        edge = avail.left() if pref == 1 else avail.right() + 1
        y = min(vw.bottom, floor)
        return {"dir": pref, "edge": edge, "y": y, "hang": y < floor - 12,
                "clip": None, "scr": avail}

    def peek_x(self, t, amt):
        d = t["dir"]
        if t["clip"] is None:
            # за краем экрана: наружу торчит перед с глазом
            return t["edge"] + d * (-6 * amt - 36 * (1 - amt))
        # за окном: прячется передом, видна спина и глаза у края
        return t["edge"] + d * (-24 * amt + 22 * (1 - amt))

    def start_peek(self, vw):
        if self.state == "sleep":
            self.say("о, видосик!", 1.8)
        t = self.peek_target(vw)
        self.peek = t
        tx = self.peek_x(t, 1.0)
        if not t["hang"] and abs(self.ground_y() - t["y"]) < 4 and \
                self.screen_of(self.x, self.y - 5) == self.screen_of(tx, t["y"] - 5):
            self.goto(tx, ("peek",))
        else:
            self.crouch_then_leap(tx, t["y"], "peek")

    def enter_peek(self):
        vw = World.video_window()
        if not vw:
            self.peek = None
            self.set_state("idle", 1)
            return
        self.peek = self.peek_target(vw)
        self.leave_surface()
        self.peek_amt = 1.0
        self.peek_duck = 0.0
        self.vx = self.vy = 0
        self.set_state("peek")
        if random.random() < 0.5:
            self.say(self.line("peek"), 2.2)

    def end_peek(self):
        hang = self.peek and self.peek["hang"]
        self.peek = None
        self.peek_cd = 4
        if random.random() < 0.4:
            self.say(self.line("peek_end"), 1.8)
        if hang:
            self.drop()
        else:
            self.set_state("idle", 2)

    def update_visibility(self):
        fs = World.fullscreen_active()
        hide = fs is not None and World.video_window() is not fs
        if hide and not self.hidden:
            self.hidden = True
            self.hide()
        elif not hide and self.hidden:
            self.hidden = False
            self.show()

    def line(self, kind):
        return random.choice(self.skin.lines.get(kind) or LINES[kind])

    def say(self, text, dur=2.6):
        self.bubble = text
        self.bubble_t = dur

    def burst(self, kind, n, life=1.0):
        for _ in range(n):
            self.particles.append(Particle(
                kind, self.x + rnd(-25, 25), self.y - rnd(40, 60),
                rnd(-40, 40), rnd(-90, -40), life * rnd(0.8, 1.2), rnd(0.8, 1.3)))

    def dust(self, n=8):
        for i in range(n):
            d = -1 if i % 2 else 1
            self.particles.append(Particle(
                "dust", self.x + d * rnd(10, 30), self.y - 3,
                d * rnd(40, 110), rnd(-30, -5), rnd(0.35, 0.6), rnd(0.8, 1.4)))

    # ---------- главный цикл ----------
    def tick(self):
        dt = max(0.004, min(self.clock.restart() / 1000.0, 0.05))  # 0 бывает, когда кадры слиплись
        self.update_visibility()
        if self.hidden:
            return
        self.t += dt
        self.state_t += dt
        self.peek_cd -= dt
        self.grab_cd -= dt
        cur = cursor_pos()

        # моргание
        self.next_blink -= dt
        if self.next_blink <= 0:
            self.blink = 0.13
            self.next_blink = rnd(2, 6)
        self.blink = max(0.0, self.blink - dt)

        # глаза следят за курсором (а при подглядывании — за видео)
        dx, dy = cur.x() - self.x, cur.y() - (self.y - 45)
        tx = max(-1.0, min(1.0, dx / 150)) * self.facing
        ty = max(-1.0, min(1.0, dy / 150))
        if self.state == "peek":
            tx, ty = 1.0, 0.2
        self.eye_dx += (tx - self.eye_dx) * min(1, dt * 8)
        self.eye_dy += (ty - self.eye_dy) * min(1, dt * 8)

        # пружинка сплющивания
        self.squash_v += (-self.squash * 260 - self.squash_v * 14) * dt
        self.squash += self.squash_v * dt

        # едем вместе с окном, на котором стоим
        if self.state in GROUNDED and self.surface is not None:
            idx, w = self.surface_win()
            if w is None or not self.platform_ok(idx, w, self.x):
                self.drop()
            else:
                if self.surf_pos:
                    self.x += w.x - self.surf_pos[0]
                self.track_window(w, dt)

        self.update_idle_sleep()

        # Тагилла видит другого маскота — охота; тот видит Тагиллу — бежать
        self.hunt_cd -= dt
        if not self.idle_sleep:
            self.update_rage()
        if not self.idle_sleep and (self.state in CALM or self.state in ("chase", "peek")):
            if self.skin.smashes:
                if self.raging() and self.hunt_cd <= 0 and self.hunt_target():
                    self.start_hunt()
            elif self.hunter_near():
                self.start_flee()

        # заметили видео — идём подглядывать (видео смотрят и когда пользователь «бездействует»)
        if self.state in CALM and self.peek_cd <= 0:
            vw = World.video_window()
            if vw and not vw.fs or vw and vw.act:
                self.start_peek(vw)
            else:
                self.peek_cd = 1.0

        st = self.state
        lo, hi = self.bounds()
        ground = self.ground_y()

        if st == "chase":
            self.tick_chase(dt, cur)

        elif st == "hunt":
            self.tick_hunt(dt)

        elif st == "flee":
            self.tick_flee(dt)

        elif st in ("walk", "run", "goto"):
            speed = WALK_SPEED if st == "walk" else RUN_SPEED
            if st == "goto":
                gap = self.goal[0] - self.x
                if abs(gap) < 6:
                    self.x = self.goal[0]
                    self.arrive(self.goal[1])
                    speed = 0
                else:
                    self.facing = 1 if gap > 0 else -1
                    speed = RUN_SPEED if abs(gap) > 250 else WALK_SPEED * 1.6
            if self.state == st:
                self.x += self.facing * speed * self.skin.speed * dt
                if speed:
                    self.leg_phase += dt * speed / self.skin.stride
                blocked = False
                if self.surface is not None:
                    idx, w = self.surface_win()
                    blocked = World.covered(self.x + self.facing * 25, w.y + 3, idx)
                if self.x < lo or self.x > hi or blocked:
                    self.x = max(lo, min(hi, self.x))
                    if st == "goto":
                        self.arrive(self.goal[1])
                    elif self.surface is not None:
                        self.at_edge(blocked)
                    else:
                        self.facing = -self.facing
                if self.state == st:
                    self.y = ground
                if st == "run" and random.random() < dt * 6:
                    self.particles.append(Particle("dust", self.x - self.facing * 20, self.y - 2,
                                                   -self.facing * rnd(20, 60), rnd(-25, -5), 0.4, 0.7))

        elif st == "crouch":
            self.y = ground
            self.squash = 0.28
            self.squash_v = 0
            if self.state_t >= self.state_len:
                self.arrive(self.goal[1])

        elif st in AIR:
            prev_x, prev_y = self.x, self.y
            self.vy += GRAVITY * dt
            self.x += self.vx * dt
            self.y += self.vy * dt
            vg = self.virtual_geo()
            if self.x < vg.left() + 40 or self.x > vg.right() - 40:
                self.x = max(vg.left() + 40.0, min(vg.right() - 40.0, self.x))
                if abs(self.vx) > 300:
                    self.say(self.line("bonk"), 1.2)
                    self.squash = -0.2
                self.vx = -self.vx * 0.55
            if self.y < vg.top() + 70 and self.vy < 0:
                self.y, self.vy = vg.top() + 70.0, -self.vy * 0.4
            if self.vx:
                self.facing = 1 if self.vx > 0 else -1
            if self.spin:
                self.tilt += self.spin * dt  # кувыркается после удара кувалдой
            else:
                self.tilt += (max(-20, min(20, self.vx * 0.03)) - self.tilt) * min(1, dt * 6)

            if self.chase and (self.try_hit_cursor_air(cur) if self.skin.smashes
                               else self.try_catch_cursor(cur)):
                pass
            elif st == "jump" and self.skin.smashes and self.prey and self.try_air_bonk():
                pass
            elif st == "leap" and self.leap and self.leap[2] and self.vy > 0 and self.y >= self.leap[1]:
                self.x, self.y = self.leap[0], self.leap[1]
                self.vx = self.vy = 0
                self.tilt = 0
                then, self.leap = self.leap[2], None
                self.arrive((then,))
            else:
                if st == "thrown":
                    self.try_grab_in_flight(prev_x)
                if self.state == st and self.vy > 0:
                    win = self.find_platform(prev_y, self.y, self.x)
                    floor = self.floor_y()
                    if win and win.y <= floor:
                        self.land(win.y, win)
                    elif self.y >= floor:
                        self.land(floor, None)

        elif st == "cling":
            self.tick_cling(dt)

        elif st == "peek":
            self.tick_peek(dt)

        elif st == "hold":
            self.tick_hold(dt, cur)

        elif st == "smash":
            self.y = ground
            self.tick_smash(dt)

        elif st == "drag":
            target = cur - self.drag_off
            self.x, self.y = target.x(), target.y()
            self.mouse_hist.append((self.t, cur))
            self.mouse_hist = [(tt, p) for tt, p in self.mouse_hist if self.t - tt < 0.1]
            vx = 0.0
            if len(self.mouse_hist) > 1:
                (t0, p0), (t1, p1) = self.mouse_hist[0], self.mouse_hist[-1]
                if t1 > t0:
                    vx = (p1.x() - p0.x()) / (t1 - t0)
            self.tilt += (max(-30, min(30, vx * 0.025)) - self.tilt) * min(1, dt * 10)
            self.leg_phase += dt * (4 + abs(vx) / 100)

        elif st == "sleep":
            self.y = ground
            self.z_timer -= dt
            if self.z_timer <= 0:
                self.z_timer = 1.1
                self.particles.append(Particle("z", self.x + self.facing * 18, self.y - 45,
                                               self.facing * 15, -28, 2.2, 1.0))
        elif st == "happy":
            self.y = ground - abs(math.sin(self.state_t * 9)) * 18 * max(0, 1 - self.state_t / 1.3)
        elif st == "ledge":
            self.y = ground
            self.leg_phase += dt * 2.5
            if self.state_t > self.state_len:
                self.facing = -self.facing
                self.set_state("walk", rnd(2, 5))
        else:
            self.y = ground

        if self.state_len and self.state_t > self.state_len and \
                self.state in GROUNDED and self.state not in ("crouch", "goto", "ledge"):
            self.pick_next()
        elif self.state == "idle" and not self.state_len:
            self.set_state("idle", rnd(1, 3))

        self.update_hammer(dt)
        self.shake_t = max(0.0, self.shake_t - dt)
        if self.state not in AIR and self.state not in ("drag", "peek", "hold", "smash"):
            self.tilt *= max(0, 1 - dt * 10)

        # болтовня
        if self.state not in ("sleep", "drag"):
            self.next_talk -= dt
            if self.next_talk <= 0:
                if self.state == "peek":
                    self.next_talk = rnd(40, 90)
                    self.say(self.line("peek"), 2.5)
                elif self.state == "hold":
                    self.next_talk = rnd(12, 30)
                    self.say(self.line("hold"), 2.2)
                else:
                    self.next_talk = rnd(25, 70)
                    on_top = self.surface and random.random() < 0.3
                    self.say(self.line("top" if on_top else "idle"), 3.2)
        if self.bubble:
            self.bubble_t -= dt
            if self.bubble_t <= 0:
                self.bubble = None

        # частицы
        for p in self.particles:
            p.life -= dt
            p.x += p.vx * dt
            p.y += p.vy * dt
            if p.kind == "dust":
                p.vx *= 1 - dt * 4
            elif p.kind == "heart":
                p.vx = math.sin(self.t * 4 + p.size * 10) * 30
            elif p.kind in ("shard", "sweat"):
                p.vy += 1400 * dt
        self.particles = [p for p in self.particles if p.life > 0]

        self.place()
        self.update_mask()
        self.update()

    def support_under(self, x, y):
        """Куда можно встать под точкой (x, y): (окно или None для пола, высота)."""
        best = None
        for idx, w in enumerate(World.windows):
            if w.y >= y - 15 and self.platform_ok(idx, w, x) and (best is None or w.y < best.y):
                best = w
        floor = World.work_area(self.screen_of(x, y)).bottom() + 1.0
        below = (best, best.y) if best and best.y <= floor else (None, floor)
        # курсор внутри окна — можно встать и на его верх, если это ближе
        for idx in range(len(World.windows) - 1, -1, -1):
            w = World.windows[idx]
            if w.contains(x, y):
                if self.platform_ok(idx, w, x) and abs(w.y - y) < abs(below[1] - y):
                    return w, w.y
                break
        return below

    def tick_chase(self, dt, cur):
        ground = self.ground_y()
        gap = cur.x() - self.x
        self.chase_cd -= dt

        # курсор над другой поверхностью (окно / пол) — прыгаем туда
        if self.chase_cd <= 0:
            win, top = self.support_under(cur.x(), cur.y())
            here = self.surface
            there = win.id if win else None
            if there != here and abs(top - ground) > 20 and abs(gap) < 900:
                tx = cur.x()
                if win:
                    tx = max(win.x + 20, min(win.right - 20, tx))
                self.chase_cd = 0.6
                self.chase_v = 0.0
                self.leap_to(tx, top)
                return
            # курсор прямо над головой — подпрыгиваем, чтобы поймать
            rise = ground - cur.y()
            if self.skin.smashes and self.grab_cd > 0:
                pass  # только что вмазал — сперва позлорадствовать
            elif abs(gap) < (90 if self.skin.smashes else 50) and (130 if self.skin.smashes else 40) < rise < 1600:  # хоть до верха экрана
                self.chase_cd = 1.4
                self.leave_surface(0.2)
                self.vx, self.vy = gap * 2, -math.sqrt(2 * GRAVITY * (rise - 30))
                self.squash = 0.25
                self.set_state("jump")
                return

        # бег с разгоном: чем дальше курсор, тем быстрее
        if self.skin.smashes:  # Тагилле надо встать на длину кувалды, а не под курсор
            gap = gap - math.copysign(85, gap) if abs(gap) > 85 else 0.0
        want = 0.0 if abs(gap) < 14 else math.copysign(min(abs(gap) * 3.0, 700.0 * self.skin.speed), gap)
        step = 2600 * dt
        self.chase_v += max(-step, min(step, want - self.chase_v))
        if abs(self.chase_v) > 5:
            self.facing = 1 if self.chase_v > 0 else -1
        self.x += self.chase_v * dt
        speed = abs(self.chase_v)
        self.leg_phase += dt * min(speed / self.skin.stride, 20)
        if speed > 350 and random.random() < dt * 10:
            self.particles.append(Particle("dust", self.x - self.facing * 20, self.y - 2,
                                           -self.facing * rnd(20, 60), rnd(-25, -5), 0.4, 0.7))

        if self.surface is not None:
            idx, w = self.surface_win()
            lo, hi = w.x + 18, w.right - 18
            blocked = World.covered(self.x + self.facing * 25, w.y + 3, idx)
            if self.x < lo or self.x > hi or blocked:
                self.x = max(lo, min(hi, self.x))
                if not blocked and (gap > 0) == (self.facing > 0) and abs(gap) > 30:
                    self.step_off()  # курсор за краем окна — спрыгиваем
                    return
                self.chase_v = 0.0
        else:
            vg = self.virtual_geo()  # по полу бегаем через все мониторы
            self.x = max(vg.left() + 45.0, min(vg.right() - 45.0, self.x))
            ground = self.floor_y()
        self.y = ground

        if self.skin.smashes:
            self.try_hit_cursor(cur)
        else:
            self.try_catch_cursor(cur)

    def try_hit_cursor(self, cur):
        """Тагилла в погоне: курсор на расстоянии удара — замах кувалдой."""
        if self.grab_cd > 0 or self.state != "chase":
            return False
        gap = cur.x() - self.x
        if not (30 < abs(gap) < 140 and -150 < cur.y() - self.y < 0):
            return False
        self.facing = 1 if gap > 0 else -1
        self.start_smash("cursor")
        self.cursor_fling = CursorFling(cur)  # след от курсора; заодно готовим, чем его двигать
        return True

    def try_hit_cursor_air(self, cur):
        """Тагилла подпрыгнул к курсору над головой — бьёт в прыжке."""
        if self.grab_cd > 0 or math.hypot(cur.x() - self.x, cur.y() - (self.y - 100)) > 90:
            return False
        d = 1 if cur.x() >= self.x else -1
        self.hammer_a = 230
        CursorFling(cur).launch(d * rnd(1700, 2300), -rnd(900, 1200), wait=0.08)
        self.cursor_hit_fx(cur)
        return True

    def cursor_hit_fx(self, cur):
        self.hit_result = "hit"
        self.grab_cd = 3.0
        self.shake_t = 0.3
        self.squash = 0.25
        for _ in range(8):
            self.particles.append(Particle("spark", cur.x() + rnd(-10, 10), cur.y() + rnd(-10, 10),
                                           rnd(-180, 180), rnd(-240, -60), rnd(0.4, 0.7), rnd(0.8, 1.3)))
        if random.random() < 0.7:
            self.say(self.line("smash"), 1.0)

    def try_catch_cursor(self, cur):
        if self.grab_cd > 0 or self.state == "hold":
            return False
        if math.hypot(cur.x() - self.x, cur.y() - (self.y - self.skin.hold_dy)) > 40:
            return False
        self.leave_surface()
        self.cling = self.peek = self.leap = None
        self.hold_prev = None
        self.cv = (0.0, 0.0)
        self.swing = self.swing_v = 0.0
        self.shake_sign = [0.0, 0.0]
        self.grip_hits = []
        self.grip_said = 0
        self.squash = -0.2
        self.x, self.y = cur.x(), cur.y() + self.skin.hold_dy
        self.set_state("hold")
        self.next_talk = rnd(10, 20)
        self.say(self.line("catch"), 1.5)
        return True

    def tick_hold(self, dt, cur):
        if not self.chase:
            self.grab_cd = 2
            self.fling(0.0, 0.0)
            return
        # скорость и ускорение курсора
        px, py = self.hold_prev or (cur.x(), cur.y())
        self.hold_prev = (cur.x(), cur.y())
        ovx, ovy = self.cv
        nvx = ovx + ((cur.x() - px) / dt - ovx) * 0.5
        nvy = ovy + ((cur.y() - py) / dt - ovy) * 0.5
        ax = (nvx - ovx) / dt
        self.cv = (nvx, nvy)

        # раскачивается, как маятник: курсор дёрнули вправо — тело отстаёт влево
        self.swing_v += (-60 * self.swing - 4 * self.swing_v + ax * 0.03) * dt
        self.swing = max(-70.0, min(70.0, self.swing + self.swing_v * dt))
        self.tilt = self.swing
        self.leg_phase += dt * (3 + abs(nvx) / 150)

        # тряска: резкие развороты курсора на большой скорости
        for axis, v in ((0, nvx), (1, nvy)):
            if abs(v) > 900:
                last = self.shake_sign[axis]
                if last and (v > 0) != (last > 0):
                    self.grip_hits.append(self.t)
                self.shake_sign[axis] = v
        self.grip_hits = [t for t in self.grip_hits if self.t - t < 1.5]
        hits = len(self.grip_hits)
        if hits >= SHAKE_OFF_HITS:
            self.grab_cd = 5
            self.fling(nvx * 0.9, nvy * 0.9 - 250)
            self.dizzy_next = True
            self.say(self.line("flung"), 1.8)
            return
        if hits >= SHAKE_OFF_HITS - 3 and self.grip_said < 2:
            self.grip_said = 2
            self.say(self.line("slip"), 1.5)
        elif hits >= 2 and self.grip_said < 1:
            self.grip_said = 1
            self.say(self.line("grip"), 1.5)
        elif hits == 0:
            self.grip_said = 0

        self.x, self.y = cur.x(), cur.y() + self.skin.hold_dy

    def plan_smash(self):
        """Тагилла: найти, по чему вдарить. Сбоку по окну, сверху по окну или по полу."""
        if self.surface is not None or random.random() < 0.15:
            self.start_smash("down")
            return True
        ground = self.ground_y()
        hy = ground + T_STRIKE[1]  # кувалда бьёт низко, у земли
        lo, hi = self.bounds()
        best = None
        for idx, w in enumerate(World.windows):
            if w.fs or not (w.y + 8 < hy < w.bottom - 8):
                continue
            for d in (1, -1):
                ex = self.edge_x(w, d)
                if World.covered(ex + d * 4, hy, idx):
                    continue
                stand = ex - d * T_STRIKE[0]  # отсюда боёк достаёт до края окна
                dist = abs(stand - self.x)
                if lo <= stand <= hi and dist < 1200 and (best is None or dist < best[0]):
                    best = (dist, stand, w.id, d)
        if best:
            self.goto(best[1], ("smash", best[2], best[3]))
            return True
        if random.random() < 0.5 and self.plan_explore():
            return True
        self.start_smash("down")
        return True

    def hammer_head(self, angle):
        """Где центр бойка кувалды при этом угле — относительно точки между ступнями, лицом вправо."""
        a = math.radians(angle)
        return T_SHOULDER[0] - math.sin(a) * T_HEAD_DIST, T_SHOULDER[1] + math.cos(a) * T_HEAD_DIST

    def start_smash(self, kind, wid=None):
        self.smash = {"kind": kind, "wid": wid, "hit": False}
        self.chase_v = 0.0
        self.set_state("smash", 1.2)

    def tick_smash(self, dt):
        t, sm, f = self.state_t, self.smash, self.facing
        # по живой цели бьёт быстро и с выпадом, иначе она успевает убежать
        end, wind, strike = {"side": (270, 0.45, 0.13), "down": (345, 0.45, 0.13),
                             "victim": (290, 0.22, 0.08), "cursor": (280, 0.22, 0.08)}[sm["kind"]]
        if sm["kind"] in ("victim", "cursor") and t < wind + strike:
            lo, hi = self.bounds()
            self.x = max(lo, min(hi, self.x + f * 240 * dt))
            self.leg_phase += dt * 20
        if t < wind:  # замах: кувалда уходит назад и вверх
            k = t / wind
            k = k * k * (3 - 2 * k)
            self.hammer_a = -15 + 175 * k
            self.tilt = -f * 8 * k
        elif t < wind + strike:  # удар через верх
            k = (t - wind) / strike
            self.hammer_a = 160 + (end - 160) * k
            self.tilt = f * 10 * k
        else:
            if not sm["hit"]:
                sm["hit"] = True
                self.smash_impact()
            self.hammer_a = end
            self.tilt *= max(0, 1 - dt * 6)

    def smash_impact(self):
        sm, f = self.smash, self.facing
        if sm["kind"] == "cursor":
            cur = cursor_pos()
            fling, self.cursor_fling = self.cursor_fling, None
            hx, hy = self.hammer_head(280)
            if abs(cur.x() - (self.x + f * hx)) < 95 and abs(cur.y() - (self.y + hy)) < 90:
                if fling in flings:
                    fling.launch(f * rnd(1800, 2400), -rnd(700, 1100))
                else:
                    CursorFling(cur).launch(f * rnd(1800, 2400), -rnd(700, 1100), wait=0.08)
                self.cursor_hit_fx(cur)
                return
            if fling in flings:
                fling.finish()
            self.hit_result = "miss"
            self.grab_cd = 1.0
            sm["kind"] = "down"
            if random.random() < 0.6:
                self.say(self.line("miss"), 1.2)
        if sm["kind"] == "victim":
            v = sm["victim"]
            hx = self.x + f * T_STRIKE[0]
            # выпад с замахом: достаёт всё, что перед ним на расстоянии до 140 px
            if v in pets and v.state not in AIR and v.state not in ("drag", "hold") \
                    and 0 <= (v.x - self.x) * f < 140 and abs(v.y - self.y) < 45:
                if random.random() < BLOCK_CHANCE:
                    v.block_hammer(self)  # поймал кувалду — Тагилла отлетает сам
                else:
                    self.bonk(v, f)
                return
            # промазал — кувалда в пол
            self.hit_result = "miss"
            sm["kind"] = "down"
            if v in pets and random.random() < 0.6:
                v.say(v.line("dodge"), 1.3)
            if random.random() < 0.6:
                self.say(self.line("miss"), 1.2)
        if sm["kind"] == "side":
            ox, oy = T_STRIKE
            hx, hy = self.x + f * ox, self.y + oy
            _, w = World.get(sm["wid"])
            clip = w.rect() if w else None
        else:
            hx, hy = self.x + f * T_STRIKE[0], self.y
            _, w = self.surface_win()
            # по окну — трещины по окну; по полу — по панели задач под ногами
            clip = w.rect() if w else QRect(int(hx - 200), int(hy), 400, 250)
        overlays.append(CrackOverlay(hx, hy, clip))
        for _ in range(14):
            self.particles.append(Particle("shard", hx + rnd(-8, 8), hy + rnd(-8, 4),
                                           rnd(-260, 260) - f * 60, rnd(-480, -140),
                                           rnd(0.7, 1.1), rnd(0.7, 1.4)))
        if sm["kind"] == "down":
            self.dust(8)
        self.squash = 0.25
        self.shake_t = 0.3
        if random.random() < 0.5:
            self.say(self.line("smash"), 1.2)

    # ---------- охота Тагиллы на других маскотов ----------
    def hunt_target(self):
        if not self.skin.smashes:
            return None
        prey = [p for p in pets if p is not self and not p.skin.smashes and not p.hidden]
        return min(prey, key=lambda p: abs(p.x - self.x) + abs(p.y - self.y), default=None)

    def update_idle_sleep(self):
        """Пользователь отошёл (не трогает мышь и клавиатуру) — спим; вернулся — просыпаемся."""
        idle = platform_backend.idle_seconds()
        if self.idle_sleep:
            if idle < 1.0:
                self.idle_sleep = False
                if self.state == "sleep":
                    self.set_state("idle", 1.5)
                    self.squash = -0.2
                    if random.random() < 0.6:
                        self.say(self.line("wake"), 1.8)
            elif self.state != "sleep" and self.state in CALM | {"chase", "hunt", "flee", "gloat"}:
                self.set_state("sleep")  # разбудили (видео кончилось и т.п.) — досыпаем
            return
        if idle > IDLE_SLEEP and self.state in CALM | {"chase", "hunt", "flee", "gloat"} \
                and not World.video_window():
            self.idle_sleep = True
            self.prey = None
            self.set_state("sleep")  # без длительности — до возвращения пользователя
            if random.random() < 0.5:
                self.say(self.line("yawn"), 1.8)

    def raging(self):
        return self.skin.smashes and (self.rage_forever or self.t < self.rage_until)

    def update_rage(self):
        """Приступы буйства: случайная пауза RAGE_PAUSE, потом звереет на RAGE_LENGTH."""
        if not self.skin.smashes or self.rage_forever:
            return
        if self.next_rage is None:
            self.next_rage = self.t + rnd(*RAGE_PAUSE)
        if self.t >= self.next_rage and not self.raging():
            if self.hunt_target():
                self.start_rage()
            else:
                self.next_rage = self.t + 5  # бить некого — проверим попозже

    def start_rage(self):
        self.rage_until = self.t + rnd(*RAGE_LENGTH)
        self.next_rage = self.rage_until + rnd(*RAGE_PAUSE)
        self.hunt_cd = 0.0
        self.shake_t = 0.3
        self.say(self.line("rage"), 1.4)

    def toggle_rage_forever(self, on):
        self.rage_forever = on
        if on:
            self.start_rage()
        else:
            self.rage_until = self.t  # приступ кончается прямо сейчас
            self.next_rage = self.t + rnd(*RAGE_PAUSE)

    def hunter_near(self):
        if self.skin.smashes:
            return None
        for p in pets:
            if p is not self and p.prey is self and p.state in ("hunt", "smash") \
                    and not p.hidden and abs(p.x - self.x) < 1100:
                return p
        return None

    def start_hunt(self):
        if self.state == "peek":
            self.end_peek()
            return
        self.prey = self.hunt_target()
        self.chase_v = 0.0
        self.chase_cd = 0.0
        self.set_state("hunt")
        self.say(self.line("rage"), 1.4)
        self.shake_t = 0.2

    def start_flee(self):
        if self.state == "peek":
            self.end_peek()
            return
        if self.state == "hold":
            return  # на курсоре он и так в безопасности (почти)
        self.chase_cd = 0.3
        if self.state == "sleep" or random.random() < 0.6:
            self.say(self.line("spotted"), 1.4)
        self.set_state("flee")

    def tick_hunt(self, dt):
        v = self.hunt_target()
        if v is None or not self.raging():
            if v is not None:
                self.say(self.line("calm"), 1.8)  # приступ прошёл
            self.prey = None
            self.set_state("idle", 1.5)
            return
        self.prey = v
        self.chase_cd -= dt
        ground = self.ground_y()
        gap = v.x - self.x
        if random.random() < dt * 0.2:
            self.say(self.line("rage"), 1.3)
        out_of_reach = v.state in ("hold", "cling", "drag") or (v.state == "peek" and v.peek and v.peek["hang"])
        if out_of_reach:
            # жертва на курсоре / на краю окна — встаём под ней и прыгаем
            rise = ground - (v.y - 30)
            if abs(gap) < 70 and self.chase_cd <= 0 and v.state != "drag" and 0 < rise < 700:
                self.chase_cd = 1.6
                self.jump(math.sqrt(2 * GRAVITY * (rise + 25)), gap * 2.5)
                if random.random() < 0.5:
                    self.say(self.line("taunt_up"), 1.2)
                return
        elif v.state not in AIR:
            if abs(v.y - ground) > 25 and self.chase_cd <= 0 and abs(gap) < 900:
                # жертва на другой высоте (на окне / на полу) — прыгаем туда же
                tx = v.x - math.copysign(55, gap)
                _, vw = v.surface_win()
                if vw:
                    tx = max(vw.x + 20, min(vw.right - 20, tx))
                self.chase_cd = 1.0
                self.chase_v = 0.0
                self.leap_to(tx, v.y)
                return
            if abs(v.y - ground) <= 25 and abs(gap) < 100:
                self.facing = 1 if gap > 0 else -1
                self.start_smash("victim")
                self.smash["victim"] = v
                return

        # бег к жертве — быстрее, чем она убегает
        want = 0.0 if abs(gap) < 10 else math.copysign(min(abs(gap) * 4.0, 480.0 * self.skin.speed), gap)
        step = 3000 * dt
        self.chase_v += max(-step, min(step, want - self.chase_v))
        if abs(self.chase_v) > 5:
            self.facing = 1 if self.chase_v > 0 else -1
        self.x += self.chase_v * dt
        speed = abs(self.chase_v)
        self.leg_phase += dt * min(speed / self.skin.stride, 22)
        if speed > 350 and random.random() < dt * 12:
            self.particles.append(Particle("dust", self.x - self.facing * 20, self.y - 2,
                                           -self.facing * rnd(20, 60), rnd(-25, -5), 0.4, 0.8))
        if self.surface is not None:
            idx, w = self.surface_win()
            lo, hi = w.x + 18, w.right - 18
            if self.x < lo or self.x > hi:
                self.x = max(lo, min(hi, self.x))
                if (gap > 0) == (self.facing > 0) and abs(gap) > 40:
                    self.step_off()  # жертва внизу за краем окна — спрыгиваем
                    return
                self.chase_v = 0.0
        else:
            vg = self.virtual_geo()
            self.x = max(vg.left() + 45.0, min(vg.right() - 45.0, self.x))
            ground = self.floor_y()
        self.y = ground

    def tick_flee(self, dt):
        h = self.hunter_near()
        if not h:
            if random.random() < 0.5:
                self.say(self.line("relief"), 1.5)
            self.set_state("idle", 1.5)
            return
        self.chase_cd -= dt
        ground = self.ground_y()
        away = 1 if self.x >= h.x else -1
        if self.surface is not None:
            lo, hi = self.bounds()
        else:  # по полу удираем хоть на другой монитор
            vg = self.virtual_geo()
            lo, hi = vg.left() + 45.0, vg.right() - 45.0
        dist = abs(h.x - self.x)
        cornered = (away > 0 and self.x >= hi - 25) or (away < 0 and self.x <= lo + 25)
        same_level = abs(h.y - self.y) < 30
        if self.chase_cd <= 0 and same_level and h.state == "hunt" and dist < 260:
            if cornered or random.random() < dt * 1.2:
                # перепрыгнуть через Тагиллу
                self.chase_cd = 1.4
                tx = max(lo, min(hi, h.x - away * rnd(170, 260)))
                self.leap_to(tx, ground, extra=rnd(140, 200))
                if random.random() < 0.4:
                    self.say(self.line("dodge"), 1.2)
                return
            if random.random() < dt * 0.6 and self.plan_explore():
                self.chase_cd = 1.4  # удрать на окно
                return
        if cornered and self.surface is not None:
            self.facing = away
            self.step_off()  # с окна — вниз
            return
        speed = 270.0 if dist < 400 else 170.0
        self.facing = away
        self.x = max(lo, min(hi, self.x + away * speed * dt))
        self.leg_phase += dt * min(speed / self.skin.stride, 20)
        self.y = ground
        if random.random() < dt * 3:  # пот ручьём
            self.particles.append(Particle("sweat", self.x - away * 8 + rnd(-10, 10), self.y - 55,
                                           -away * rnd(20, 60), rnd(-140, -70), 0.7, 1.0))
        if random.random() < dt * 0.25:
            self.say(self.line("flee"), 1.3)

    def try_air_bonk(self):
        """Тагилла в прыжке сбивает жертву с курсора / с края окна."""
        v = self.prey
        if v not in pets or v.state not in ("hold", "cling", "peek"):
            return False
        if math.hypot(v.x - self.x, (v.y - 30) - (self.y - 100)) > 80:
            return False
        self.hammer_a = 230
        self.bonk(v, 1 if v.x >= self.x else -1, 0.8)
        return True

    def bonk(self, v, d, power=1.0):
        self.hit_result = "hit"
        for _ in range(8):
            self.particles.append(Particle("spark", v.x + rnd(-15, 15), v.y - rnd(25, 45),
                                           rnd(-160, 160), rnd(-220, -60), rnd(0.5, 0.8), rnd(0.8, 1.3)))
        self.shake_t = 0.3
        self.squash = 0.25
        v.get_bonked(d, power)
        if random.random() < 0.6:
            self.say(self.line("smash"), 1.0)

    def block_hammer(self, hunter):
        """Жертва поймала кувалду руками и отшвырнула Тагиллу."""
        self.facing = 1 if hunter.x >= self.x else -1
        self.set_state("block", 0.9)
        self.squash = 0.35
        for _ in range(10):
            self.particles.append(Particle("spark", self.x + self.facing * 30 + rnd(-10, 10), self.y - rnd(40, 70),
                                           rnd(-200, 200), rnd(-260, -80), rnd(0.5, 0.9), rnd(0.9, 1.4)))
        self.say(self.line("block"), 1.6)
        hunter.get_knocked_back(self.facing)

    def get_knocked_back(self, d):
        """Тагиллу отбросили: летит кувырком назад, потом лежит со звёздочками."""
        self.hit_result = None
        self.hunt_cd = 4.0
        self.leave_surface(0.5)
        self.vx = d * rnd(750, 1050)
        self.vy = -rnd(550, 750)
        self.spin = d * rnd(300, 520)
        self.bounces = 1
        self.dizzy_next = True
        self.squash = -0.3
        self.shake_t = 0.4
        self.set_state("thrown")
        self.say(self.line("blocked"), 1.6)

    def get_bonked(self, d, power=1.0):
        """По этому маскоту прилетело кувалдой: улетает кувырком и отскакивает от пола."""
        if self.state == "hold":
            self.grab_cd = 4.0
        self.leave_surface(0.5)
        self.cling = self.peek = self.leap = None
        self.vx = d * rnd(800, 1200) * power
        self.vy = -rnd(650, 950) * power
        self.spin = d * rnd(540, 900)
        self.bounces = 2
        self.dizzy_next = True
        self.squash = -0.35
        self.set_state("thrown")
        self.burst("spark", 5, 0.7)
        self.say(self.line("bonked"), 1.6)

    def update_hammer(self, dt):
        if self.state == "smash":
            return
        st = self.state
        if st in MOVING:
            target = 125   # на плече, бойком назад
        elif st in AIR:
            target = 130
        elif st in ("drag", "hold", "cling"):
            target = 8 + self.tilt * 0.6  # болтается в опущенной руке
        elif st in ("happy", "gloat"):
            target = 180 + math.sin(self.t * 12) * 15  # победно над головой
        elif st == "sleep":
            target = -5
        elif st in ("sit", "ledge"):
            target = -25
        else:
            target = -15   # упёрта бойком в землю
        if self.hammer_a > 200:
            self.hammer_a -= 360  # после удара (315°) — обратно к -45°, а не через весь круг
        self.hammer_a += (target - self.hammer_a) * min(1, dt * 10)

    def at_edge(self, blocked):
        roll = random.random()
        if blocked or roll < 0.4:
            self.facing = -self.facing
        elif roll < 0.7:
            self.step_off()
        else:
            _, w = self.surface_win()
            self.x = (w.right - 6) if self.facing > 0 else (w.x + 6)
            self.set_state("ledge", rnd(5, 12))
            if random.random() < 0.3:
                self.say("посижу тут", 1.6)

    def tick_cling(self, dt):
        c = self.cling
        idx, w = World.get(c["wid"]) if c else (-1, None)
        if not w:
            self.drop()
            return
        d = c["dir"]
        ex = self.edge_x(w, d)
        if w.fs or World.covered(ex - d * 3, self.y - 30, idx):
            self.drop(-d * 60)
            return
        if self.track_window(w, dt):
            return
        if c["mode"] == "climb":
            c["dy"] -= 48 * dt
            self.leg_phase += dt * 5
        elif c["mode"] == "slide":
            c["dy"] += 40 * dt
            self.leg_phase += dt * 1.5
        else:
            self.leg_phase += dt * 2
            if self.state_t > self.state_len:
                roll = random.random()
                if roll < 0.6:
                    c["mode"] = "climb"
                elif roll < 0.85:
                    c["mode"] = "slide"
                else:
                    self.drop(-d * 80)
                    return
        self.facing = d
        self.x = ex - d * CLING_OFF
        self.y = w.y + c["dy"]
        floor = self.floor_y()
        if c["mode"] == "climb" and c["dy"] <= 52:
            if self.platform_ok(idx, w, ex + d * 24):
                self.cling = None
                self.x, self.y = ex + d * 24, w.y
                self.surface, self.surf_pos = w.id, (w.x, w.y)
                self.squash = 0.3
                self.set_state("land", 0.35)
                if random.random() < 0.4:
                    self.say(self.line("summit"), 1.8)
            else:
                c["mode"] = "hang"
                c["dy"] = 53
                self.state_t, self.state_len = 0.0, 1.5
        elif self.y >= floor:
            self.cling = None
            self.y = floor
            self.set_state("land", 0.3)
        elif self.y >= w.bottom - 1:
            self.drop()

    def tick_peek(self, dt):
        vw = World.video_window()
        if not vw:
            self.end_peek()
            return
        t = self.peek = self.peek_target(vw)
        # иногда прячемся и снова выглядываем
        self.peek_duck -= dt
        if self.peek_duck <= 0 and random.random() < dt * 0.07:
            self.peek_duck = rnd(1, 2.5)
        target = 0.15 if self.peek_duck > 0 else 1.0
        self.peek_amt += (target - self.peek_amt) * min(1, dt * 5)
        d = t["dir"]
        self.facing = d
        self.x = self.peek_x(t, self.peek_amt)
        self.y = t["y"]
        self.tilt += (d * 7 * self.peek_amt - self.tilt) * min(1, dt * 6)
        self.leg_phase += dt * 1.5

    def place(self):
        jx = jy = 0
        if self.shake_t > 0:  # отдача от удара
            jx, jy = random.randint(-3, 3), random.randint(-2, 2)
        self.move(int(round(self.x - AX)) + jx, int(round(self.y - AY)) + jy)

    def to_local(self, wx, wy):
        return wx - (round(self.x) - AX), wy - (round(self.y) - AY)

    def local_rect(self, r):
        lx, ly = self.to_local(r.x(), r.y())
        return QRect(int(lx), int(ly), r.width(), r.height())

    # ---------- маска окна, чтобы прозрачное не ловило клики ----------
    def update_mask(self):
        holding = self.state == "hold"
        if holding:
            hd = self.skin.hold_dy
            rects = [QRect(AX - 75, AY - hd + 10, 150, hd + 13)]
        else:
            rects = [QRect(self.skin.mask)]
        if self.bubble:
            rects.append(self.bubble_rect().toAlignedRect().adjusted(-3, -3, 3, 14))
        for p in ([] if holding else self.particles):
            lx, ly = self.to_local(p.x, p.y)
            rects.append(QRect(int(lx) - 14, int(ly) - 14, 28, 28))
        key = tuple((r.x(), r.y(), r.width(), r.height()) for r in rects)
        if key != self.last_mask:
            self.last_mask = key
            reg = QRegion()
            for r in rects:
                reg = reg.united(QRegion(r))
            self.setMask(reg)

    # ---------- отрисовка ----------
    def bubble_rect(self):
        f = self.bubble_font()
        br = QFontMetrics(f).boundingRect(QRect(0, 0, 190, 200), Qt.TextFlag.TextWordWrap, self.bubble)
        w, h = br.width() + 20, br.height() + 12
        bx = max(2, min(W - w - 2, AX - w / 2))
        return QRectF(bx, AY - self.skin.bubble_dy - h, w, h)

    def bubble_font(self):
        f = ui_font(10)
        f.setBold(True)
        return f

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        if self.state == "peek" and self.peek:
            # прячемся за окном с видео / за краем экрана
            reg = QRegion(self.local_rect(self.peek["scr"]))
            if self.peek["clip"] is not None:
                reg = reg.subtracted(QRegion(self.local_rect(self.peek["clip"])))
            p.setClipRegion(reg)
            self.draw_pet(p)
            p.setClipping(False)
        else:
            self.draw_pet(p)
        self.draw_particles(p)
        if self.bubble:
            self.draw_bubble(p)
        p.end()

    def draw_pet(self, p):
        st = self.state
        hanging = st in ("drag", "cling", "hold") or (st == "peek" and self.peek and self.peek["hang"])
        p.save()
        p.translate(AX, AY)

        sq = self.squash
        if st == "sleep":
            sq += math.sin(self.t * 2.2) * 0.03
        elif st in ("idle", "sit", "ledge"):
            sq += math.sin(self.t * 3) * 0.015
        sy, sx = 1 - sq, 1 + sq * 0.7

        if st in ("drag", "hold"):
            # держим за макушку / висим на курсоре: вращаем вокруг верха
            pivot = 4 - self.skin.hold_dy
            p.translate(0, pivot)
            p.rotate(self.tilt)
            p.translate(0, -pivot)
        else:
            p.rotate(self.tilt)
        p.scale(sx * self.facing, sy)
        p.setPen(Qt.PenStyle.NoPen)
        if self.skin.name == "tagilla":
            self.draw_tagilla(p, st, hanging)
        else:
            self.draw_claude(p, st, hanging)
        p.restore()

    def eye_mode(self, st):
        if st == "sleep":
            return "closed"
        if st == "happy":
            return "happy"
        if st == "dizzy":
            return "dizzy"
        if st in ("drag", "thrown", "flee"):
            return "surprised"
        if st == "block":
            return "happy"
        if st == "hold":
            return "surprised" if len(self.grip_hits) >= 3 else "happy"
        return "closed" if self.blink > 0 else "open"

    def draw_claude(self, p, st, hanging):
        def px(cx, cy, w=1, h=1, col=BODY):
            p.fillRect(QRectF(cx * P, cy * P, w * P, h * P), col)

        sitting = st in ("sit", "sleep", "dizzy", "ledge")
        walking = st in MOVING
        top = -8 if sitting else -11  # верхний ряд тела
        bob = 0
        if walking:
            bob = -0.5 if int(self.leg_phase) % 2 else 0
        top += bob

        # ноги
        legs = (-5, -3, 2, 4)
        if hanging:
            for i, lx in enumerate(legs):
                sw = math.sin(self.leg_phase * 2 + i * 1.3) * 0.5
                px(lx + sw, -3, 1, 4, SHADE)
        elif st == "ledge":
            for i, lx in enumerate(legs):
                sw = math.sin(self.leg_phase * 2 + i * 1.7) * 0.4
                px(lx + 1 + sw, -1, 1, 3.5, SHADE)
        elif st in AIR:
            for i, lx in enumerate(legs):
                px(lx + (0.5 if i < 2 else -0.5) * (1 if self.vy < 0 else -1), -3, 1, 2.5, SHADE)
        elif not sitting:
            ph = int(self.leg_phase) % 2
            for i, lx in enumerate(legs):
                if walking and i % 2 == ph:
                    px(lx + 0.5, -3 + bob, 1, 2, SHADE)
                else:
                    px(lx, -3 + bob, 1, 3, SHADE)

        # тело
        px(-6, top, 12, 8, BODY)
        px(-6, top + 7, 12, 1, SHADE)
        px(-5, top, 3, 1, LIGHT)

        # ручки
        if st == "cling":
            # перебирает руками по краю окна
            ph = int(self.leg_phase) % 2 if self.cling and self.cling["mode"] == "climb" else 0
            px(-8, top + ph * 1.5, 2, 2, BODY)
            px(6, top + (1 - ph) * 1.5, 2, 2, BODY)
        else:
            arms_up = st in ("drag", "happy", "jump", "thrown", "hold", "block") or hanging \
                or (st in ("fall", "leap") and self.vy < 0)
            ay = top + (0 if arms_up else 3)
            if walking:
                ay += 0.5 if int(self.leg_phase) % 2 else 0
            px(-8, ay, 2, 2, BODY)
            px(6, ay, 2, 2, BODY)

        # глаза
        ex = self.eye_dx * 0.6
        ey = self.eye_dy * 0.5
        eye_y = top + 2
        mode = self.eye_mode(st)
        pen = QPen(EYE, P * 0.55)
        pen.setCapStyle(Qt.PenCapStyle.SquareCap)
        for cx in (-4, 3):
            if mode == "open":
                px(cx + ex, eye_y + ey, 1, 2, EYE)
            elif mode == "closed":
                px(cx - 0.3, eye_y + 1.2, 1.6, 0.6, EYE)
            else:
                p.setPen(pen)
                x0, y0 = (cx + 0.5) * P, (eye_y + 1) * P
                s = P * 0.8
                if mode == "happy":
                    p.drawPolyline([QPointF(x0 - s, y0 + s * 0.5), QPointF(x0, y0 - s * 0.5),
                                    QPointF(x0 + s, y0 + s * 0.5)])
                elif mode == "dizzy":
                    p.drawLine(QPointF(x0 - s, y0 - s), QPointF(x0 + s, y0 + s))
                    p.drawLine(QPointF(x0 - s, y0 + s), QPointF(x0 + s, y0 - s))
                elif mode == "surprised":
                    d = 1 if cx < 0 else -1
                    p.drawPolyline([QPointF(x0 - d * s, y0 - s), QPointF(x0 + d * s * 0.6, y0),
                                    QPointF(x0 - d * s, y0 + s)])
                p.setPen(Qt.PenStyle.NoPen)

        if mode in ("happy", "surprised") or st in ("sit", "ledge", "peek"):
            px(-5.5, eye_y + 2.6, 1.5, 0.8, BLUSH)
            px(4, eye_y + 2.6, 1.5, 0.8, BLUSH)

        # звёздочки над головой
        if st == "dizzy":
            for i in range(3):
                a = self.t * 4 + i * 2.09
                self.star(p, math.cos(a) * 7 * P, (top - 2) * P + math.sin(a) * 1.5 * P, 4)

    def tagilla_frame(self, st, hanging):
        """Какой кадр Тагиллы показать сейчас."""
        if hanging or st in ("drag", "hold", "cling"):
            return "hang"
        if st == "smash" and self.smash:
            wind = 0.22 if self.smash["kind"] in ("victim", "cursor") else 0.45
            return "windup" if self.state_t < wind else "strike"
        if st in AIR:
            return "jump"
        if st in MOVING:  # цикл ходьбы: walk1, walk2, ... сколько кадров есть
            walk = tagilla_frames()["walk"]
            return walk[int(self.leg_phase) % len(walk)]
        if st in ("sit", "ledge", "dizzy"):
            return "sit"
        if st == "sleep":
            return "sleep"
        if st in ("gloat", "happy"):
            return "windup"  # победно задрал кувалду
        return "idle"

    def draw_tagilla(self, p, st, hanging):
        """Тагилла: целиком кадрами из спрайт-листа (нарисован GPT)."""
        fr = tagilla_frames()
        x, y, w, h, ax = fr["frames"][self.tagilla_frame(st, hanging)]
        p.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform, False)  # чёткие пиксели
        p.drawImage(QPointF(-ax, -h), fr["img"], QRectF(x, y, w, h))
        if st == "dizzy":
            for i in range(3):
                a = self.t * 4 + i * 2.09
                self.star(p, math.cos(a) * 22, -h - 6 + math.sin(a) * 6, 4)
        if st in ("hunt", "smash", "gloat") and self.prey and int(self.t * 6) % 2:
            p.setPen(QColor(255, 60, 40))  # озверел
            p.setFont(ui_font(12))
            p.drawText(QPointF(-6, -h - 4), "!")

    def star(self, p, x, y, r):
        path = QPainterPath()
        for i in range(10):
            a = -math.pi / 2 + i * math.pi / 5
            rr = r if i % 2 == 0 else r * 0.45
            pt = QPointF(x + math.cos(a) * rr * 1.6, y + math.sin(a) * rr * 1.6)
            path.moveTo(pt) if i == 0 else path.lineTo(pt)
        path.closeSubpath()
        p.fillPath(path, QColor("#FFD84A"))

    def draw_particles(self, p):
        for pt in self.particles:
            lx, ly = self.to_local(pt.x, pt.y)
            k = max(0.0, pt.life / pt.max_life)
            if pt.kind == "dust":
                r = (3 + (1 - k) * 6) * pt.size
                p.setBrush(QColor(200, 190, 180, int(180 * k)))
                p.setPen(Qt.PenStyle.NoPen)
                p.drawEllipse(QPointF(lx, ly), r, r)
            elif pt.kind == "heart":
                c = QColor(255, 90, 120, int(255 * min(1, k * 2)))
                s = 2.6 * pt.size
                for (hx, hy) in ((-2, -1), (-1, -1), (1, -1), (2, -1), (-2, 0), (-1, 0), (0, 0),
                                 (1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)):
                    p.fillRect(QRectF(lx + hx * s, ly + hy * s, s, s), c)
            elif pt.kind == "z":
                f = ui_font(int(9 + (1 - k) * 8))
                f.setBold(True)
                p.setFont(f)
                p.setPen(QColor(90, 110, 200, int(255 * min(1, k * 2))))
                p.drawText(QPointF(lx, ly), "z")
            elif pt.kind == "sweat":
                p.setPen(Qt.PenStyle.NoPen)
                p.setBrush(QColor(140, 200, 255, int(220 * k)))
                p.drawEllipse(QPointF(lx, ly), 2.2 * pt.size, 3.2 * pt.size)
            elif pt.kind == "shard":
                c = QColor(225, 240, 255, int(230 * min(1, k * 2)))
                s = 3.2 * pt.size
                p.setBrush(c)
                p.setPen(QPen(QColor(120, 150, 180, int(200 * k)), 0.8))
                p.drawPolygon([QPointF(lx, ly - s), QPointF(lx + s * 0.8, ly + s * 0.6),
                               QPointF(lx - s * 0.6, ly + s * 0.4)])
            elif pt.kind == "spark":
                p.save()
                p.setOpacity(k)
                self.star(p, lx, ly, 3 * pt.size)
                p.restore()

    def draw_bubble(self, p):
        r = self.bubble_rect()
        path = QPainterPath()
        path.addRoundedRect(r, 9, 9)
        tail = QPainterPath()
        tx = AX + self.facing * 6
        tail.moveTo(tx - 7, r.bottom() - 1)
        tail.lineTo(tx + 7, r.bottom() - 1)
        tail.lineTo(tx - 2 * self.facing, r.bottom() + 11)
        tail.closeSubpath()
        path = path.united(tail)
        p.setOpacity(min(1.0, self.bubble_t * 4))
        p.setPen(QPen(QColor("#2a2a2a"), 2))
        p.setBrush(QColor("#fffaf3"))
        p.drawPath(path)
        p.setPen(QColor("#2a2a2a"))
        p.setFont(self.bubble_font())
        p.drawText(r.adjusted(10, 6, -10, -6),
                   Qt.AlignmentFlag.AlignCenter | Qt.TextFlag.TextWordWrap, self.bubble)
        p.setOpacity(1)

    # ---------- мышь ----------
    def mousePressEvent(self, e):
        if e.button() == Qt.MouseButton.LeftButton:
            self.press_pos = e.globalPosition()
            self.drag_off = e.globalPosition() - QPointF(self.x, self.y)
        elif e.button() == Qt.MouseButton.RightButton:
            self.menu(e.globalPosition().toPoint())

    def mouseMoveEvent(self, e):
        if self.press_pos is None or self.state == "drag":
            return
        if (e.globalPosition() - self.press_pos).manhattanLength() > 5:
            self.leave_surface()
            self.cling = self.peek = self.leap = None
            if self.cursor_fling in flings:
                self.cursor_fling.finish()
            self.cursor_fling = None
            self.peek_cd = 15
            self.set_state("drag")
            self.mouse_hist = []
            self.squash = -0.15
            if random.random() < 0.6:
                self.say(self.line("pick"), 1.3)

    def mouseReleaseEvent(self, e):
        if e.button() != Qt.MouseButton.LeftButton or self.press_pos is None:
            return
        self.press_pos = None
        if self.state != "drag":
            self.pet()
            return
        vx = vy = 0.0
        if len(self.mouse_hist) > 1:
            (t0, p0), (t1, p1) = self.mouse_hist[0], self.mouse_hist[-1]
            if t1 > t0:
                vx = (p1.x() - p0.x()) / (t1 - t0)
                vy = (p1.y() - p0.y()) / (t1 - t0)
        if math.hypot(vx, vy) < 400 and self.try_cling_near():
            self.say(self.line("grab"), 1.4)
            return
        self.vx = max(-2500, min(2500, vx))
        self.vy = max(-2500, min(2500, vy))
        self.dizzy_next = math.hypot(self.vx, self.vy) > 1800
        self.set_state("thrown" if math.hypot(vx, vy) > 400 else "fall")
        if self.state == "thrown" and random.random() < 0.6:
            self.say("уиииии!", 1.2)

    def mouseDoubleClickEvent(self, e):
        if e.button() == Qt.MouseButton.LeftButton and self.state in GROUNDED:
            self.jump(900, self.facing * 120)
            self.burst("spark", 5, 0.8)
            self.say("хоп!", 1)

    def pet(self):
        if self.state == "sleep":
            self.say("а? что? я не сплю!", 2)
            self.set_state("idle", 1.5)
            return
        if self.state in AIR:
            return
        self.burst("heart", 4, 1.4)
        if random.random() < 0.6:
            self.say(self.line("pet"), 1.5)
        if self.state in GROUNDED:
            self.set_state("happy", 1.3)

    def menu(self, pos):
        m = QMenu()
        acts = [
            ("Гулять", lambda: self.set_state("walk", rnd(4, 8))),
            ("Сидеть", lambda: self.set_state("sit", rnd(8, 15))),
            ("Спать", lambda: self.set_state("sleep", rnd(20, 40))),
            ("Прыгнуть", lambda: self.jump(900, self.facing * 120)),
            ("Залезть на окно", self.plan_explore),
            ("Сказать что-нибудь", lambda: self.say(self.line("idle"), 3)),
        ]
        for name, fn in acts:
            m.addAction(name).triggered.connect(fn)
        ch = QAction("Бегать за курсором", m, checkable=True, checked=self.chase)
        ch.triggered.connect(self.toggle_chase)
        m.addAction(ch)
        m.addSeparator()
        if self.skin.smashes:
            m.addAction("Крушить!").triggered.connect(self.plan_smash)
            m.addAction("Взбеситься сейчас").triggered.connect(self.start_rage)
            rage = QAction("Буйство без перерыва", m, checkable=True, checked=self.rage_forever)
            rage.triggered.connect(self.toggle_rage_forever)
            m.addAction(rage)
        who = m.addMenu("Персонаж")
        for name, skin in SKINS.items():
            act = QAction(skin.title, who, checkable=True, checked=skin is self.skin)
            act.triggered.connect(lambda _=False, n=name: self.set_skin(n))
            who.addAction(act)
        friend = m.addMenu("Позвать друга")
        for name, skin in SKINS.items():
            friend.addAction(skin.title).triggered.connect(
                lambda _=False, n=name: pets.append(Pet(skin=n)))
        if len(pets) > 1:
            m.addAction("Убрать этого").triggered.connect(self.remove)
        m.addAction("Выход").triggered.connect(QApplication.quit)
        m.exec(pos)

    def set_skin(self, name):
        self.skin = SKINS[name]
        QSettings("claude-mascot", "mascot").setValue("skin", name)  # новые маскоты — такие же
        self.prey = None
        self.hunt_cd = 3.0
        self.last_mask = None
        self.hammer_a = -15.0
        self.smash = None
        if self.state == "smash":
            self.set_state("idle", 1)
        self.squash = 0.4
        self.burst("spark", 6, 0.8)

    def toggle_chase(self, on):
        self.chase = on
        if on and self.skin.smashes:
            platform_backend.prepare_cursor()  # Тагилла будет бить по курсору
        if on:
            self.say("сейчас догоню!", 1.5)
            if self.state in GROUNDED:
                self.set_state("chase")
        else:
            self.set_state("idle", 2)

    def remove(self):
        self.say("пока!", 0.6)
        if self in pets:
            pets.remove(self)
        QTimer.singleShot(500, self.close)


def main():
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    app = QApplication(sys.argv)
    app.setApplicationName("claude-mascot")
    app.setQuitOnLastWindowClosed(False)

    if not platform_backend.start(app, World):
        print("Маскот уже запущен.")
        return
    pets.append(Pet())
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
