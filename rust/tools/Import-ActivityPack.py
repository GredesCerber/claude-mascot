#!/usr/bin/env python3
"""Compile the supplied art pack into props-only integer rectangle data.

Uses Python's standard library only. Run from any directory:
    python tools/Import-ActivityPack.py [path/to/SVG-folder]
The source archive and SVGs remain outside the application and its binaries.
"""
from __future__ import annotations

import re
import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = Path(__file__).resolve().parents[3] / "idea-pack" / "claude-mascot-pack-SVG"
GRID = 43.55
ORANGE = bytes.fromhex("d97757")
SKIN = {bytes.fromhex(c) for c in ("ea9983", "e2967e", "ea8b71", "e58c73", "e79a83", "f09b80")}


def quantize(value: str) -> int:
    return int(round(float(value) / GRID))


def parse_svg(path: Path):
    root = ET.parse(path).getroot()
    rects = []
    for elem in root.iter():
        if elem.tag.rsplit("}", 1)[-1] != "rect":
            continue
        x, y = quantize(elem.attrib["x"]), quantize(elem.attrib["y"])
        w, h = quantize(elem.attrib["width"]), quantize(elem.attrib["height"])
        if min(w, h) < 1 or max(w, h) > 255:
            raise ValueError(f"Unexpected cell geometry in {path.name}")
        fill = elem.attrib.get("fill", "#000000")
        match = re.fullmatch(r"#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})", fill)
        if not match:
            raise ValueError(f"Unsupported fill {fill!r} in {path.name}")
        color = match.group(1)
        if len(color) == 3:
            color = "".join(ch * 2 for ch in color)
        rects.append((x, y, w, h, bytes.fromhex(color)))
    if not rects:
        raise ValueError(f"No pixel rectangles in {path}")

    orange = {i for i, r in enumerate(rects) if r[4] == ORANGE}
    if not orange:
        raise ValueError(f"No canonical orange mascot pixels in {path.name}")

    # Build exact-grid adjacency once. Components containing secondary skin
    # tones attached to canonical orange are treated as imported mascot pixels.
    at = {}
    for i, (x, y, w, h, _) in enumerate(rects):
        for cy in range(y, y + h):
            for cx in range(x, x + w):
                at.setdefault((cx, cy), []).append(i)

    def neighbors(i):
        x, y, w, h, _ = rects[i]
        found = set()
        for cy in range(y - 1, y + h + 1):
            for cx in range(x - 1, x + w + 1):
                found.update(at.get((cx, cy), ()))
        found.discard(i)
        return found

    # Find orange-cell groups and use their largest core as the placement
    # anchor. All orange is removed from the pack, including orange mini-pets.
    remaining, orange_components = set(orange), []
    while remaining:
        seed = remaining.pop()
        todo, comp = [seed], {seed}
        while todo:
            for j in neighbors(todo.pop()):
                if j in remaining and rects[j][4] == ORANGE:
                    remaining.remove(j)
                    comp.add(j)
                    todo.append(j)
        orange_components.append(comp)
    primary = max(orange_components, key=len)
    primary_rects = [rects[i] for i in primary]
    body_bounds = (
        min(r[0] for r in primary_rects), min(r[1] for r in primary_rects),
        max(r[0] + r[2] for r in primary_rects), max(r[1] + r[3] for r in primary_rects),
    )
    body_center = (body_bounds[0] + body_bounds[2]) * 0.5
    body_bottom = body_bounds[3]

    # Other substantial orange islands are secondary mascots. Their nearby
    # non-orange fills (skin, eyes, clothing) are excluded as well.
    secondary_boxes = []
    secondary_face_boxes = []
    for comp in orange_components:
        if comp == primary:
            continue
        rs = [rects[i] for i in comp]
        box = (min(r[0] for r in rs) - 1, min(r[1] for r in rs) - 1,
               max(r[0] + r[2] for r in rs) + 1, max(r[1] + r[3] for r in rs) + 1)
        secondary_face_boxes.append(box)
        if abs((box[0] + box[2]) * 0.5 - body_center) > max(3, body_bounds[2] - body_bounds[0]) \
           or abs((box[1] + box[3]) * 0.5 - (body_bounds[1] + body_bottom) * 0.5) > max(3, body_bounds[3] - body_bounds[1]):
            secondary_boxes.append(box)

    # Skin-tone rectangles connected to orange are imported body. The local
    # body box also catches dark/white face pixels and alternate outfit fills.
    imported_body = set(orange)
    queue = list(orange)
    seen = set(orange)
    while queue:
        i = queue.pop()
        for j in neighbors(i):
            if j in seen or rects[j][4] not in SKIN:
                continue
            seen.add(j)
            imported_body.add(j)
            queue.append(j)

    # Components let an accessory overlap the body box (laptop/cup/glasses)
    # without keeping the original Claude silhouette. Body-adjacent components
    # are removed unless they extend beyond the core or are a small, saturated
    # glasses-like cluster in the supplied reading scene.
    nonbody = set(range(len(rects))) - imported_body
    component_by_rect = {}
    components = []
    while nonbody:
        seed = nonbody.pop()
        todo, comp = [seed], {seed}
        while todo:
            i = todo.pop()
            for j in neighbors(i):
                if j in nonbody:
                    nonbody.remove(j)
                    comp.add(j)
                    todo.append(j)
        ci = len(components)
        components.append(comp)
        for i in comp:
            component_by_rect[i] = ci

    accessory_components = set()
    for ci, comp in enumerate(components):
        rs = [rects[i] for i in comp]
        total = sum(r[2] * r[3] for r in rs)
        outside = sum(
            max(0, r[0] + r[2] - body_bounds[2]) * r[3]
            + max(0, body_bounds[0] - r[0]) * r[3]
            + max(0, r[1] + r[3] - body_bounds[3]) * r[2]
            + max(0, body_bounds[1] - r[1]) * r[2]
            for r in rs
        )
        if outside >= 3 and (outside == total or outside / total >= 0.2):
            accessory_components.add(ci)
        elif path.stem == "reading-data-document" and total <= 24:
            accessory_components.add(ci)

    kept = []
    for i, (x, y, w, h, rgb) in enumerate(rects):
        is_dark_or_white_face = (max(rgb) <= 90 or min(rgb) >= 232)
        in_source_face_zone = (
            body_bounds[0] - 1 <= x < body_bounds[2] + 1
            and body_bounds[1] - 1 <= y < body_bounds[3] + 1
        ) or any(bx0 <= x < bx1 and by0 <= y < by1 for bx0, by0, bx1, by1 in secondary_face_boxes)
        if i in imported_body or is_dark_or_white_face and in_source_face_zone:
            continue
        if any(bx0 <= x < bx1 and by0 <= y < by1 for bx0, by0, bx1, by1 in secondary_boxes):
            continue
        inside_core = body_bounds[0] <= x < body_bounds[2] and body_bounds[1] <= y < body_bounds[3]
        if inside_core and component_by_rect.get(i) not in accessory_components:
            continue
        # A large body/costume component entirely within the mascot bounds is
        # never an accessory, even if it has many pixels.
        ci = component_by_rect.get(i)
        if inside_core and ci is not None:
            comp = components[ci]
            rs = [rects[j] for j in comp]
            comp_outside = any(
                r[0] < body_bounds[0] or r[1] < body_bounds[1]
                or r[0] + r[2] > body_bounds[2] or r[1] + r[3] > body_bounds[3]
                for r in rs
            )
            if not comp_outside and ci not in accessory_components:
                continue
        kept.append((x, y, w, h, rgb, 0))

    # Tag the largest surviving object so selected actions can move the actual
    # prop, while all other scene pixels remain at their original grid cells.
    kept_components = {}
    for i, rec in enumerate(kept):
        x, y, w, h, _, _ = rec
        original = at.get((x, y), [])
        ci = next((component_by_rect[j] for j in original if j in component_by_rect), -1)
        kept_components.setdefault(ci, []).append(i)
    main_prop = max(kept_components.values(), default=[], key=len)
    for i in main_prop:
        x, y, w, h, rgb, _ = kept[i]
        kept[i] = (x, y, w, h, rgb, 1)

    if any(v < 0 or v > 255 for v in body_bounds):
        raise ValueError(f"Mascot anchor outside compact format: {path.name} {body_bounds}")
    return body_bounds, kept


