# Simple pack actions

Implemented in `rust/src/pack_actions/simple.rs`, using the shared `Frame`, `cell`, and `alpha` API. Inspected the actual PNG contact sheets for all IDs before authoring. Every listed ID has distinct hand keyframes, timing boundaries, a preparation, an identifiable main action, and a recovery. The body and eyes remain renderer-owned.

| ID | Scene | Authored action |
|---|---|---|
| 1 | Крепкий кофе | Take the handled cup, raise its rim to the face, hold a sip, show the coffee surface, return it. |
| 3 | Удачный API | Raise the API placard, lift the response card, reveal its check, lower both. |
| 5 | Сонная улыбка | Bring hands toward the eyes, rub in two positions, settle, release the sleepy gesture. |
| 6 | Зелёный щит | Lift edge-first, turn the shield toward the viewer, brace against one edge impact, lower. |
| 11 | Знак ошибки | Raise the magenta error paddle, recoil, inspect, acknowledge, lower. |
| 16 | Сердце идеи | Lift the heart medallion between both hands, present its golden rim, lower. |
| 18 | Песочные часы | Turn the hourglass through a horizontal pose, let sand run from top to bottom, return upright. |
| 20 | Грустное сердечко | Lift two jagged heart halves, separate them, bring them together, lower. |
| 23 | Капли грусти | Let tears descend, bring tissues to the eyes, wipe and lower the hands. |
| 30 | Зелёный шар | Lift the globe by its handle, reach with the free hand, shift continents to turn it, lower. |
| 36 | Древний свиток | Lift its rod, unroll parchment to two widths, read rows, roll back onto the rod. |
| 37 | Зелёный лист | Pinch the stem, lift, wave left/right/left, lower. |
| 38 | Лампочка идеи | Raise an unlit bulb, switch on and present it, switch off before lowering. |
| 40 | Мягкое сияние | Open hands, gather separated arcs into a light ring, hold, contract the ring and relax. |
| 42 | Знак зависимости | Raise a two-handed IT DEPENDS board, present the message, lower. |
| 45 | Облачко любви | Form and release a heart, let it rise while a warm cloud forms, disperse and relax. |
| 54 | Чтение документа | Lift a document, wear open-frame glasses, scan rows, turn one page around its spine, lower. |
| 59 | Замочек с ключом | Hold the open lock, insert the key into its keyhole, close the shackle, turn, withdraw and tug-test. |
| 61 | Крепкий сон | Fold hands, settle lower, release a rising Z trail, return upright. |
| 64 | Розовая карточка | Raise the pink alert window, reach its close control, dismiss its warning, fold away. |
| 67 | Пузырёк мыслей | Touch chin, grow one thought dot into a three-dot bubble, dismiss it. |
| 72 | Добро пожаловать | Unfurl a WELCOME banner between both hands, hold, roll down. |
| 73 | Зелёная галочка | Lift a green badge, draw the check in two strokes, present, lower. |

All scenes set `replace_props` and `replace_effects` through `Frame::neutral()`. Unsupported IDs return `None`. Hands are absolute relative to the canonical body, and props add exactly the same `body_x` and `bob` as the renderer. No body/face copies, image decoding, runtime SVG/HTML, sinusoidal interpolation, or randomness are used.

The foreground API draws the coffee cup, tears/tissues, and the open glasses rims. Other props remain in the background pass. Mirroring applies to whole prop cells; API/IT DEPENDS/WELCOME letter glyphs remain readable on the opposite side. All authored coordinates fit x[-28,28], y[-43,0]. Local appearance/retirement opacity uses the common `alpha` helper so props finish fading before the neutral recovery pose, and both passes are empty at and after duration.

Seven module tests cover supported IDs, neutral/empty boundaries, contact with held objects, mirrored geometry and floor bounds, distinct keyframe signatures, the key tip meeting the keyhole, and the leaf's two waves versus a document's single turn. `rustfmt --edition 2021 rust/src/pack_actions/simple.rs` passed. Parent owns Cargo execution and full renderer validation.

Integration dependency: scenes 30, 54, 59 and 64 deliberately use the free hand across the body to contact the globe, page, keyhole and window control. Root is adding the common stepped shoulder/elbow connectors. Glasses leave the common eyes visible. The eight-pose designs are intentionally crisp and the source illustrations are reinterpreted as original procedural pixel objects, not traced assets.
