# Claude Mascot Rust

Windows-first native Rust port maintained at [GredesCerber/claude-mascot](https://github.com/GredesCerber/claude-mascot).
Version 0.12.6 keeps **Claude only** and includes
a separate native settings window, recoloring, personal space, collision
impulses, ordinary jump-over avoidance, leapfrog and group towers of up to eight.
It includes varied window/landing selection, deliberate approaches to window edges,
a 0.22-second pixel crouch before exploration jumps, voluntary ledge descent,
desktop-icon perches, adaptive jumps, cursor/Explorer reactions,
75 activities from the supplied pixel pack and nine friendly social activities.
Hard hits now produce bounded tumbling, bounces and a short dizzy recovery.
Floating windows can be reached by a hop to their side followed by climbing;
safe routes are chosen randomly rather than by shortest distance. Covered desktop
icons release their occupants under gravity, including after Alt+Tab.
Version 0.7.1 establishes this fork's own release/update channel and migrates
the old upstream update setting while keeping other preferences.
The original [Python/Linux application](https://github.com/kerimovernes/claude-mascot)
is preserved at the repository root. See [attribution](../ATTRIBUTION.md).

Claude was already drawn in code. Its body, face, limbs and laptop are small
pixel rectangles in a premultiplied BGRA buffer presented by Win32 layered
windows. No GIF, external sprite atlas or graphics runtime is needed.
Rendering rate is independent of the 240 Hz physics loop.

## Install and remove

Windows 10/11 x64. No Python or Rust runtime is required.
Use the per-user MSI for shortcuts, upgrades and removal through Windows
Settings or the Start Menu uninstall shortcut. See [MSI packaging](docs/msi.md).
MSI installs download verified installer updates, apply them through Windows
Installer and restart the pet. Automatic checking and installation can be disabled
in Settings. Checking is enabled by default; silent installation requires the
new checkbox. Without it, one confirmation installs the
update directly, without opening a release webpage. See [updates](docs/updates.md).

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
Open **Settings → Open settings** for the modeless control window, or launch
with `--settings`. The window applies preferences immediately and contains
color, frame rate, scale, activity intensity, personal space, display/behavior
switches, the complete activity catalog and quick commands. Closing it leaves
the pets running. Intensity changes optional behavior frequency, not FPS.

| Menu | Actions |
| --- | --- |
| Заняться | Jump, pet, dance, wave, laptop, stretch, look around |
| Заняться → Все занятия (75) | All pack scenes grouped by mood, code, workshop, nature, food, celebration, adventure, reflection and sport |
| Поведение | Wander, sit, sleep, climb, chase the cursor |
| Друзья | Invite another Claude (up to eight), remove the selected pet |
| Друзья → Вместе с другом | Greeting, high-five, dance together, share a laptop, comfort a friend, tag, group tower, leapfrog, friendly rough play |
| Настройки | Scale, speech, mood meter, fullscreen hiding, startup, frame rate |

Laptop opens for 1 second, types for 8 seconds and closes for 1 second.
Stretch lasts 2.5 seconds; look around lasts 2 seconds. These activities can
occasionally start while idle, or be selected from the menu.
An airborne command waits for landing. Dragging, jumping, a replacement command
or losing support interrupts the sequence and removes its prop.

Chase includes running, hopping and hanging from the cursor. Shake the cursor,
disable chase or choose Jump to release. The pet never moves the actual cursor.
It rebounds from screen walls and occasionally hops onto reachable window tops.
With user reactions enabled, nearby cursor movement can invite a short chase;
this ends automatically and never attaches the pet to the cursor. Explorer focus
or folder navigation can draw a brief glance and greeting. Desktop icon perches
use cached public UI Automation geometry; labels and paths are not collected.
Both features have separate menu switches. High jumps respect monitor headroom.
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
Tag and leapfrog require a sufficiently wide shared runway. A tower gathers all
available pets on one support (up to eight), forms through real jumps, then
dismounts from the top down. Low ceilings or unsafe routes prevent the entire
group from starting. Busy, sleeping or manually controlled pets remain excluded.
See [social play](docs/social-play.md) and [desktop perches](docs/desktop-perches.md).

The default edge-to-edge settling range is 24–96 pixels at scale 1. Greeting,
dancing and comfort work from a distance; they do not pull pets into contact.
Safe ordinary encounters can trigger a jump over a friend. Strong throws use
body contacts and pass part of their velocity to the other pet. See
[behavior and contact](docs/behavior-model.md). Custom spacing is saved with
`--spacing 32:112`; the context menu also offers three presets.

Each live pet can be recolored independently. Presets and a system color picker
derive matching limb/shadow/highlight colors while preserving Claude's geometry.
The last selected color is saved as the default for new pets and future sessions.

Speech varies by event and activity: 300 scene lines (four per catalog entry)
plus 221 options for 44 reactions and social roles. Recent choices are remembered
to avoid immediate repeats; repeated clicks and ambient chatter have cooldowns.
Speech randomness is separate from physics and is reseeded for each native session.
Nearby friends take speaking turns so their bubbles do not overlap.

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

See [architecture](docs/architecture.md), [0.10.0 checks](docs/verification-0.10.0.md)
and the [remaining roadmap](docs/planning/PLAN-RU.md).
Runtime speech banks and their cooldowns are implemented in `dialogue.rs`.

This port does not provide complete Python parity. Linux/KDE/Wayland integration
is provided by Python; window route planning and media peek remain simplified.
Mixed-DPI hardware coverage, a complete clean-install/uninstall MSI cycle and a
published portable updater cycle remain separate acceptance checks. An installed
MSI upgrade from 0.7.1 to 0.8.0 was verified on the development machine.
