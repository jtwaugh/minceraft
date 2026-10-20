"""Tests for validate.py's item coverage check and the textures.json export.

Run from art/: python3 -I tools/test_art.py
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest

TOOLS = os.path.dirname(os.path.abspath(__file__))
ART = os.path.dirname(TOOLS)


def run(script, *args):
    return subprocess.run(
        [sys.executable, "-I", os.path.join(TOOLS, script), *args],
        cwd=ART, capture_output=True, text=True,
    )


class ItemCoverage(unittest.TestCase):
    def test_current_data_passes(self):
        r = run("validate.py")
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_missing_texture_fails_with_name(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "items.toml")
            with open(path, "w") as f:
                f.write('[[item]]\nname = "no_such_icon_xyz"\n')
            r = run("validate.py", path)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("no_such_icon_xyz", r.stdout)


class Export(unittest.TestCase):
    def test_item_and_ui_tiles_exported_at_drawn_sizes(self):
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "textures.json")
            r = run("export_textures.py", out)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            with open(out) as f:
                batches = json.load(f)["batches"]
        textures, sizes = {}, {}
        for b in batches.values():
            textures.update(b.get("textures", {}))
            sizes.update(b.get("sizes", {}))
        want = {"heart_full": (8, 8), "heart_half": (8, 8), "slot": (10, 10), "slot_selected": (10, 10)}
        for n in ("tl", "t", "tr", "l", "c", "r", "bl", "b", "br"):
            want[f"panel_{n}"] = (4, 4)
        for n in range(10):
            want[f"digit_{n}"] = (4, 6)
        for n in ("sword", "hoe", "seeds", "raw_beef", "leather", "raw_pork", "raw_chicken", "feather", "wheat"):
            want[n] = (8, 8)
        for name, (w, h) in want.items():
            self.assertIn(name, textures)
            self.assertEqual(tuple(sizes.get(name, (8, 8))), (w, h), name)
            rows = textures[name]
            self.assertEqual((len(rows[0]), len(rows)), (w, h), name)


if __name__ == "__main__":
    unittest.main()
