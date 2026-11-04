# Proposed pull request

## Title

Add a native Windows Rust desktop mascot alongside the Python app

## Body

### Summary

This pull request adds a Windows-first Rust implementation under `rust/` while
preserving the existing Python application and its Linux/KDE behavior. It adds
a native layered-window host, a platform-independent mascot simulation, a CPU
renderer, Windows media-session detection, JSON settings and atlas loading, and
Windows install, uninstall, build, and portable packaging scripts.

The Rust application targets Windows 10/11 x64. It can be built as a
self-contained executable, installed separately from the Python app, or run
from the extracted portable package. Startup behavior is opt-in.

### Implementation

- **Engine:** Keeps pet state and simulation separate from Win32 calls. It
  handles movement, window surfaces, idle behavior, commands, attacks, and
  multi-pet interactions using world snapshots supplied by the host.
- **Windows platform host:** Creates per-monitor-DPI-aware layered windows,
  handles input and menus, samples the desktop, manages the notification icon,
  and owns single-instance and native resource lifetimes.
- **Rendering:** Draws Claude and Tagilla into a premultiplied BGRA surface.
  Tagilla can use a custom PNG atlas with JSON frame coordinates; the app reads
  image and metadata as data and does not execute skin scripts.
- **Media:** Queries Windows Global System Media Transport Controls and only
  reports a video window when a visible matching app has an actively playing
  session identified as video. It does not infer playback from window titles.
- **Settings and packaging:** Stores versioned JSON preferences under the
  user's local application data, writes settings through a temporary file and
  replacement, and offers separate install/uninstall scripts. Autostart is
  opt-in.
- **Documentation:** Adds Rust usage, architecture, and verification notes
  without changing the Python application's existing documentation or
  installation flow.

### Validation

Run from `rust/`:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
cargo run -- --headless
cargo run -- --preview .\previews
```

On Windows, the native host can also be exercised with:

```powershell
cargo run -- --smoke-test 5 --trace .\smoke-trace.json
```

The Windows display matrix still requires manual checks for mixed DPI and
negative-origin monitors, transparent click-through, fullscreen hiding,
window movement, menu and tray actions, and real media sessions. Automated
simulation and native lifecycle tests do not establish those desktop behaviors.

### Known limitations

This is not pixel-perfect or feature-complete parity with the Python version.
Claude's pixel art and some movement are approximations; climbing routes,
video-peek placement, cursor attacks,
and multi-pet tactics remain simplified or incomplete. The engine never moves
the real system cursor, so cursor fling is not implemented. The Rust port does
not replace the Python implementation's Linux/KDE/Wayland support.

Window-smash planning and input-transparent crack/shard overlays are included.
They attach to a window ID, fade after 6.5 seconds, and disappear if the window
closes. Their routes and appearance differ from the original Qt implementation.

### Packaging

`Build.ps1 -Package` produces a portable Windows x64 zip containing the
executable, README, and install/uninstall scripts. `Install.ps1` installs into
`%LOCALAPPDATA%\claude-mascot-rust`; `Uninstall.ps1` removes that installation
without touching the Python app and preserves settings unless explicitly
asked to remove them.
