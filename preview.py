#!/usr/bin/env python3
r"""Рисует все позы маскота в одну картинку, не запуская его на рабочем столе.

    Windows: %LOCALAPPDATA%\claude-mascot\venv\Scripts\python.exe preview.py [out.png] [скин]
    Linux:   ~/.local/share/claude-mascot/venv/bin/python preview.py [out.png] [скин]

Скин — ключ из SKINS в mascot.py (claude, tagilla, ...); по умолчанию claude.

Нужен после любых правок внешнего вида: открой картинку и проверь каждую позу.
"""
import os
import sys

os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import mascot as m  # noqa: E402
from PyQt6.QtCore import QPoint  # noqa: E402
from PyQt6.QtGui import QColor, QFont, QImage, QPainter  # noqa: E402
from PyQt6.QtWidgets import QApplication  # noqa: E402

out_path = sys.argv[1] if len(sys.argv) > 1 else "preview.png"
skin = sys.argv[2] if len(sys.argv) > 2 else "claude"
app = QApplication([])
m.World.set_state({"areas": [], "windows": [
    dict(id="win", x=300, y=300, w=300, h=400, cls="demo", df="", cap="", fs=False, act=False)]})
cursor = [QPoint(450, 260)]
m.QCursor.pos = staticmethod(lambda: cursor[0])

pet = m.Pet(skin=skin)
pet.clock = type("FixedClock", (), {"restart": lambda self: 16})()  # ровно 16 мс на кадр
pet.next_talk = 1e9


def pose_idle():
    pet.set_state("idle", 99)


def pose_walk():
    pet.set_state("walk", 99)


def pose_sit():
    pet.set_state("sit", 99)


def pose_sleep():
    pet.set_state("sleep", 99)


def pose_happy():
    pet.set_state("happy", 99)
    pet.burst("heart", 4)
    pet.say("хе-хе")


def pose_drag():
    pet.set_state("drag")
    pet.say("уиии!")


def pose_jump():
    pet.vx, pet.vy = 100, -600
    pet.set_state("jump")


def pose_dizzy():
    pet.set_state("dizzy", 99)


def pose_ledge():
    pet.x, pet.y = 594, 300
    pet.surface, pet.surf_pos = "win", (300, 300)
    pet.set_state("ledge", 99)


def pose_cling():
    pet.x, pet.y = 300 - m.CLING_OFF, 500
    pet.start_cling("win", 1, "climb")
    pet.state_len = 99


def pose_peek():
    m.World.playing = ["demo"]  # «играет видео» в демо-окне
    pet.enter_peek()


def pose_hold():
    pet.chase = True
    pet.grab_cd = 0
    pet.x, pet.y = cursor[0].x(), cursor[0].y() + pet.skin.hold_dy
    pet.try_catch_cursor(cursor[0])


def pose_smash():
    pet.start_smash("down")


POSES = [("idle", pose_idle), ("walk", pose_walk), ("sit", pose_sit), ("sleep", pose_sleep),
         ("happy", pose_happy), ("drag", pose_drag), ("jump", pose_jump), ("dizzy", pose_dizzy),
         ("ledge", pose_ledge), ("cling", pose_cling), ("peek", pose_peek), ("hold", pose_hold)]
if pet.skin.smashes:
    POSES += [("smash: замах", pose_smash), ("smash: удар", pose_smash)]
TICKS = {"jump": 12, "smash: замах": 26, "smash: удар": 40}

COLS = 4
rows = (len(POSES) + COLS - 1) // COLS
sheet = QImage(m.W * COLS, (m.H + 20) * rows, QImage.Format.Format_ARGB32)
sheet.fill(QColor("#7a8a9a"))
painter = QPainter(sheet)
painter.setFont(QFont("Sans", 11))
for i, (name, setup) in enumerate(POSES):
    pet.particles, pet.bubble, pet.chase = [], None, False
    m.World.playing = []
    pet.x, pet.y, pet.vx, pet.vy = 150.0, 800.0, 0.0, 0.0
    pet.surface = pet.cling = pet.peek = None
    pet.tilt = pet.squash = pet.squash_v = 0.0
    pet.hammer_a = -45.0
    setup()
    for _ in range(TICKS.get(name, 30)):
        pet.tick()
    x, y = (i % COLS) * m.W, (i // COLS) * (m.H + 20)
    painter.drawImage(x, y + 20, pet.grab().toImage())
    painter.setPen(QColor("white"))
    painter.drawText(x + 8, y + 16, f"{name}  (state={pet.state})")
painter.end()
sheet.save(out_path)
print("сохранено:", os.path.abspath(out_path))
