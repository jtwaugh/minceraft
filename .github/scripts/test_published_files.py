"""Tests for published_files.py; no build needed. python3 -I test_published_files.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import published_files as pf

IGNORE = "/*\n!/index.html\n!/viewer\n!/client\n"
SRC = {
    "index.html": '<script type="module" src="./client/menu.js"></script>',
    "viewer/index.html": "<script type=\"module\">\nimport init from './pkg/terrain_web.js';\n"
    "import { a, b } from './mesh.js';\nconst WASM_URL = './pkg/terrain_web_bg.wasm';\n"
    "fetch('./textures.json');\nnew Worker(new URL('./worker.js', import.meta.url));</script>",
    "viewer/worker.js": "import { x } from './flyover/plan.js';",
    "viewer/mesh.js": "export const m = 1;",
    "viewer/flyover/plan.js": "export const p = 1;",
    "viewer/pkg/terrain_web.js": "new URL('terrain_web_bg.wasm', import.meta.url);",
    "viewer/pkg/terrain_web_bg.wasm": "",
    "viewer/textures.json": "{}",
    "client/index.html": '<script type="module" src="./scene.js"></script>',
    "client/menu.js": "import { M } from './scenes.js';",
    "client/scenes.js": "",
    "client/scene.js": "import t from '../viewer/pkg/terrain_web.js';\n"
    "import g from './pkg/game_web.js';\nimport { q } from '../viewer/mesh.js';\n"
    "load('../viewer/textures.json'); import('./lazy.js'); // http://x.example/a.js",
    "client/lazy.js": "",
    "client/pkg/game_web.js": "",
}
ENTRIES = ["index.html", "viewer/index.html", "client/index.html"]


def pub(ignore):
    rules = pf.parse_ignore(ignore)
    return lambda p: pf.is_published(p, rules)


class Test(unittest.TestCase):
    def test_references(self):
        refs = pf.references(SRC["client/scene.js"])
        for want in ["../viewer/pkg/terrain_web.js", "./pkg/game_web.js", "../viewer/mesh.js",
                     "../viewer/textures.json", "./lazy.js"]:
            self.assertIn(want, refs)
        self.assertFalse([r for r in refs if "example" in r])
        refs = pf.references(SRC["viewer/index.html"])
        for want in ["./pkg/terrain_web.js", "./mesh.js", "./pkg/terrain_web_bg.wasm",
                     "./textures.json", "./worker.js"]:
            self.assertIn(want, refs)

    def test_crawl_finds_transitive(self):
        seen = pf.crawl(SRC.get, ENTRIES)
        for f in ["viewer/flyover/plan.js", "viewer/worker.js", "client/pkg/game_web.js",
                  "viewer/pkg/terrain_web_bg.wasm", "client/lazy.js", "client/scenes.js"]:
            self.assertIn(f, seen)

    def test_ok(self):
        self.assertEqual(pf.check(SRC.get, pub(IGNORE), ENTRIES), [])

    def test_removed_dirs_fail(self):
        errs = "\n".join(pf.check(SRC.get, pub("/*\n!/index.html\n!/client\n"), ENTRIES))
        self.assertIn("viewer/mesh.js: not in the published set", errs)
        self.assertIn("viewer/pkg/terrain_web.js: not in the published set", errs)
        errs = "\n".join(pf.check(SRC.get, pub("/*\n!/index.html\n!/viewer\n"), ENTRIES))
        self.assertIn("client/pkg/game_web.js: not in the published set", errs)

    def test_missing_on_disk(self):
        src = dict(SRC)
        del src["viewer/textures.json"]
        errs = pf.check(src.get, pub(IGNORE), ENTRIES)
        self.assertTrue(any(e.startswith("viewer/textures.json: not found") for e in errs))

    def test_ignore_rules(self):
        p = pub(IGNORE)
        self.assertTrue(p("viewer/pkg/a.js"))
        self.assertFalse(p("art/x.png"))
        self.assertFalse(p("CLAUDE.md"))


if __name__ == "__main__":
    unittest.main()
