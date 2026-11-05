# 0.2.1

- Recover pets released beyond the desktop bounds, including below the screen.
- Follow the desktop work area when the taskbar appears or hides.
- Keep horizontal momentum on landing and damp it with friction.
- Render at 60 FPS by default; choose 30, 60, 120, 144 or 240 in the menu,
  or any integer from 30 through 240 with `--fps`.
- Run physics at a fixed 240 Hz independently of rendering.
- Add a per-user MSI with application icons, shortcuts, upgrade support and
  Windows Installer ownership of installed executable updates.
- Include SVG, PNG and ICO mascot icon assets.
- Draw larger antialiased speech text at the final display resolution, with
  an opaque dark background; retain nearest-neighbor scaling for the pet.

The executable is unsigned. Packaging as MSI does not establish an antivirus
verdict. Microsoft sample review of the original 0.1.0 executable is separate
from validation of this version.
