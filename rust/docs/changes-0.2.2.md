# 0.2.2

- Keep a sleeping pet aligned with the desktop floor when the taskbar changes
  the monitor work area.
- Sample the pointer position on release, including any pause before release,
  so a stationary pet does not inherit an old nonzero throw velocity.
- Fix the GitHub release download route used by the portable updater.
- Reject MSI-managed destinations in the update helper and portable installer
  scripts, preserving Windows Installer ownership.
- Use the included mascot icon in the notification area and restore it after
  Explorer restarts; release the native icon when its host closes.
- Embed the mascot file icon in packaged Windows executables without requiring
  an additional resource compiler.
- Add a Start Menu uninstall shortcut for MSI and script installations.
- Preserve settings and unrelated files during portable removal, and clean the
  matching startup entry on MSI uninstall without disabling it on upgrades.
- Release a partially allocated DIB on failure and avoid zero-millisecond
  fallback waits for positive sub-millisecond durations.
- Record presented frames, physics steps and requested FPS in smoke traces.

Version 0.2.1 introduced configurable FPS, offscreen release recovery, landing
inertia, readable speech rendering, SVG/ICO icon assets and a per-user MSI.
