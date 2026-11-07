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

All interactions are cancelled when actor identity, support, or manual input
becomes invalid. No pair may claim a ghost actor after list compaction because
all participation is resolved by stable IDs every update.
