#!/usr/bin/env python3
"""Делает спрайт маскота из картинки на однотонном фоне (например, сгенерированной GPT).

    python tools/make_sprite.py исходник.png результат.png [высота=110] [фон=#FF00FF]

Вырезает фон по цвету, обрезает по фигуре, уменьшает до нужной высоты и делает
жёсткие пиксельные края (полупрозрачное — либо целиком видно, либо нет).
"""
import sys

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QColor, QGuiApplication, QImage


def is_background(c, key):
    # близко к цвету фона по оттенку — с запасом на сглаживание на краях
    return abs(c.red() - key.red()) + abs(c.green() - key.green()) + abs(c.blue() - key.blue()) < 150


def main():
    src, dst = sys.argv[1], sys.argv[2]
    height = int(sys.argv[3]) if len(sys.argv) > 3 else 110
    key = QColor(sys.argv[4] if len(sys.argv) > 4 else "#FF00FF")
    QGuiApplication([])
    img = QImage(src).convertToFormat(QImage.Format.Format_ARGB32)
    w, h = img.width(), img.height()
    x0, y0, x1, y1 = w, h, -1, -1
    for y in range(h):
        for x in range(w):
            c = img.pixelColor(x, y)
            if is_background(c, key):
                img.setPixelColor(x, y, QColor(0, 0, 0, 0))
            else:
                # на краях фигуры убираем розовый ореол
                if c.red() > c.green() + 60 and c.blue() > c.green() + 60:
                    c = QColor(c.green(), c.green(), c.green(), c.alpha())
                    img.setPixelColor(x, y, c)
                x0, y0, x1, y1 = min(x0, x), min(y0, y), max(x1, x), max(y1, y)
    img = img.copy(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    small = img.convertToFormat(QImage.Format.Format_ARGB32_Premultiplied).scaledToHeight(
        height, Qt.TransformationMode.SmoothTransformation).convertToFormat(QImage.Format.Format_ARGB32)
    for y in range(small.height()):
        for x in range(small.width()):
            c = small.pixelColor(x, y)
            small.setPixelColor(x, y, QColor(c.red(), c.green(), c.blue(), 255 if c.alpha() > 120 else 0))
    small.save(dst)
    print(f"{dst}: {small.width()}x{small.height()}")


if __name__ == "__main__":
    main()
