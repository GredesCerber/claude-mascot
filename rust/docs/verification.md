# Verification record — 2026-11-04

Verified locally on Windows 11 Pro, build 10.0.26300, with Rust/Cargo 1.99.1
and `stable-x86_64-pc-windows-gnu` installed through rustup. The delivered
executable uses GNU; the local MSVC linker/SDK were unavailable.

## Automated checks

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --locked --all-targets` | Passed: 36 tests (13 library, 22 engine, 1 native lifecycle) |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo build --locked --release` | Passed |
| Release `--headless --trace …` | Passed: 36,000 steps / 600 simulated seconds, two pets, finite state |
| Release `--preview …` | Passed: 19 PNGs including both characters and cracks; representative images visually inspected |
| Release `--smoke-test 6 --config … --trace …` | Passed: clean exit at 6.029 seconds, 209 loop frames, 104 world snapshots, one pet |
| Native lifecycle | Passed: 50 create/present/resize/drop cycles, bounded GDI/USER object counts, transparent/no-activate effect styles, singleton mutex lifetime |
| PowerShell scripts | Build/Install/Uninstall parse without syntax errors; installation/removal not executed |

The smoke configuration disabled fullscreen hiding to exercise visible drawing.
An earlier 180-second interactive inspection also exited cleanly. Character
and menu rendering, scale selection and settings persistence were observed.
Synthetic mouse input stopped when the computer-use tool detected concurrent
user input.

## Delivered executable

- Size: **976,896 bytes**.
- SHA-256: `72c201b282ca6e66d0e1dcfb1b06c1edb1e9ab81494fb12183e1bf8ba4442295`.
- A short sampled run reached approximately **40.29 MiB working set**. This is
  a smoke measurement, not a Python comparison or a long-session memory guarantee.
- PE imports use Windows system DLLs; no Python, Rust, Qt, MinGW or WebView
  runtime DLL must be shipped with the executable.

## Remaining desktop verification

Mixed DPI, physical negative-origin monitor layouts, precise cross-process
click-through, interactive drag/throw/double-click, real media play/pause across
player variants, long-running multi-pet behavior, shell/tray recovery, and
autostart/install/uninstall need broader manual testing. Simulation tests do
not establish every desktop behavior or pixel-perfect Python parity.

The CI workflow configures Windows build/test/lint and Linux simulation checks;
GitHub Actions was not run in this local task.
