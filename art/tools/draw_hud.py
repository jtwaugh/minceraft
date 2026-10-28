"""Batch 24 of the Mince Raft textures: HUD art. For now, the crosshair.

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

batch24 = {"crosshair": CROSSHAIR}

errs = []
for k, v in batch24.items():
    if len(v) != N or any(len(r) != N or set(r) - PAL for r in v):
        errs.append(f"{k}: not a {N}x{N} palette grid")
if errs:
    raise SystemExit("\n".join(errs))

out = {
    "Batch 24": {"title": "HUD", "tiles": False, "textures": batch24,
                 "sizes": {k: (N, N) for k in batch24}},
}
json.dump(out, open("batches_24.json", "w"), indent=1)
print("ok", {k: len(v["textures"]) for k, v in out.items()})
