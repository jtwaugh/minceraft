"""Batch 22 of the Mince Raft textures: the pixel font's letters and punctuation.

Capitals A to Z, ten punctuation marks and font_unknown, each a 3x5 glyph in a 4x6 cell, drawn
like digit_* in draw_7_12.py: off-white glyph, near-black drop shadow one
pixel right and below, transparent elsewhere. Lower case reuses the
capitals and digit_* are the digits (#1174).
Run from the art/ directory: writes batches_22.json and checks sizes.
"""
import json

PAL = set("0123456789abcdef.")
W, H = 4, 6

GLYPHS = {
    "a": ["010", "101", "111", "101", "101"],
    "b": ["110", "101", "110", "101", "110"],
    "c": ["011", "100", "100", "100", "011"],
    "d": ["110", "101", "101", "101", "110"],
    "e": ["111", "100", "110", "100", "111"],
    "f": ["111", "100", "110", "100", "100"],
    "g": ["011", "100", "101", "101", "011"],
    "h": ["101", "101", "111", "101", "101"],
    "i": ["111", "010", "010", "010", "111"],
    "j": ["001", "001", "001", "101", "010"],
    "k": ["101", "101", "110", "101", "101"],
    "l": ["100", "100", "100", "100", "111"],
    "m": ["101", "111", "111", "101", "101"],
    "n": ["110", "101", "101", "101", "101"],
    "o": ["010", "101", "101", "101", "010"],
    "p": ["110", "101", "110", "100", "100"],
    "q": ["010", "101", "101", "110", "011"],
    "r": ["110", "101", "110", "101", "101"],
    "s": ["011", "100", "010", "001", "110"],
    "t": ["111", "010", "010", "010", "010"],
    "u": ["101", "101", "101", "101", "111"],
    "v": ["101", "101", "101", "101", "010"],
    "w": ["101", "101", "111", "111", "101"],
    "x": ["101", "101", "010", "101", "101"],
    "y": ["101", "101", "010", "010", "010"],
    "z": ["111", "001", "010", "100", "111"],
    # punctuation
    "period": ["000", "000", "000", "000", "010"],
    "comma": ["000", "000", "000", "010", "100"],
    "colon": ["000", "010", "000", "010", "000"],
    "exclamation": ["010", "010", "010", "000", "010"],
    "question": ["111", "001", "011", "000", "010"],
    "apostrophe": ["010", "010", "000", "000", "000"],
    "hyphen": ["000", "000", "111", "000", "000"],
    "slash": ["001", "001", "010", "100", "100"],
    "lparen": ["001", "010", "010", "010", "001"],
    "rparen": ["100", "010", "010", "010", "100"],
    # fallback for any character the font lacks: a question mark
    "unknown": ["111", "001", "011", "000", "010"],
}


def glyph(bits):
    g = [["."] * W for _ in range(H)]
    on = {(x, y) for y, r in enumerate(bits) for x, c in enumerate(r) if c == "1"}
    for (x, y) in on:
        if (x + 1, y + 1) not in on:
            g[y + 1][x + 1] = "0"
    for (x, y) in on:
        g[y][x] = "e"
    return ["".join(r) for r in g]


batch22 = {f"font_{k}": glyph(v) for k, v in GLYPHS.items()}

errs = []
for k, v in batch22.items():
    if len(v) != H or any(len(r) != W or set(r) - PAL for r in v):
        errs.append(f"{k}: not a {W}x{H} palette grid")
if len(batch22) != 37:
    errs.append(f"want 37 glyphs, got {len(batch22)}")
if errs:
    raise SystemExit("\n".join(errs))

out = {
    "Batch 22": {"title": "Pixel font", "tiles": False, "textures": batch22,
                 "sizes": {k: (W, H) for k in batch22}},
}
json.dump(out, open("batches_22.json", "w"), indent=1)
print("ok", {k: len(v["textures"]) for k, v in out.items()})
