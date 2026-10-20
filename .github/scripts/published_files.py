"""Check that the files the deploy publishes cover everything the pages load.

The published set is what .vercelignore leaves in (an ignore-all rule followed
by `!` re-includes). Starting from the root, viewer and client index.html,
follow static imports, <script src>, new URL(..., import.meta.url) and
relative asset paths (.js/.wasm/.json) and fail naming any referenced file
that is not published (or not on disk).

Run after ./build.sh: python3 -I .github/scripts/published_files.py
"""
import fnmatch
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ENTRIES = ["index.html", "viewer/index.html", "client/index.html"]
FOLLOW = (".js", ".mjs", ".html")
SKIP_DIRS = {".git", "target", "node_modules", "work"}

IMPORT_FROM = re.compile(r"""\b(?:import|export)\b[^'"`;]*?\bfrom\s*['"]([^'"]+)['"]""")
IMPORT_BARE = re.compile(r"""\bimport\s*['"]([^'"]+)['"]""")
IMPORT_CALL = re.compile(r"""\bimport\(\s*['"]([^'"]+)['"]""")
SCRIPT_SRC = re.compile(r"""<script\b[^>]*\bsrc\s*=\s*['"]([^'"]+)['"]""", re.I)
NEW_URL = re.compile(r"""new\s+URL\(\s*['"]([^'"]+)['"]\s*,\s*import\.meta\.url""")
ASSET = re.compile(r"""['"](\.{1,2}/[^'"\s]+\.(?:js|mjs|wasm|json))['"](?!\s*:)""")
ABSOLUTE = re.compile(r"^[a-z][a-z0-9+.-]*:|^/", re.I)


def parse_ignore(text):
    rules = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        neg = line.startswith("!")
        pat = line[1:] if neg else line
        rules.append((neg, pat.strip("/")))
    return rules


def is_published(path, rules):
    """Last matching rule wins; a rule matching a parent directory covers it."""
    parts = path.split("/")
    state = True
    for neg, pat in rules:
        for i in range(1, len(parts) + 1):
            if fnmatch.fnmatchcase("/".join(parts[:i]), pat):
                state = neg
                break
    return state


def references(text):
    """Relative paths, as written, that a source file loads."""
    found = set()
    for rx in (IMPORT_FROM, IMPORT_BARE, IMPORT_CALL, SCRIPT_SRC, NEW_URL, ASSET):
        found.update(rx.findall(text))
    return sorted(r for r in found if not ABSOLUTE.match(r))


def resolve(src, ref):
    ref = ref.split("?")[0].split("#")[0]
    return os.path.normpath(os.path.join(os.path.dirname(src), ref)).replace(os.sep, "/")


def crawl(read, entries):
    """Return {file: [referrers]}. read(path) -> text, or None if absent."""
    seen = {e: [] for e in entries}
    todo = list(entries)
    while todo:
        cur = todo.pop()
        text = read(cur)
        if text is None or not cur.endswith(FOLLOW):
            continue
        for ref in references(text):
            target = resolve(cur, ref)
            if target not in seen:
                seen[target] = []
                todo.append(target)
            seen[target].append(cur)
    return seen


def check(read, published, entries=ENTRIES):
    """Return error lines for referenced files that are unpublished or absent."""
    errors = []
    for f, by in sorted(crawl(read, entries).items()):
        who = ", ".join(sorted(set(by))) or "entry point"
        if f.startswith(".."):
            errors.append(f"{f}: outside the repo (from {who})")
        elif not published(f):
            errors.append(f"{f}: not in the published set (from {who})")
        elif read(f) is None:
            errors.append(f"{f}: not found on disk, run ./build.sh (from {who})")
    return errors


def main():
    with open(os.path.join(ROOT, ".vercelignore")) as f:
        rules = parse_ignore(f.read())

    def read(path):
        try:
            with open(os.path.join(ROOT, path), encoding="utf-8", errors="replace") as f:
                return f.read()
        except OSError:
            return None

    files = []
    for d, dirs, names in os.walk(ROOT):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
        for n in names:
            files.append(os.path.relpath(os.path.join(d, n), ROOT).replace(os.sep, "/"))
    pub = sorted(f for f in files if is_published(f, rules))
    print("published files:")
    for f in pub:
        print("  " + f)
    print(f"{len(pub)} files")

    errors = check(read, lambda p: is_published(p, rules))
    for e in errors:
        print("ERROR " + e, file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
