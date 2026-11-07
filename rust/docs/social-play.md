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

Tower is a two-pet play first: the lower pet remains on the shared support while
the upper pet makes a real ballistic jump toward the lower pet. The upper pet
can perch only after normal physics reports a descending landing on its friend.
The coordinator requires enough headroom for the scaled jump and a clear side
runway for dismount. It holds for about 2.5 seconds, then the upper pet makes a
small physical side hop before gravity returns it to the floor. Narrow supports
and low ceilings are rejected. The coordinator keeps a small anchor relative to
the lower pet, allowing only the engine's bounded perch-follow correction while
the lower pet stands. The lower pet stays silent during the perch so the two
bubbles do not collide.

All interactions are cancelled when actor identity, support, or manual input
becomes invalid. No pair may claim a ghost actor after list compaction because
all participation is resolved by stable IDs every update.
