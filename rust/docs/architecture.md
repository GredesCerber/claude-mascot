# Rust port architecture (0.4.0)

The Python application at the repository root is preserved. Rust is Windows-first
and Claude-only. Combat, atlas decoding and crack overlays were removed in 0.4.0.

| Module | Responsibility |
| --- | --- |
| lib.rs | Physical-pixel coordinates, rectangles, world/window snapshots |
| engine.rs | Deterministic physics, support tracking, mood, commands and timed activities |
| render.rs | Procedural Claude/laptop pixels, gestures, cached fonts, transparent BGRA and PNG previews |
| platform.rs | Win32 windows, DPI, alpha hit testing, bounded message pump, capture, menus, tray |
| main.rs | Fixed-step simulation, frame deadlines, input routing, visibility and lifecycle |
| timing.rs | FPS pacing and rolling drag velocity |
| settings.rs | Versioned JSON and legacy character migration |
| media.rs | Background best-effort playback monitoring |
| update*.rs | Portable staging, digest checks, rollback and MSI ownership safeguards |

The engine consumes a World snapshot and never draws, creates windows, moves
the real cursor or sends input to other applications. Pet.pos is between its
feet. Surfaces follow native IDs across enumeration reordering; losing support
causes a fall. Window snapshots reuse vector capacity. Removing combat also
removes the host's per-physics-step cloning of all pets for pairwise interactions.

Physics advances at 240 Hz independently of the selected 30..240 render FPS.
Each message pump processes at most 256 messages; consecutive captured moves
coalesce without crossing button/command boundaries. Visibility calls occur only
on changes. Render deadlines are checked after input. These safeguards preserve
the 0.3.1 drag performance fix.

Laptop is one timed state: open 0..1 s, type 1..9 s, close 9..10 s.
Stretch and LookAround last 2.5 and 2 seconds. The renderer derives poses and
props from state/time; props have no separate window or image decoder.
Airborne pose commands wait for landing. Drag, jump, replacement commands and
missing support interrupt activities. Seeded idle choices require support and
no active chase or drag. Bubbles are cosmetic and do not observe actual typing.

Legacy Tagilla settings resolve to Claude. The retired custom_atlas key is
ignored on read and omitted on write. Unknown character names still fail.

## Verification boundaries

Engine tests cover support loss, taskbar changes, monitor seams, offscreen
release, bounded motion, climbing, chase, mood and timed activities.
Renderer tests compare action phases, speech scaling and premultiplied alpha.
Native tests cover menu commands, message budgeting, coalescing and handle cleanup.
PNG previews and bounded smoke traces complement these checks.

Physical mixed-DPI testing, installed MSI lifecycle and a published updater
cycle require separate acceptance. See [0.4.0 verification](verification-0.4.0.md)
and [remaining work](planning/PLAN-RU.md). Historical reports describe their
named versions; removed features in those reports are not current capabilities.
