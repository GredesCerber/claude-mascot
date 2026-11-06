# Claude Mascot Rust

The implementation and verification roadmap is in
[docs/planning/PLAN-RU.md](docs/planning/PLAN-RU.md). Proposed animations and
phrases are marked as proposals; they are not runtime features of 0.3.1.

Windows-first native Rust port of Claude Mascot. It runs as a small layered
desktop window, with a right-click menu and an optional notification-area icon.
The existing Python version remains available at the repository root; the Rust
port does not replace its Linux/KDE support.

Version 0.3 adds screen-edge rebounds, animated climbing, cursor-chasing hops,
occasional autonomous jumps onto reachable window tops, and short dance/wave
activities. The behavior reference is the original `mascot.py`, including
`tick_chase`, `try_catch_cursor`, and its window-jump/climbing behavior.

Each pet has a session-only satisfaction meter. Rough throws make it unhappy;
gentle repositioning leaves its mood unchanged. Time gradually calms an unhappy
pet, and petting improves its mood faster. Mood changes Claude's eyebrows and
the meter's color, without enabling Tagilla's rage mode. Hide the meter under
`Настройки → Показывать настроение` if preferred.

## Windows use

A per-user MSI installer is also available. It adds shortcuts and supports
Windows Installer upgrades and removal. See [MSI packaging](docs/msi.md).
MSI installations receive updates through a newer MSI, while portable installs
can use the built-in updater.

The portable x64 release is a zip containing `claude-mascot.exe`, this README,
and the install/uninstall scripts. Extract it, then run the executable directly
or use `Install.ps1` to copy it to `%LOCALAPPDATA%\claude-mascot-rust` and add
shortcuts to the Desktop and Start Menu. Windows 10 and 11 x64 are the intended
targets. The release is self-contained; no Python or Rust runtime is needed.

To install from PowerShell in the extracted folder:

```powershell
powershell -ExecutionPolicy Bypass -File .\Install.ps1
```

Add `-Autostart` to explicitly opt in to launching at sign-in:

```powershell
powershell -ExecutionPolicy Bypass -File .\Install.ps1 -Autostart
```

Autostart is off by default. It can also be toggled from the mascot or tray
icon's right-click menu. `Uninstall.ps1` removes the Rust executable, its
shortcuts, and its own autostart entry. Settings are kept unless
`-RemoveSettings` is supplied, which removes only `settings.json` and
`last-error.log`. Other user files are preserved. The Rust installation and
settings are separate from the Python version.

## Controls

Right-click the mascot or its notification-area icon for the native menu.
Quick actions appear at the top; the remaining commands are grouped under
`Поведение`, `Персонажи и друзья`, and `Настройки`.

| Menu item | Action |
| --- | --- |
| Прыгнуть / Погладить / Потанцевать / Помахать | Immediate interactions and timed poses |
| Гулять / Сидеть / Спать / Карабкаться | Choose a basic behavior |
| Гоняться | Toggle cursor chase |
| Крушить / Ярость | Tagilla behavior commands |
| Claude / Тагилла | Change the main pet's character |
| Позвать Claude / Позвать Тагиллу | Add another pet (up to eight) |
| Убрать этого питомца | Remove the pet whose menu was opened |
| Закрыть всех питомцев | Exit the application |
| Пауза | Pause or resume simulation |
| Скрыть в полноэкранном режиме | Toggle hiding over fullscreen windows |
| Запускать вместе с Windows | Toggle the current user's startup entry |
| Маленький / Обычный / Большой размер | Set 75%, 100%, or 150% display scale |
| Настройки → Частота кадров | Choose 30, 60, 120, 144 or 240 FPS (default 60) |
| Настройки → Показывать реплики / Показывать настроение | Toggle speech and the satisfaction meter |

For another frame-rate limit, run `claude-mascot.exe --fps 90`. Values from
30 through 240 are saved in settings. Physics runs independently of rendering;
actual frame rate depends on the machine and desktop load.

Left-click and release to pet the character. Drag to reposition it; release
with a quick movement to throw it. Double-click to jump. These interactions use
the Windows desktop and do not move, click, or close other application windows.
Enable `Поведение → Бегать за курсором` for running, hopping and cursor grabs.
Shake the cursor, turn chase off, or choose Jump to release a hanging pet.
Explicit Sit and Sleep suppress autonomous window exploration; choose Wander
to resume it. Automatic window hops skip unreachable, covered or offscreen tops.

