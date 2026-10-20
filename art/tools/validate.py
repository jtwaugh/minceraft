"""Check every batches_*.json file in art/.

Rules:
- every character is a palette index (0-9, a-f) or "." for transparent;
- every texture and face is a rectangle (all rows the same length);
- textures are 8x8 unless the batch lists them in "sizes" as [w, h];
- model faces match the batch's "face_sizes" when it has one, and a model
  must have exactly the faces listed there;
- no texture or model name appears in two batches;
- every tool and item `name` in data/items.toml has a texture of that name
  (pass another items.toml path as the first argument to check that instead).

Run from the art/ directory: python3 tools/validate.py
Exits non-zero and lists every problem if anything fails.
"""
import glob
import json
import sys
import tomllib

PAL = set("0123456789abcdef.")
errors = []
seen = {}


def check(where, rows, want=None):
    if not rows or not isinstance(rows, list):
        errors.append(f"{where}: no rows")
        return
    w = len(rows[0])
    for i, r in enumerate(rows):
        if len(r) != w:
            errors.append(f"{where} row {i}: length {len(r)}, expected {w}")
        bad = set(r) - PAL
        if bad:
            errors.append(f"{where} row {i}: characters outside the palette {sorted(bad)}")
    if want and (w, len(rows)) != tuple(want):
        errors.append(f"{where}: is {w}x{len(rows)}, expected {want[0]}x{want[1]}")


for path in sorted(glob.glob("batches_*.json")):
    for label, batch in json.load(open(path)).items():
        sizes = batch.get("sizes", {})
        for name, rows in batch.get("textures", {}).items():
            if name in seen:
                errors.append(f"{name}: in both {seen[name]} and {label}")
            seen[name] = label
            check(f"{label} {name}", rows, sizes.get(name, (8, 8)))
        face_sizes = batch.get("face_sizes", {})
        for model, faces in batch.get("models", {}).items():
            if model in seen:
                errors.append(f"{model}: in both {seen[model]} and {label}")
            seen[model] = label
            want = face_sizes.get(model)
            if want:
                missing, extra = set(want) - set(faces), set(faces) - set(want)
                if missing:
                    errors.append(f"{label} {model}: missing faces {sorted(missing)}")
                if extra:
                    errors.append(f"{label} {model}: unexpected faces {sorted(extra)}")
            for face, rows in faces.items():
                check(f"{label} {model}.{face}", rows, (want or {}).get(face))

items_path = sys.argv[1] if len(sys.argv) > 1 else "../data/items.toml"
with open(items_path, "rb") as f:
    items = tomllib.load(f)
for kind in ("tool", "item"):
    for entry in items.get(kind, []):
        if entry["name"] not in seen:
            errors.append(f"{items_path}: {kind} {entry['name']!r} has no texture in any batches_*.json")

if errors:
    print("\n".join(errors))
    sys.exit(1)
print(f"ok: {len(seen)} textures and models")