def main():
    src = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else DEFAULT_SOURCE
    files = sorted(src.glob("*.svg"), key=lambda p: p.name.casefold())
    if len(files) != 75:
        raise SystemExit(f"Expected exactly 75 SVG scenes in {src}, found {len(files)}")
    scenes = [parse_svg(path) for path in files]
    cursor = 4 + 1 + 4 * len(scenes)
    offsets = []
    for _, cells in scenes:
        offsets.append(cursor)
        cursor += 10 + 8 * len(cells)
    output = bytearray(b"APSP" + bytes([len(scenes)]))
    output.extend(b"".join(struct.pack("<I", offset) for offset in offsets))
    for (body_min_x, body_min_y, body_max_x, body_max_y), cells in scenes:
        if cells:
            min_x, min_y = min(c[0] for c in cells), min(c[1] for c in cells)
            max_x, max_y = max(c[0] + c[2] for c in cells), max(c[1] + c[3] for c in cells)
        else:
            min_x = min_y = max_x = max_y = 0
        output.extend(struct.pack("<BBBBBBBBH", min_x, min_y, max_x, max_y,
                                  body_min_x, body_min_y, body_max_x, body_max_y, len(cells)))
        for x, y, w, h, rgb, tag in cells:
            output.extend(struct.pack("<BBBBBBBB", x, y, w, h, *rgb, tag))
    target = ROOT / "assets" / "activity-scenes.bin"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(output)
    print(f"Compiled {len(scenes)} props-only scenes, {sum(len(c) for _, c in scenes):,} cells, "
          f"{len(output):,} bytes -> {target}")


if __name__ == "__main__":
    main()
