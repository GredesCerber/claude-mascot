# Pixel activities

The catalog contains 78 activities: 75 stable source concepts plus three new
procedural scenes (flag, confetti stomp, dumbbell workout). Original IDs 0–74
are preserved. All 75 source concepts now have authored start/action/end
sequences in `src/pack_actions/simple.rs` and `advanced.rs`; they are not a
single per-category sparkle applied to static art.

`pack_actions::Frame` shares physical palm geometry with the canonical renderer.
Props have back and foreground passes; the latter holds cups, glasses and
costume edges without replacing the body or eyes. An activity's visual rig
ignores residual impact squash/rotation, while physics values keep updating.
Positions use whole logical pixels and the shared 12Hz pose clock. Mirroring
moves held objects and hands together; readable sign lettering stays readable.
User body colors apply to every pose, and no secondary humans/pets are painted.

The procedural scenes in `showcase.rs` use separate timed beats for the waving
flag, two hand-launched confetti bursts and two dumbbell repetitions. They are
original interpretations informed by
[the Codrops motion study](https://tympanus.net/codrops/2026/05/05/reverse-engineering-claude-ais-mascot-animations-with-svg-and-gsap/),
not imported SVG bodies or JavaScript animations. Grounded walking/jumping keeps
the existing physics and now has a short palm settling pose on touchdown.

## Source records

The user-supplied 75-image pack is recorded in `assets/activity-scenes.bin`.
The importer and sanitized props remain as the legacy fallback/reference path.
The authored activities replace these props at runtime; no source image/SVG
is loaded from the user's filesystem. To rebuild source records with Python:

```powershell
python rust/tools/Import-ActivityPack.py
```

The default source is sibling `work/idea-pack/claude-mascot-pack-SVG`. The importer
expects exactly 75 SVGs on the original source grid; changing their sorted order
would change stable IDs and is a compatibility change.

## Inspecting animations

```powershell
cargo run --manifest-path rust/Cargo.toml --example activity_frames -- rust/work/activity-review --samples 5
cargo run --manifest-path rust/Cargo.toml --example activity_frames -- rust/work/activity-left --facing -1 --scale 3 --samples 5 pixel-flag-waver reading-data-document
```

Without `--samples`, the exporter writes the full 12Hz sequence. It uses the
actual renderer, not separate demonstration artwork. Coverage and review notes:
[rollout](planning/activity-animation-rollout.md),
[simple scenes](handoffs/sol-simple-actions.md),
[advanced scenes](handoffs/sol-advanced-actions.md).

## Provenance

Source concepts: [Claude Mascot Pack](https://getillustrations.com/illustration-pack/claude-mascot-pack).
The original archive was supplied by the user. The runtime props and timelines
are procedural reinterpretations; raw source artwork is kept outside release
packages. Names and character remain subject to their owners' rights.
