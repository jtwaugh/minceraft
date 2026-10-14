"""Export the full tileset for the viewer.

Merges every art/batches_*.json (in numeric order), adds the palette from the
PROMPT.md table, checks every grid, and writes one JSON file:

  {"palette": {"0": "#1a1a1a", ...},
   "batches": {"Batch 1": {"title", "tiles", "textures" | "models"}, ...}}

Usage (from the repo root): python3 art/tools/export_textures.py OUT.json
"""
import glob
import json
import os
import re
import sys

ART = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def load(name):
    with open(os.path.join(ART, name)) as f:
        return json.load(f)


with open(os.path.join(ART, "PROMPT.md")) as f:
    palette = dict(re.findall(r"^([0-9a-f]) (#[0-9a-f]{6})\b", f.read(), re.M))
if len(palette) != 16:
    sys.exit(f"expected 16 palette entries in PROMPT.md, found {len(palette)}")

def first_batch(path):
    m = re.search(r"batches_(\d+)", os.path.basename(path))
    return int(m.group(1)) if m else 0


batches = {}
for path in sorted(glob.glob(os.path.join(ART, "batches_*.json")), key=first_batch):
    for bname, batch in load(os.path.basename(path)).items():
        if bname in batches:
            sys.exit(f"{bname} appears in more than one batches_*.json file")
        batches[bname] = batch


def check(where, rows):
    if not rows or any(len(r) != len(rows[0]) for r in rows):
        sys.exit(f"{where}: ragged grid")
    for r in rows:
        for ch in r:
            if ch != "." and ch not in palette:
                sys.exit(f"{where}: bad palette index {ch!r}")


count = 0
seen = {}
for bname, batch in batches.items():
    for tname, rows in batch.get("textures", {}).items():
        check(f"{bname}/{tname}", rows)
        if tname in seen:
            sys.exit(f"texture {tname!r} is in both {seen[tname]} and {bname}")
        seen[tname] = bname
        count += 1
    for mname, faces in batch.get("models", {}).items():
        for fname, rows in faces.items():
            check(f"{bname}/{mname}.{fname}", rows)
            count += 1

with open(sys.argv[1], "w") as f:
    json.dump({"palette": palette, "batches": batches}, f, separators=(",", ":"))
print(f"exported {len(batches)} batches, {count} grids -> {sys.argv[1]}")
