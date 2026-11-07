# Pixel activities

The catalog adapts all 75 SVG scenes from the user-supplied Claude mascot art
pack as **props and effects only**. The canonical Claude body, face, palette,
and proportions remain renderer-owned. The importer removes the artwork's
orange mascot pixels, attached skin-tone pixels, face details, alternate body
fills in the source body region, and nearby secondary mascot fragments. It
keeps accessory rectangles such as screens, cups, tools, books, plants, signs,
and scene decorations. A small glasses-like cluster in the reading scene is
preserved as an accessory.

The checked-in `assets/activity-scenes.bin` contains compact integer grid
rectangles, colors, prop tags, and source body anchor coordinates. It is built
into the executable. No source SVG/PNG parsing or external source-file lookup
occurs at runtime. Props draw on the same five-renderer-pixel source grid as
the canonical character and scale down only when a large scene would exceed
the 140-by-128-pixel fit bounds. Each scene anchors to its source mascot's
orange center and foot line; it never resizes or redraws the base mascot.

## Updating the compiled scenes

With Python 3, run:

```powershell
python tools/Import-ActivityPack.py
```

The default source folder is the sibling workspace folder
`work/idea-pack/claude-mascot-pack-SVG`. A different SVG folder can be supplied
as the script's first argument. The importer expects exactly 75 crisp pixel
scenes on the supplied 43.55-unit grid and rejects unsupported geometry or
colors. Catalog IDs follow case-insensitively sorted SVG filenames; changing
that order changes IDs and should be treated as a compatibility change.

The active scene uses timed opening and closing fades. Baking steam, growth,
code cursors, blinking status lights, tears, orbiting thought pixels,
confetti, and lift beats add small theme-specific motion. Tagged props such as
the rocket, barbell, and pie move independently while the base character stays
fixed. Themes with no retained accessory still receive a cosmetic animated
accent. Scenes with malware, server, or security names are illustrations only;
they make no real scanning, network, security, or server claims.

## Catalog API

`src/activities.rs` exports `ActivityId`, `ActivitySpec`, `ActivityGroup`,
`CATALOG`, and `draw_activity`. `ActivityId::new(index)` validates the stable
0–74 range. Each catalog item has a Russian title, cosmetic speech phrase,
category, source slug, and duration. `draw_activity` paints only props/effects;
the renderer draws canonical Claude separately. Category names are available
through `ActivityGroup::label_ru()`.

## Source and rights note

The underlying archive and its SVG/PNG files were supplied by the user for this
integration. No redistribution license or public-release permission was
identified in the supplied materials. Keep the raw source bundles outside the
executable. This note records provenance only; it does not infer or grant a
license for redistributing the original artwork or its adaptation.
