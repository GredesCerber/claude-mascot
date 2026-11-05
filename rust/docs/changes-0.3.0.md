# 0.3.0

- Rebound from outer screen walls on a fast throw, with reduced speed and
  squash feedback. Adjacent monitor seams remain traversable at the fixed
  simulation timestep, and slow throws retain their horizontal momentum.
- Run and hop toward the cursor, catch it in the air at the hand anchor, and
  drop naturally when chase is disabled. Re-grabbing has a cooldown.
- Occasionally explore reachable window tops autonomously, with an 8–18 second
  cooldown between exploration launches. Covered and offscreen tops are skipped;
  explicit Sit and Sleep keep their meaning.
- Animate climbing hands/legs, dangling arms, dances and waves. Short tapered
  velocity trails disappear while held or nearly stationary.
- Add per-pet satisfaction: fast throws lower it, petting raises it, and time
  gradually restores a low mood. The optional meter and Claude's eyebrows show
  the result. Satisfaction is reset when a new pet/session starts.
- Group the native context menu into behaviors, characters/friends and settings,
  with quick actions, radio selections and unavailable character actions disabled.
- Keep the MSI uninstaller, EXE icon and guarded update behavior from 0.2.2.

Shared Python behavior in `mascot.py` was used as the reference for climbing,
chasing, cursor grabs and exploration. Rust remains Windows-first; its complete
Python/Linux behavior parity is not claimed.