## Settings and custom atlas

The settings file is `%LOCALAPPDATA%\claude-mascot-rust\settings.json`. It is
created when a preference is changed. The file contains a version number,
selected skin, scale (`0.75`, `1.0`, or `1.5`), fullscreen hiding, bubble
visibility, pause state, and an optional custom atlas directory. The supported
settings version is currently `1`.

To replace Tagilla's atlas, set `custom_atlas` to a directory containing
`tagilla_frames.png` and `tagilla_frames.json`. The custom atlas is used for
Tagilla; Claude keeps the built-in Claude renderer. The JSON shape is:

```json
{
  "frames": {
    "idle": [0, 0, 64, 96, 32],
    "walk1": [64, 0, 64, 96, 32],
    "walk2": [128, 0, 64, 96, 32],
    "walk3": [192, 0, 64, 96, 32],
    "walk4": [256, 0, 64, 96, 32],
    "walk5": [320, 0, 64, 96, 32],
    "walk6": [384, 0, 64, 96, 32],
    "walk7": [448, 0, 64, 96, 32],
    "walk8": [512, 0, 64, 96, 32],
    "sit": [576, 0, 64, 96, 32],
    "sleep": [640, 0, 64, 96, 32],
    "windup": [704, 0, 64, 96, 32],
    "strike": [768, 0, 64, 96, 32],
    "hang": [832, 0, 64, 96, 32],
    "jump": [896, 0, 64, 96, 32],
    "dangle": [960, 0, 64, 96, 32]
  }
}
```

Each frame is `[x, y, width, height, anchor_x]` in pixels in the PNG. Frames
must fit inside the image and include `idle`; other frames fall back to `idle`
when missing. Atlas images are limited to 4096 by 4096 pixels and 32 MiB on
disk. The renderer treats the anchor as the character's horizontal foot point.
The app loads image and JSON data only; it does not run scripts from a skin
directory. After changing settings or atlas data, restart the app.

## Build and developer commands

Install Rust with the `x86_64-pc-windows-msvc` target and the Visual Studio C++
Build Tools plus Windows SDK. From this directory, the standard MSVC workflow is:

```powershell
.\Build.ps1
# Create the portable zip as well:
.\Build.ps1 -Package
```

Equivalent direct commands:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

An alternative verified Windows GNU toolchain is available directly from
rustup. Install the GNU toolchain distribution, then prefix Cargo commands
with its selector:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
cargo +stable-x86_64-pc-windows-gnu fmt --check
cargo +stable-x86_64-pc-windows-gnu test --locked
cargo +stable-x86_64-pc-windows-gnu clippy --locked --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu build --locked --release
```

The rustup GNU distribution includes the linker/runtime tools used by this
build; no separate MinGW or Chocolatey installation is needed. The PowerShell
build script uses the default Cargo toolchain.

Useful local runs (append `--` before program arguments when using `cargo run`):

```powershell
cargo run -- --preview .\previews
cargo run -- --headless
cargo run -- --smoke-test 5 --trace .\smoke-trace.json
# Windows-only: expose the mascot window for native inspection
cargo run -- --inspect-window
```

`--preview` writes PNGs for both characters and representative states.
`--headless` runs the platform-independent simulation without creating a
window. `--smoke-test` exercises the native Windows host for a bounded duration
and `--trace` writes summary counters. `--inspect-window` is a Windows-only QA
option that exposes the native window for inspection. See [docs/architecture.md](docs/architecture.md)
for module boundaries and known behavior gaps, and
[docs/verification-0.3.1.md](docs/verification-0.3.1.md) for the current verification
record.

## Current limits

This is a Windows-first port, not feature-complete parity with the Python app.
Claude's drawing is an approximation; climbing routes, video peek placement,
cursor attacks, and multi-pet tactics are simplified. Tagilla has timed,
input-transparent crack and shard effects; real cursor fling is not implemented.
The Rust port does not provide the Python version's
Linux/KDE/Wayland behavior. Review the parity table in
[docs/architecture.md](docs/architecture.md) before relying on a specific
behavior.
