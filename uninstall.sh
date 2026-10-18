#!/usr/bin/env bash
# Останавливает маскота и удаляет всё, что поставил install.sh.
set -uo pipefail

DEST="${XDG_DATA_HOME:-$HOME/.local/share}/claude-mascot"

is_mascot() {
    local cmd cwd
    cmd="$(tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null)" || return 1
    cwd="$(readlink "/proc/$1/cwd" 2>/dev/null)"
    [[ "$cmd" == *mascot.py* ]] && [[ "$cmd" == *claude-mascot/* || "$cwd" == */claude-mascot ]]
}

for pid in $(pgrep -x python 2>/dev/null; pgrep -x python3 2>/dev/null); do
    if is_mascot "$pid"; then
        kill "$pid" 2>/dev/null
    fi
done
# KWin-скрипт маскот выгружает сам при выходе; на всякий случай — ещё раз
busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting unloadScript s claude-mascot-windows >/dev/null 2>&1

DESKTOP_DIR="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")"
rm -rf "$DEST"
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/applications/claude-mascot.desktop" \
      "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/claude-mascot.desktop" \
      "$DESKTOP_DIR/Claude-маскот.desktop"
echo "Маскот удалён."
