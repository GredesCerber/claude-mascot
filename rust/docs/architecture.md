# Rust port architecture (0.8.0)

The Python application at the repository root is preserved. Rust is Windows-first
and Claude-only. Combat, atlas decoding and crack overlays were removed in 0.4.0.

| Module | Responsibility |
| --- | --- |
| lib.rs | Physical-pixel coordinates, rectangles, world/window snapshots |
| engine.rs | Deterministic physics, support tracking, mood, commands and timed activities |
| engine_exploration.rs | Ownership, cancellation and bounded progression of approach/crouch/flight/climb routes |
| exploration*.rs | Pure geometry for varied jump targets, edge approaches and voluntary descent |
| contacts.rs | Body contacts, swept crossings, scaled masses and impact impulses |
| behavior.rs | Personal space, safe jump-over avoidance and context decisions |
| preferences.rs | Modeless native controls, bubble-style theme and settings events |
| palette.rs | User-selected body colors with consistent shading and readable eyes |
| social.rs | Fixed-capacity pair coordinator, stable IDs, 10 Hz selection, smooth timelines and cancellation |
| activities.rs | Props adapted from all 75 illustrations, Russian catalog, bounded decorative timelines and accents |
| dialogue.rs / activity_dialogue.rs | Contextual phrase banks, independent speech RNG, recent-line history and chatter cooldowns |
| render.rs | Procedural Claude/laptop pixels, gestures, cached fonts, transparent BGRA and PNG previews |
| platform.rs | Win32 windows, DPI, alpha hit testing, bounded message pump, capture, menus, tray |
| desktop.rs | Dedicated UIA worker, bounded cached desktop-icon geometry, stale-cache expiry |
| main.rs | Fixed-step simulation, frame deadlines, input routing, visibility and lifecycle |
| timing.rs | FPS pacing and rolling drag velocity |
| settings.rs | Versioned JSON and legacy character migration |
| media.rs | Background best-effort playback monitoring |
| update*.rs | Portable staging, digest checks, rollback and MSI ownership safeguards |

Each physics step advances pets, resolves contacts, updates free behavior, then
advances social scenes, using bounded storage and stable IDs. Owned scenes
exclude free steering; removed IDs release their context slots.

The engine consumes a World snapshot and never draws, creates windows, moves
the real cursor or sends input to other applications. Pet.pos is between its
feet. Surfaces follow native IDs across enumeration reordering; losing support
causes a fall. Window snapshots reuse vector capacity. Removing combat also
removes the host's per-physics-step cloning of all pets for pairwise interactions.

Physics advances at 240 Hz independently of the selected 30..240 render FPS.
Sprite poses and prop timelines share a stepped 12 Hz rendering clock.
World position, velocity, contacts and input keep their physics rate.
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

Social pairs reserve at most two pets on the same physical support; independent
pair slots and cooldowns prevent one pet from joining two scenes. The coordinator
borrows pets through a fixed array of up to eight references. Active timers and
cancellation advance each physics step; only new pair selection runs at 10 Hz.
The host also validates ownership after input/removal without advancing time.
Gaze is a bounded, smoothed world direction; the renderer mirrors it into local
face coordinates. The laptop shows its rear lid to the viewer.

Tag, leapfrog and group towers send bounded intents through normal physics.
Towers reserve all eligible members of one support after checking whole-group
height and trajectories; upper members dismount in order. Manual commands
clear ownership before new input takes effect.
Responsive behavior compares bounded window metadata at 10 Hz, rebaselines
while disabled, and gives manual commands priority. Nearby cursor interest is
short-lived and independent of the user's persistent chase setting.
Desktop UIA reads run off the presentation thread, cache at most 128 icons,
expire after three seconds, and expose no names or paths. Icon surfaces are
below real windows in the world snapshot. Leap strength adapts to the requested
height while reserving headroom for the scaled body.

Pack prop data is compiled from SVG rectangles by an offline importer.
Imported character bodies/faces are removed. The renderer always draws the
canonical Claude for every activity, sharing body proportions, palette, eye
geometry and scale; only gestures, expression and accessories vary. This keeps
artwork variations from changing the pet's style.
The executable reads integer cell records directly, without parsing SVG or PNG
on animation frames. Catalog activities use one Activity state plus an ID,
so the state machine does not need 75 duplicate branches. Preset start scenes
can be selected with --activity SLUG; --pets 1..8 starts additional Claudes.

Legacy Tagilla settings resolve to Claude. The retired custom_atlas key is
ignored on read and omitted on write. Unknown character names still fail.

## Verification boundaries

Engine tests cover support loss, taskbar changes, monitor seams, offscreen
release, bounded motion, climbing, chase, mood and timed activities.
Renderer tests compare action phases, speech scaling and premultiplied alpha.
Native tests cover menu commands, message budgeting, coalescing and handle cleanup.
PNG previews and bounded smoke traces complement these checks.

Physical mixed-DPI testing, installed MSI lifecycle and a published updater
cycle require separate acceptance. See [0.7.0 verification](verification-0.7.0.md)
and [remaining work](planning/PLAN-RU.md). Historical reports describe their
named versions; removed features in those reports are not current capabilities.
