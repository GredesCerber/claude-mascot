# Context, distance and contact

The desktop simulation uses local rules and public window/cursor snapshots.
It does not send desktop context to an online model, read folder contents or
observe keyboard input. Decisions have explicit reasons, priorities and
cooldowns so a pet can keep its current intention instead of changing every frame.

Dialogue uses 37 event/role banks and four scene-specific lines for each of the
75 activities. It distinguishes opening/navigation in Explorer from merely
refocusing an existing folder window. Recent lines are remembered, repeated
same-event clicks are throttled and ambient speech waits after an action.
Native sessions reseed only speech; deterministic physics randomness is untouched.
Close social partners take turns showing bubbles.

Manual input comes first. Dragging, explicit sleep/sit, a selected activity or
an owned social scene prevents optional idle reactions from replacing it.
Explorer activity, nearby friends, available support and mood can invite a
brief observation or rest. Existing cursor play and video-window interest use
their own bounded routes. A quiet pet continues to obey manual commands.

## Personal space

The default free gap between body edges is 24–96 pixels at scale 1; the body
half-width allowance is 40 pixels per pet and scales with the pet. The selected
range is a settling band, not a leash: pets may wander farther apart and do not
have to approach one another to greet, dance or offer comfort. Contact actions
such as high-five have a separate, closer distance range.

Crowding produces bounded steering with hysteresis. During ordinary walking,
a pet can jump over a friend when the runway and headroom allow it. Otherwise
it yields or waits. This is distinct from throwing a pet into another pet.
Sleep, manual actions, different windows and monitor-separated supports are
not treated as a reason to pull two pets together.

The settings window exposes the range, and the context menu offers compact,
normal and spacious presets. `--spacing MIN:MAX` saves a custom range, for
example `--spacing 32:112`. Values are checked before use.

## Physical contact

The contact coordinator checks at most eight bodies (28 pairs), including
swept fast crossings. Strong hits exchange velocity using scaled body mass
and restitution. A dragged pet is kinematic, preserving the pointer's control.
Soft contact resolves overlap without repeatedly injecting impulse energy.
Members of a coordinated tower follow its assembly/dismount plan; unrelated
impacts can interrupt their scene. Contact and spacing contain no native UI calls.

## Preferences and color

The modeless native settings window shares the app's bounded message pump.
It provides frame rate, scale, activity intensity, personal space, display and
behavior switches, and activity commands. Intensity changes optional activity
frequency, not the physics step or the presentation frame rate.

Each live Claude can use its own body color. Native palette presets and the
system color picker derive matching shadows/highlights from one RGB color.
The original palette remains exact; dark colors use readable light eyes.
Accessories keep their own colors. The last selected color is saved as the
default for newly invited pets and future sessions; a persistent multi-pet
roster is not implemented.

## Exploration routes (0.8.0)

Pure geometry planners are separated from `engine_exploration.rs`, which owns
approach, crouch, flight, climb and ledge arrival. Decisions use the simulation
RNG; safety checks use window IDs and geometry only. Normal transitions use
17% exploration / up to 25% ledge-descent thresholds, bounded by availability,
intensity and cooldowns. Manual commands and social activities have priority.
A route has a 45-second maximum lifetime. Physics runs during airborne travel;
missing destinations do not teleport or stop the pet in midair.

Climbing starts only after walking to the actual edge position, preserves feet
height on attachment and uses ordinary vertical climb motion. Rollover walks
onto the top. Jump preparation holds a fixed pixel pose for 0.22 seconds.
Descent can first stage at a safe ledge and must clear its old platform on the
returning arc. See [upstream comparison](behavior-parity.md).
