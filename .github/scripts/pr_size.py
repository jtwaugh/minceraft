#!/usr/bin/env python3
"""Count the lines of a change that a reviewer has to read.

    pr_size.py BASE HEAD [--limit N]

Diffs BASE...HEAD (the merge base, as a pull request does) and counts
added plus deleted lines, leaving out:

- tests: files under a tests/ or test/ folder, Rust files named tests.rs,
  *_tests.rs or tests_*.rs, files named *_test.* or *.test.*, Python files
  named test_*.py, and Rust #[cfg(test)] modules inside source files;
- prose: Markdown and .txt files, and anything under docs/;
- generated files: lockfiles, images, wasm, and the art batch JSON.

Tests are left out so the limit never argues against writing them.
Prints a per-file table, writes it to $GITHUB_STEP_SUMMARY when set, and
exits 1 when the count is over the limit (default 200). Agents run it
before opening a PR: `python3 .github/scripts/pr_size.py origin/master HEAD`.
"""
import argparse
import os
import re
import subprocess
import sys

LIMIT = 200

TEST_PATH = re.compile(
    r"(^|/)tests?/|(^|/)(tests?|tests?_[^/]*|[^/]*_tests?)\.rs$|_test\.[a-z]+$|\.test\.[a-z]+$"
    r"|(^|/)test_[^/]*\.py$"
)
PROSE = re.compile(r"\.(md|txt)$|^docs/|^LICENSE")
GENERATED = re.compile(
    r"(^|/)(Cargo\.lock|package-lock\.json)$|\.(png|jpg|gif|wasm)$|^art/batches_[0-9_]+\.json$"
)
CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]")
HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def test_module_start(rev, path):
    """First line (1-based) of a #[cfg(test)] item in a Rust file at REV, or None."""
    try:
        text = git("show", f"{rev}:{path}")
    except subprocess.CalledProcessError:
        return None  # the file doesn't exist at REV
    for i, line in enumerate(text.splitlines(), 1):
        if CFG_TEST.match(line):
            return i
    return None


def classify(path):
    if GENERATED.search(path):
        return "generated"
    if PROSE.search(path):
        return "prose"
    if TEST_PATH.search(path):
        return "tests"
    return "code"


def count(base, head):
    """Return [(path, counted, tests, skipped_kind)] for every changed file."""
    rows = []
    names = git("diff", "--no-renames", "--name-only", f"{base}...{head}").split()
    merge_base = git("merge-base", base, head).strip()
    for path in names:
        kind = classify(path)
        diff = git("diff", "--no-renames", "-U0", f"{base}...{head}", "--", path)
        changed = sum(
            1 for line in diff.splitlines()
            if line[:1] in "+-" and not line.startswith(("+++", "---"))
        )
        if kind != "code":
            rows.append((path, 0, changed if kind == "tests" else 0, kind))
            continue
        if not path.endswith(".rs"):
            rows.append((path, changed, 0, ""))
            continue
        # Lines at or below the #[cfg(test)] marker are tests, on each side.
        old_cut = test_module_start(merge_base, path)
        new_cut = test_module_start(head, path)
        code = tests = 0
        old_line = new_line = 0
        for line in diff.splitlines():
            m = HUNK.match(line)
            if m:
                old_line, new_line = int(m.group(1)), int(m.group(3))
                continue
            if line.startswith(("+++", "---")):
                continue
            if line.startswith("-"):
                is_test = old_cut is not None and old_line >= old_cut
                old_line += 1
            elif line.startswith("+"):
                is_test = new_cut is not None and new_line >= new_cut
                new_line += 1
            else:
                continue
            if is_test:
                tests += 1
            else:
                code += 1
        rows.append((path, code, tests, ""))
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    ap.add_argument("head")
    ap.add_argument("--limit", type=int, default=LIMIT)
    args = ap.parse_args()

    rows = count(args.base, args.head)
    total = sum(r[1] for r in rows)
    tests = sum(r[2] for r in rows)
    over = total > args.limit

    lines = [
        f"### PR size: {total} reviewable lines (limit {args.limit})",
        "",
        f"Tests ({tests} lines), prose and generated files are not counted.",
        "",
        "| File | Counted | Tests | Not counted |",
        "|---|--:|--:|---|",
    ]
    for path, code, t, kind in sorted(rows, key=lambda r: -r[1]):
        lines.append(f"| `{path}` | {code} | {t} | {kind} |")
    if over:
        lines += [
            "",
            f"Over the limit by {total - args.limit}. Split the change into smaller PRs, "
            "or, if it can't sensibly be split, the maintainer can add the `oversize-ok` label.",
        ]
    report = "\n".join(lines)
    print(report)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(report + "\n")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as f:
            f.write(f"lines={total}\nover={'true' if over else 'false'}\n")
    sys.exit(1 if over else 0)


if __name__ == "__main__":
    main()
