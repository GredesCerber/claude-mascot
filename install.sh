#!/usr/bin/env bash
# Ставит маскота в ~/.local/share/claude-mascot и запускает его.
#   ./install.sh              — установить / обновить и запустить
#   ./install.sh --autostart  — то же + запуск при входе в систему
set -euo pipefail

SRC="$(cd "$(dirname "$0")" && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/claude-mascot"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
AUTOSTART_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
AUTOSTART=0
for arg in "$@"; do
    case "$arg" in
        --autostart) AUTOSTART=1 ;;
        *) echo "Неизвестный параметр: $arg"; exit 1 ;;
    esac
done

if ! command -v python3 >/dev/null; then
    echo "Нужен python3 (3.9+). Поставь его через пакетный менеджер и запусти ещё раз."
    exit 1
fi

if ! ldconfig -p 2>/dev/null | grep libxcb-cursor >/dev/null; then
    echo "⚠ Не нашёл libxcb-cursor — без неё Qt может не запуститься. Поставь пакет:"
    echo "    Fedora/Nobara: sudo dnf install xcb-util-cursor"
    echo "    Ubuntu/Debian: sudo apt install libxcb-cursor0"
    echo "    Arch:          sudo pacman -S xcb-util-cursor"
fi

# процесс маскота: python …mascot.py, запущенный из папки claude-mascot (путь может быть относительным)
is_mascot() {
    local cmd cwd
    cmd="$(tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null)" || return 1
    cwd="$(readlink "/proc/$1/cwd" 2>/dev/null)"
    [[ "$cmd" == *mascot.py* ]] && [[ "$cmd" == *claude-mascot/* || "$cwd" == */claude-mascot ]]
}

stop_running() {
    for pid in $(pgrep -x python 2>/dev/null; pgrep -x python3 2>/dev/null); do
        if is_mascot "$pid"; then
            kill "$pid" 2>/dev/null || true
        fi
    done
}

echo "→ Копирую файлы в $DEST"
mkdir -p "$DEST"
cp "$SRC/mascot.py" "$SRC/platform_linux.py" "$SRC/kwin_windows.js" "$DEST/"

if [ ! -x "$DEST/venv/bin/python" ]; then
    echo "→ Создаю виртуальное окружение"
    python3 -m venv "$DEST/venv"
fi
echo "→ Ставлю PyQt6 (первый раз ~1 минута)"
"$DEST/venv/bin/pip" install -q --disable-pip-version-check --upgrade PyQt6

ENTRY="[Desktop Entry]
Type=Application
Name=Claude-маскот
Comment=Пиксельный маскот на рабочем столе
Exec=$DEST/venv/bin/python $DEST/mascot.py
Icon=face-smile-big
Terminal=false
Categories=Amusement;"

mkdir -p "$APPS"
printf '%s\n' "$ENTRY" > "$APPS/claude-mascot.desktop"

DESKTOP_DIR="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")"
if [ -d "$DESKTOP_DIR" ] && [ "$DESKTOP_DIR" != "$HOME" ]; then
    printf '%s\n' "$ENTRY" > "$DESKTOP_DIR/Claude-маскот.desktop"
    chmod +x "$DESKTOP_DIR/Claude-маскот.desktop"
    echo "→ Ярлык на рабочем столе: $DESKTOP_DIR/Claude-маскот.desktop"
fi

if [ "$AUTOSTART" = 1 ]; then
    mkdir -p "$AUTOSTART_DIR"
    printf '%s\n' "$ENTRY" > "$AUTOSTART_DIR/claude-mascot.desktop"
    echo "→ Добавил в автозапуск"
fi

echo "→ Запускаю"
stop_running
sleep 0.5
setsid nohup "$DEST/venv/bin/python" "$DEST/mascot.py" >/dev/null 2>&1 &
echo "Готово! Маскот должен упасть сверху на экран."
