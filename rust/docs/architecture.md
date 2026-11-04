# Rust port architecture

## Decision record

### Context

The Python application combines simulation, drawing, desktop discovery, menus,
and platform integration in one PyQt module. The Rust port needs native Windows
behavior, reproducible simulation, and local installation without changing the
existing Python/Linux implementation.

### Decisions

1. **Keep the Python application intact.** Rust lives under `rust/`; a user can
   continue using the original Windows or Linux version while the native port
   gains coverage.
2. **Keep behavior independent of Win32.** `engine` receives a `World` snapshot
   and mutates `Pet` values. It cannot create windows, move the real cursor,
   query the OS, or draw. This makes desktop physics and interactions
   deterministic and testable on non-Windows hosts.
3. **Use a small custom CPU renderer.** `render` draws the Claude pixel figure
   and Tagilla sprite atlas to a premultiplied BGRA buffer. The Windows adapter
   presents that buffer in native layered windows. This avoids binding mascot
   behavior to a full GUI framework and keeps alpha hit testing with the host.
4. **Keep settings as data.** `settings` reads versioned JSON and an optional
   sprite atlas path. It does not execute skin code or load plugins.
5. **Let the host own visibility and timing.** Fullscreen hiding, pause, window
   lifecycle, input events, and OS polling live in `main`/`platform`. Fullscreen
   is not a simulation pause: if the host elects to keep updating, the engine
   continues to advance.

## Module contracts

| Module | Owns | Must not own |
| --- | --- | --- |
| `lib.rs` | Shared `Vec2`, `Rect`, `WindowInfo`, `World` data types | OS calls or behavior |
| `engine.rs` | Per-pet state, bounded physics, surface tracking, commands, timed idle/rage, pairwise interactions | HWNDs, rendering, cursor writes, filesystem |
| `render.rs` | Character pixels, Tagilla atlas decoding, bubbles, particles/effects, PNG previews | Simulation transitions or desktop discovery |
| `effects.rs` | Window-attached cracks, shards, clipping and expiry | Changing foreign windows or accepting user input |
| `media.rs` | Background WinRT playback polling and process identity matching | Blocking the UI thread or guessing playback from a title |
| `platform.rs` | DPI setup, world snapshots, HWND creation/presentation, messages, mouse events, single-instance guard | Character state and behavior policy |
| `settings.rs` | Versioned configuration and startup preference | Arbitrary code or external process execution |
| `main.rs` | Startup, native loop, input-to-command routing, spawning/removal, previews and headless mode | Win32 details outside the platform adapter |

`World.windows` are supplied bottom-to-top. `Pet.pos` is the world-space point
between the feet, in physical desktop pixels. `Pet.surface` is an index into the
current window vector; the engine also tracks the native window ID so ordinary
reordering does not move a pet to another window. A disappeared surface causes a
fall. `video_window` is populated only when the media provider confirms playback;
the engine does not infer playback from a title.

`Pet::update` clamps a frame step to 50 ms, rejects invalid coordinates, applies
window carrying/shake-off, and advances one pet. The host then calls
`update_interactions` once for the same fixed step. Interaction RNG uses a
per-pet seed derived from the stable pet ID, so a replay with the same command
and snapshot sequence produces the same outcomes. Pairwise behavior never
targets another Tagilla or writes to the real cursor.

## Important invariants

- Desktop and application windows are never moved, closed, or sent input by the
  simulation. Tagilla's smash and crack effects are visual overlays only.
- Tagilla attack impacts are typed simulation events. `Pet::attack_serial`
  advances once per swing at the impact frame; the host may render local crack
  overlays for window attacks. The engine never changes the real window or
  system cursor.
- Invalid pet coordinates recover to the last known cursor or `(0, 0)` and clear
  the platform surface.
- Surface identity is the window ID, not just the enumeration index. Closing a
  window detaches the pet; normal window motion carries it; a large one-frame
  displacement throws it clear.
