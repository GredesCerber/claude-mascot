# Verification: 0.3.0, 2026-11-06

Three Luna agents with low reasoning handled behavior, animation and the native
menu. The parent reviewed and integrated their changes, added regression cases
for grounded throws and wall turns, and checked the delivered artifacts.
Behavior references were the shared Python `mascot.py` implementation: cursor
chasing/catching, climbing and window exploration. This is not a Rust Linux host.

## Local checks

Windows, Rust/Cargo 1.99.1, `stable-x86_64-pc-windows-gnu`:

- Formatting, `cargo test --all-targets --locked`, Clippy with `-D warnings`,
  release compilation and Windows EXE icon embedding passed.
- 81 tests passed: 36 library, 1 binary, 43 engine regressions, 1 native lifecycle.
- Engine coverage includes a fast airborne rebound, a horizontal floor throw,
  low-speed momentum, a walking wall turn, adjacent monitor traversal at 240 Hz,
  chase hops/catches and release, autonomous reachable-window exploration,
  covered/offscreen target rejection, manual Sit/Sleep, and mood feedback.
- Climbing phases, dance/wave poses, the cursor-hand opening and velocity-trail
  suppression were tested. Representative PNGs were visually inspected.
- The actual native context menu and Settings submenu were opened and inspected.
  The mood display was disabled through the menu; the meter disappeared and the
  isolated QA configuration saved `show_mood: false`. The test instance exited
  cleanly. Production/user preferences were not edited by this check.
- The native menu construction test validates submenu structure, disabled
  character actions, radio selection, and a non-preset FPS display. Missing menu
  IDs are rejected before checking their state bits.
- A 600-second headless simulation checks finite position and bounded mood.
- 35 representative PNG previews are exported by the release executable.
- MSI is built, WiX-validated and decompiled. Its extracted EXE must match the
  standalone EXE's SHA-256. Expected ICE91 per-user-directory warnings remain.

Coverage percentage was not measured. Exact executable hashes and short native
FPS measurements are recorded in the delivered `BUILD-INFO.json`, `SHA256SUMS`
and `checks` directory.

## Mood and behavior semantics

Satisfaction starts at 75 and is per pet, per session. Throws faster than a
gentle reposition reduce it; clicks/petting improve it. Values below 75 recover
gradually, while bonuses above 75 are retained. A low mood changes the face,
meter and mild reactions, without enabling Tagilla's rage. The meter and speech
can be hidden independently from Settings. Explicit Sit/Sleep suppress window
exploration; exploratory launches have a randomized 8–18 second cooldown.

## Limits

Physical mixed-DPI/multi-monitor behavior, many media players, the complete
published-release update cycle, and actual MSI install/upgrade/uninstall were
not exhaustively tested. MSI runtime lifecycle was not executed on this user's
installation. Python behavior parity remains partial as documented in
`architecture.md`. The executable is unsigned. Microsoft review of the original
0.1.0 sample is a separate case; a local scan of a new build is not a final
verdict for that sample or for other computers.

Local verification: PASS. Windows native smoke/UI: VERIFIED for the checks above.
Installed MSI lifecycle and the published-release update cycle: NOT PERFORMED.
The existing source PR is updated; a public binary release is not published.
