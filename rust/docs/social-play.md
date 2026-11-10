# Social play

The social coordinator uses stable pet IDs and fixed-size pair slots. Manual
requests are resolved against the current pet list on the next scheduler tick;
autonomous play is deterministic and infrequent. It never moves the operating
system cursor.

## Tag

Tag lasts about 7.5 seconds on a shared, sufficiently wide floor or visible
window ledge. One pet runs after its friend and the other runs toward the
available runway. A gentle catch swaps roles with a short cooldown. Both actors
remain within the support bounds. Dragging either pet, issuing a command,
removing a participant, closing their supporting window, or disabling social
play cancels the pair and clears its motion intents.

## Tower

Tower reserves all available pets on one support, up to eight. A group base is
chosen near the support center and the others jump in order of distance. Each
trajectory rises clear of the column before crossing it and perches only on a
descending head crossing. The complete group must fit by height and jump range;
an unsafe group is refused with a bubble instead of taking a silent subset.

After about 2.5 seconds together, upper pets dismount one by one, waiting for
the preceding pet to clear the column horizontally. Mixed sizes, support loss,
input, removal and timeout have explicit cancellation rules. Reserved members
do not receive mutual free-steering or body impulses during planned stack motion.

## Leapfrog

Two friends alternate between crouching and jumping across one another, changing
roles after landing. Headroom and a safe shared runway are required. Ordinary
walking can also trigger a single jump over a friend, separate from this game.

Greeting, dancing and comfort work from a distance without pulling friends
together. High-five and shared laptop use separate, closer, scale-aware ranges.

## Rough play

A short, friendly shove between two content Claudes on one wide support. Both
square up for 0.8 seconds with hands forward, the initiator runs in and stops
just short of body contact, and the partner receives a small playful hop with
about one roll, a short dizzy beat and a laugh. The shove needs room on the
partner's far side, both moods at least 40, and for autonomous selection random
activities enabled, intensity of at least 25 and moods of at least 55. It is a
rare, separately hashed choice. Sleep, dragging, manual commands, chase, lost
support, disabled social play and existing pair or tower ownership all prevent
or cancel it without a shove.

## Knock-back

Only genuinely hard hits tumble a Claude: a side contact closing at 480 px/s or
more (vertical drops need 950 px/s), a thrown body hitting a screen edge at
900 px/s or more, or a hard thrown floor landing. The receiving body must also
change velocity by at least 240 px/s, so walking, running, tag, leapfrog, jump
overs and a normal hop onto a friend's head never spin. Pets owned by a social
coordinator only tumble from much harder hits.

A tumble keeps the ordinary momentum transfer, gives a grounded body a bounded
upward pop, and spins in the direction of the push (at most 14 rad/s, lightly
damped). It has one or two floor bounces, then a 0.7 to 1.8 second dizzy
recovery with circling stars before Claude eases upright. A 0.45 second
cooldown prevents a second launch from the same collision. The body stays below
the top of the work area and inside its walls. Petting, jumping, dragging and
commands interrupt the tumble or dizziness. The renderer rotates only the body
sprite, sampled on the shared 12 Hz pose clock in 15 degree nearest-neighbor
steps; speech and the mood meter stay upright.

All interactions are cancelled when actor identity, support, or manual input
becomes invalid. No pair may claim a ghost actor after list compaction because
all participation is resolved by stable IDs every update.
