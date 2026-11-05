# Verification: 0.2.2, 2026-11-06

Local Windows verification used Rust/Cargo 1.99.1 and the
`stable-x86_64-pc-windows-gnu` toolchain. This record supersedes the old
0.1.0 verification record for the files delivered as 0.2.2.

## Team and acceptance

Three Luna agents, medium reasoning, reviewed separate areas. The engine owner
fixed sleeping pets following taskbar work-area changes; the native owner fixed
tray icon ownership/recovery, DIB cleanup and timer fallback; the delivery owner
fixed the updater route and MSI ownership guards. The engine owner then performed
a separate read-only integration review. The parent integrated changes, fixed
stationary pointer releases, and ran the final Windows checks below.

| Check | Result |
| --- | --- |
| Formatting | PASS: `cargo fmt --check` |
| Compilation and release build | PASS: `cargo build --release --locked` |
| Lint | PASS: `cargo clippy --all-targets --locked -- -D warnings` |
| Tests | PASS: 63 total, 33 library + 1 binary + 28 engine + 1 Windows lifecycle |
| Native resources | PASS: create/present/resize/drop, repeated tray creation, simulated `TaskbarCreated`, bounded GDI/USER counts |
| Simulation | PASS: 36,000 steps, 600 simulated seconds, two pets, finite state |
| Rendering | PASS: 19 exported state PNGs; speech previews at 0.5/1/1.5/2 scale; representative previews inspected |
| EXE icon | PASS: seven ICO images embedded with Windows resource APIs; large/small icon extraction and post-embedding native runs passed |
| GitHub transport | PASS: the actual release executable's `--check-updates` completed with current 0.2.2 and no available matching update |
| MSI | PASS: built, WiX validated and decompiled; expected ICE91 warnings for intentionally per-user directories |
| MSI payload | PASS: extracted executable SHA-256 equals the tested release executable |
| PowerShell | PASS: Windows PowerShell 5 parsing/UTF-8 BOM, sandbox MSI guards and uninstall with unrelated-file preservation |
| Uninstall routing | PASS: MSI shortcut/action decompiled, cleanup before file removal, skip on major upgrade; exact-path startup matcher tests passed |
| Local Defender | Custom scan completed; executable remained present with unchanged SHA-256, no matching detection history; definitions 1.463.135.0 |
| Coverage | Not measured; test counts do not establish percentage coverage |

The final release executable SHA-256 is
`284f4a61a2ea67c5aaa12695b44e3e7c6028c1d3270e972a035ec7b8e4ffddcf`.

## Native rendering measurements

Each run used a separate temporary configuration, one pet, fullscreen hiding
disabled and a three-second native smoke duration. Counts below are actual
presentations, not just loop iterations. The existing user instance was not
terminated. Physics used the same fixed 240 Hz timestep across all runs.

| Requested FPS | Presented frames | Measured FPS |
| --- | --- | --- |
| 30 | 90 | 30.00 |
| 60 | 180 | 59.99 |
| 90 | 270 | 89.99 |
| 120 | 360 | 119.98 |
| 144 | 432 | 143.99 |
| 240 | 720 | 239.98 |

These are short measurements on this machine, not a performance guarantee for
other displays, many pets or background workloads.

## Verification boundaries

- MSI installation, upgrade and removal were not executed on the user's system;
  validation and payload inspection do not establish those runtime behaviors.
- The full published-release download/replacement/restart cycle was not run.
  The helper's replacement, digest, rollback, path and MSI guard cases passed
  tests; the live network check only tested release discovery.
- Physical mixed-DPI/multi-monitor interaction, real media-player combinations
  and a real Explorer restart need broader desktop testing. `TaskbarCreated`
  was simulated without restarting the user's shell.
- EXE icon embedding is a Windows packaging step in `Build.ps1` and both Windows
  workflows. A bare `cargo build` does not perform this post-build resource step.
- This remains a Windows-first port with the approximations documented in
  `architecture.md`, not complete parity with Python/Linux behavior.
- Microsoft review of the original 0.1.0 Wacatac detection is independent of
  these checks. Its final verdict was not rechecked in this pass. Neither MSI
  packaging nor passing tests establishes an antivirus verdict.
- The final executable is unsigned. GitHub Actions was not run locally; local
  Windows checks do not establish the upstream maintainer's CI acceptance.

Local checks: PASS. Target acceptance: VERIFIED for the bounded Windows native
smoke runs above; NOT PERFORMED for installed MSI lifecycle and the complete
published-release update cycle. No public binary release was published.
