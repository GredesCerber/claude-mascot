# Claude Mascot Rust

Windows-first native Rust port. Version 0.5.0 keeps **Claude only** and adds
75 activities from the supplied pixel pack, five friendly pair interactions,
nearby-cursor gaze, and a rear-facing laptop lid. The original Python/Linux
application at the repository root is maintained separately.

Claude was already drawn in code. Its body, face, limbs and laptop are small
pixel rectangles in a premultiplied BGRA buffer presented by Win32 layered
windows. No GIF, external sprite atlas or graphics runtime is needed.
Rendering rate is independent of the 240 Hz physics loop.

## Install and remove

Windows 10/11 x64. No Python or Rust runtime is required.
Use the per-user MSI for shortcuts, upgrades and removal through Windows
Settings or the Start Menu uninstall shortcut. See [MSI packaging](docs/msi.md).
MSI installs update through a newer MSI.

Alternatively, extract the portable ZIP and run `claude-mascot.exe`.
`Install.ps1` copies the portable application to
`%LOCALAPPDATA%\claude-mascot-rust` and creates shortcuts.
Autostart is off by default; use the menu or `Install.ps1 -Autostart` to enable it.
`Uninstall.ps1` removes the portable installation and its own startup entry.
Settings are preserved unless `-RemoveSettings` is supplied.
The built-in updater applies to portable installs; see [updates](docs/updates.md).

## Controls

Click to pet, drag with the left button to carry, release quickly to throw,
double-click to jump. Right-click the pet or its tray icon for the native menu.

| Menu | Actions |
| --- | --- |
| Заняться | Jump, pet, dance, wave, laptop, stretch, look around |
| Заняться → Все занятия (75) | All pack scenes grouped by mood, code, workshop, nature, food, celebration, adventure, reflection and sport |
| Поведение | Wander, sit, sleep, climb, chase the cursor |
| Друзья | Invite another Claude (up to eight), remove the selected pet |
| Друзья → Вместе с другом | Greeting, high-five, dance together, share a laptop, comfort a friend |
| Настройки | Scale, speech, mood meter, fullscreen hiding, startup, frame rate |

Laptop opens for 1 second, types for 8 seconds and closes for 1 second.
Stretch lasts 2.5 seconds; look around lasts 2 seconds. These activities can
occasionally start while idle, or be selected from the menu.
An airborne command waits for landing. Dragging, jumping, a replacement command
or losing support interrupts the sequence and removes its prop.

Chase includes running, hopping and hanging from the cursor. Shake the cursor,
disable chase or choose Jump to release. The pet never moves the actual cursor.
It rebounds from screen walls and occasionally hops onto reachable window tops.
Explicit Sit or Sleep suppresses autonomous exploration; choose Wander to resume.
Rough throws reduce session-only satisfaction; petting helps it recover faster.

Friendly scenes require two available pets close together on the same floor or
window top. Each pet belongs to at most one pair. Drag, a new command, jumping,
loss of support or removal of either pet interrupts the scene. Pair selection
uses a 10 Hz scheduler and fixed-size storage; active poses advance with physics.
No full pet objects are cloned each step. Disable friend interactions under
Friends, or disable random activities under Settings. Manual activities remain
available when random activities are disabled. Nearby gaze relaxes to neutral
when the cursor moves away, and does not override closed/blinking eyes.

The pack contained 75 static illustrations, not ready-made animation frames.
Each is a timed cosmetic activity with an entry/exit and animated accents;
selected props have additional motion. The SVGs were converted offline into
compiled pixel props. The imported bodies and faces are discarded: every scene
uses the same canonical Claude renderer, dimensions, palette and eyes. Gestures
and expressions are drawn in that shared style. See [the complete pack integration](docs/activity-pack.md).
The [visual style contract](docs/visual-style.md) defines the shared character.

Frame presets are 30, 60, 120, 144 and 240 FPS (default 60).
`claude-mascot.exe --fps 90` saves a custom limit from 30 to 240.
Actual frame rate depends on desktop load and hardware.

## Settings and migration

Preferences: `%LOCALAPPDATA%\claude-mascot-rust\settings.json`,
separate from Python. Legacy settings selecting Tagilla load as Claude.
The old custom-atlas field is ignored and omitted on the next save.
Other preferences, including FPS and scale, are retained.
The Rust port no longer includes character switching, combat or atlas loading.

## Build and verification

MSVC needs Visual Studio C++ Build Tools and the Windows SDK.
The verified Windows GNU alternative is:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
cargo +stable-x86_64-pc-windows-gnu fmt --check
cargo +stable-x86_64-pc-windows-gnu test --all-targets --locked
cargo +stable-x86_64-pc-windows-gnu clippy --all-targets --locked -- -D warnings
cargo +stable-x86_64-pc-windows-gnu build --release --locked
.\tools\Set-ExecutableIcon.ps1 -InputExe target/release/claude-mascot.exe -IconPath assets/claude-icon.ico
.\Build-Msi.ps1
```

`Build.ps1 -Package` uses the default Cargo toolchain to check, build,
embed the icon and package a portable ZIP. Executable QA arguments:

```powershell
.\target\release\claude-mascot.exe --preview .\previews
.\target\release\claude-mascot.exe --headless
.\target\release\claude-mascot.exe --pets 4
.\target\release\claude-mascot.exe --pets 8 --activity lifting-heavy-barbell
.\target\release\claude-mascot.exe --smoke-test 5 --trace .\smoke.json
```

See [architecture](docs/architecture.md), [0.5.0 checks](docs/verification-0.5.0.md)
and the [remaining roadmap](docs/planning/PLAN-RU.md).
The proposed phrase bank is planning data, not a runtime dialogue engine.

This port does not provide complete Python parity. Linux/KDE/Wayland integration
is provided by Python; window route planning and media peek remain simplified.
Mixed-DPI coverage, a real MSI install/upgrade/remove cycle and a published
portable updater cycle remain separate acceptance checks.
