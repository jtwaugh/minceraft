"""Batch 33 of the Mince Raft textures: the "Mince Raft" wordmark (#1423).

The title graphic for the menu: the pixel font's 3x5 capitals (draw_font.py)
doubled to 6x10, yellow with a brown bottom row and a near-black outline,
transparent elsewhere. One word gap, letters two
pixels apart. Well inside the menu's 160-texel layout width.
Run from the art/ directory: writes batches_33.json and checks sizes.
"""
import json

PAL = set("0123456789abcdef.")
SCALE = 2
GAP = 2
SPACE = 4

GLYPHS = {
    "m": ["101", "111", "111", "101", "101"],
    "i": ["111", "010", "010", "010", "111"],
    "n": ["110", "101", "101", "101", "101"],
    "c": ["011", "100", "100", "100", "011"],
    "e": ["111", "100", "110", "100", "111"],
    "r": ["110", "101", "110", "101", "101"],
    "a": ["010", "101", "111", "101", "101"],
    "f": ["111", "100", "110", "100", "100"],
    "t": ["111", "010", "010", "010", "010"],
}
TEXT = "mince raft"

# Solid letter pixels as (x, y) with a one-pixel margin for the outline.
on = set()
x = 1
for ch in TEXT:
    if ch == " ":
        x += SPACE - GAP
        continue
    for gy, row in enumerate(GLYPHS[ch]):
        for gx, c in enumerate(row):
            if c == "1":
                for dy in range(SCALE):
                    for dx in range(SCALE):
                        on.add((x + gx * SCALE + dx, 1 + gy * SCALE + dy))
    x += 3 * SCALE + GAP
W = x - GAP + 1  # right margin for the outline
H = 5 * SCALE + 2

grid = [["."] * W for _ in range(H)]
ring = {(px + dx, py + dy) for (px, py) in on
        for dx in (-1, 0, 1) for dy in (-1, 0, 1)} - on
for (px, py) in ring:
    grid[py][px] = "0"
for (px, py) in on:
    grid[py][px] = "5" if py == 5 * SCALE else "f"

WORDMARK = ["".join(r) for r in grid]
batch33 = {"wordmark": WORDMARK}
SIZES = {"wordmark": (W, H)}

errs = []
for k, v in batch33.items():
    w, h = SIZES[k]
    if len(v) != h or any(len(r) != w or set(r) - PAL for r in v):
        errs.append(f"{k}: not a {w}x{h} palette grid")
if W > 120:
    errs.append(f"wordmark is {W} wide; keep room within 160")
if errs:
    raise SystemExit("\n".join(errs))

out = {
    "Batch 33": {"title": "Wordmark", "tiles": False, "textures": batch33,
                 "sizes": SIZES},
}
json.dump(out, open("batches_33.json", "w"), indent=1)
print("ok", W, H)
