"""Batch 24 of the Mince Raft textures: HUD art. The crosshair and the pressed panel.

A 7x7 plus with a clear (transparent) centre pixel, so the block being aimed
at shows through; off-white arms, `.` transparent elsewhere (#1338).
Run from the art/ directory: writes batches_24.json and checks sizes.
"""
import json

PAL = set("0123456789abcdef.")
N = 7
C = N // 2

CROSSHAIR = [
    "".join("e" if (x == C) != (y == C) else "." for x in range(N))
    for y in range(N)
]

# Pressed button panel: the panel_* nine-slice (batch 10) with the bevel
# inverted, dark on the top and left, light on the bottom and right (#1422).
PANEL_PRESSED = {
    "panel_pressed_tl": [".000", "0555", "0566", "0566"],
    "panel_pressed_t": ["0000", "5555", "6666", "6656"],
    "panel_pressed_tr": ["000.", "5570", "6670", "6670"],
    "panel_pressed_l": ["0566", "0566", "0556", "0566"],
    "panel_pressed_c": ["6665", "6666", "6566", "6666"],
    "panel_pressed_r": ["6670", "5670", "6670", "6670"],
    "panel_pressed_bl": ["0566", "0566", "0577", ".000"],
    "panel_pressed_b": ["6666", "6566", "7777", "0000"],
    "panel_pressed_br": ["6670", "6670", "7770", "000."],
}

batch24 = {"crosshair": CROSSHAIR, **PANEL_PRESSED}
SIZES = {k: (N, N) if k == "crosshair" else (4, 4) for k in batch24}

errs = []
for k, v in batch24.items():
    w, h = SIZES[k]
    if len(v) != h or any(len(r) != w or set(r) - PAL for r in v):
        errs.append(f"{k}: not a {w}x{h} palette grid")
if errs:
    raise SystemExit("\n".join(errs))

out = {
    "Batch 24": {"title": "HUD", "tiles": False, "textures": batch24,
                 "sizes": SIZES},
}
json.dump(out, open("batches_24.json", "w"), indent=1)
print("ok", {k: len(v["textures"]) for k, v in out.items()})