- Fullscreen visibility is controlled by the host's `hide_fullscreen` setting.
  Do not add a second fullscreen pause inside the engine.
- Manual sleep stays active until the engine observes idle time advance after
  the sleep command and then reset on new user input. Automatic sleep starts at
  60 seconds of platform-reported inactivity.
- `drag_to` and `release` only update simulation state. The native host owns
  pointer capture and decides when a click becomes a drag.
- Cursor chase/cling is simulated from `World.cursor`. Real cursor movement is
  not performed by the engine; Tagilla's cursor attack and thrown cursor effect
  still need explicit host implementation.

## Parity status and deliberate approximations

The original behavior inventory is in the root README and `CLAUDE.md`. The Rust
port is Windows-first; retain Python/Linux for KDE Plasma, Wayland input-device
handling, KWin scripting, and Linux installation.

| Area | Rust status | Gap or approximation |
| --- | --- | --- |
| Claude and Tagilla rendering | Implemented in custom renderer; embedded Tagilla sheet and external atlas supported | Claude drawing is a compact pixel approximation, not every Qt pose/eye detail |
| Physics and floor/window landing | Implemented with bounded steps and monitor work areas | Exploration/planning and jump arcs are simpler than the Python state machine |
| Window tracking | World snapshot contract and surface identity implemented | Occlusion/contact heuristics are simplified; no edge-by-edge climbing route planner |
| Mouse control | Click, drag, throw, jump, pet, and chase commands routed by host | Cursor-cling shake threshold is simplified; engine never moves the system cursor |
| Idle and fullscreen | Idle sleep/wake in engine; fullscreen visibility belongs to host | Mixed-DPI and application-specific fullscreen behavior need broader desktop QA |
| Video peek | Starts only when a confirmed video session and matching window exist | Positioning behind the video window and reveal/hide clipping are not yet parity complete |
| Tagilla smash | Real window target planning, approach/leap, typed impacts, 6.5-second input-transparent crack/shard overlays | Routes and crack patterns differ from Python; real cursor fling is not implemented |
| Multi-pet hunt | Deterministic chase/flee/bonk/block pass exists | Window jumping, dodge tactics, rage intervals/taunt variety are simplified |
| Menus, friends, installation | Native menus with checkmarks, tray, eight pets, shortcut installers and opt-in autostart | Menu layout differs; installation/uninstallation scripts have not been executed in the user's profile |
| Persistence | Versioned JSON settings and autostart value | Skin persistence is global, not a separate saved skin per pet |

## Verification map

| Check | Covers | How |
| --- | --- | --- |
| Engine regression tests | Floor/window landing, occlusion, closing/reordering surfaces, moving-window carry and shake-off, bounded `dt`, bad coordinates, idle sleep/wake, unsupported manual sleep, cursor cling shake-off, reachable confirmed-media peek, climb edge/ledge exit, multi-pet determinism and non-self-targeting | `cargo test --test engine_regression` |
| Renderer unit tests | Premultiplied alpha, embedded atlas loading, alpha-zero background, invalid atlas bounds | `cargo test` |
| Preview images | Both skins across representative states and speech bubbles | `cargo run -- --preview <directory>` |
| Headless run | Long deterministic updates and finite state | `cargo run -- --headless` |
| Windows smoke run | Native layered window, input routing, snapshot loop, clean exit metrics | `cargo run -- --smoke-test 5 --trace <file>` |
| Manual desktop matrix | 100/125/150% and mixed DPI, negative-origin monitors, alpha click-through, topmost/no-activate, full-screen hide toggle, real window move/shake/close, idle input, media play/pause, menu and startup behavior | Run on Windows with desktop interaction |

Automated tests do not replace the native desktop matrix: physical DPI behavior,
click-through hit testing, and z-order depend on the Windows shell and cannot be
fully validated by the platform-independent engine.
