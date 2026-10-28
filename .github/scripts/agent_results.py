#!/usr/bin/env python3
"""Result schemas for agent roles, and a stdlib-only validator.

Each role has two shapes: `done` (the work finished) and `stopped` (the run
needs the maintainer: a `needs_maintainer` object). A result holding
`needs_maintainer` is checked against `stopped`, anything else against `done`.
`validate(role, text)` returns a list of readable errors, empty when valid.
The schemas use a small JSON-Schema subset: type, required, properties, items,
enum, minItems, minimum, pattern, additionalProperties.
(docs/design/factory-orchestrator.md Phase C, rule 4)

    python3 -I agent_results.py <role> < result.json   # exit 1 and errors on stderr
    python3 -I agent_results.py verify <role> <arg> <result.json>
        role implementer (arg: ticket), docs_updater (arg: plan issue),
        reconciler or spec_editor (arg: the issue it answered). Writes ok= and expected= to $GITHUB_OUTPUT (or stdout), as `expect.py ticket`
    python3 -I agent_results.py follow-ups <ticket> <result.json>
        files each follow_ups entry once, as a `planned` ticket, via planner_issue.py
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))

KINDS = ["decision", "research", "needs-you"]

STR = {"type": "string", "minLength": 1}
NEEDS_MAINTAINER = {
    "type": "object",
    "required": ["kind", "question", "options"],
    "properties": {
        "kind": {"type": "string", "enum": KINDS},
        "question": STR,
        "options": {"type": "array", "items": STR},
    },
    "additionalProperties": False,
}
STOPPED = {
    "type": "object",
    "required": ["needs_maintainer"],
    "properties": {"needs_maintainer": NEEDS_MAINTAINER},
    "additionalProperties": False,
}
ISSUE_REF = {"type": "integer", "minimum": 1}
# A role that opens a pull request reports it, or says it opened none and why.
OPENED_PR = {
    "type": "object",
    "required": ["pr"],
    "properties": {"pr": ISSUE_REF},
    "additionalProperties": False,
}
NO_PR = {
    "type": "object",
    "required": ["no_pr"],
    "properties": {"no_pr": STR},
    "additionalProperties": False,
}
# Verdicts review_gate.py accepts; blockers and nits are numbered in the review.
VERDICTS = ["blocked", "changes needed", "ready with nits", "ready to merge", "ready"]
NUMBERED = {"type": "array", "items": {
    "type": "object",
    "required": ["number", "text"],
    "properties": {"number": ISSUE_REF, "text": STR},
    "additionalProperties": False}}

SCHEMAS = {
    "implementer": {
        "done": {
            "type": "object",
            "required": ["pr", "follow_ups", "questions", "blocked_on"],
            "properties": {
                "pr": ISSUE_REF,
                "follow_ups": {"type": "array", "items": {
                    "type": "object",
                    "required": ["title", "body"],
                    "properties": {"title": STR, "body": STR},
                    "additionalProperties": False}},
                "questions": {"type": "array", "items": NEEDS_MAINTAINER},
                "blocked_on": {"type": "array", "items": ISSUE_REF},
            },
            "additionalProperties": False,
        },
        "stopped": STOPPED,
    },
    "fix": {
        "done": {
            "type": "object",
            "required": ["pushed", "commit"],
            "properties": {"pushed": {"type": "boolean", "enum": [True]}, "commit": STR},
            "additionalProperties": False,
        },
        "stopped": STOPPED,
    },
    "reviewer": {
        "done": {
            "type": "object",
            "required": ["verdict", "reviewed_commit", "blockers", "nits"],
            "properties": {
                "verdict": {"type": "string", "enum": VERDICTS},
                "reviewed_commit": {"type": "string", "pattern": "^[0-9a-f]{7,40}$"},
                "blockers": NUMBERED,
                "nits": NUMBERED,
            },
            "additionalProperties": False,
        },
        "stopped": STOPPED,
    },
    "planner": {
        "done": {
            "type": "object",
            "required": ["plan", "tickets", "questions"],
            "properties": {
                "plan": ISSUE_REF,
                "tickets": {"type": "array", "items": {
                    "type": "object",
                    "required": ["title", "body", "files"],
                    "properties": {
                        "title": STR,
                        "body": STR,
                        # Paths the ticket edits: the source for file leases.
                        "files": {"type": "array", "minItems": 1, "items": STR},
                    },
                    "additionalProperties": False}},
                "questions": {"type": "array", "items": {
                    "type": "object",
                    "required": ["kind", "title", "body"],
                    "properties": {
                        "kind": {"type": "string", "enum": ["decision", "research"]},
                        "title": STR,
                        "body": STR,
                    },
                    "additionalProperties": False}},
            },
            "additionalProperties": False,
        },
        "stopped": STOPPED,
    },
}
for _role in ("docs_updater", "reconciler", "spec_editor"):
    SCHEMAS[_role] = {"done": OPENED_PR, "no_pr": NO_PR, "stopped": STOPPED}


def _type_ok(want, value):
    if want == "object":
        return isinstance(value, dict)
    if want == "array":
        return isinstance(value, list)
    if want == "string":
        return isinstance(value, str)
    if want == "boolean":
        return isinstance(value, bool)
    if want == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    return False


def check(schema, value, path, errors):
    want = schema["type"]
    if not _type_ok(want, value):
        errors.append(f"{path}: expected {want}, got {type(value).__name__}")
        return
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{path}: {value!r} is not one of {schema['enum']}")
    if want == "string" and len(value) < schema.get("minLength", 0):
        errors.append(f"{path}: must not be empty")
    if want == "integer" and value < schema.get("minimum", value):
        errors.append(f"{path}: must be at least {schema['minimum']}")
    if want == "string" and "pattern" in schema and not re.fullmatch(schema["pattern"], value):
        errors.append(f"{path}: {value!r} does not match {schema['pattern']}")
    if want == "array":
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{path}: needs at least {schema['minItems']} item(s)")
        for i, item in enumerate(value):
            check(schema["items"], item, f"{path}[{i}]", errors)
    if want == "object":
        props = schema.get("properties", {})
        for name in schema.get("required", []):
            if name not in value:
                errors.append(f"{path}.{name}: missing")
        for name, item in value.items():
            if name in props:
                check(props[name], item, f"{path}.{name}", errors)
            elif schema.get("additionalProperties", True) is False:
                errors.append(f"{path}.{name}: unexpected field")


def check_options(nm, path, errors):
    """A decision is answered by choosing, so it needs options to choose from."""
    if nm.get("kind") == "decision" and nm.get("options") == []:
        errors.append(f"{path}.options: a decision needs at least one option")


def validate(role, text):
    """Errors for a result (JSON text or an already parsed value); [] if valid."""
    if role not in SCHEMAS:
        return [f"unknown role {role!r}; known: {', '.join(sorted(SCHEMAS))}"]
    if isinstance(text, (str, bytes)):
        try:
            value = json.loads(text)
        except ValueError as e:
            return [f"result: not valid JSON ({e})"]
    else:
        value = text
    errors = []
    stopped = isinstance(value, dict) and "needs_maintainer" in value
    shape = "stopped" if stopped else "done"
    if isinstance(value, dict) and "no_pr" in value and "no_pr" in SCHEMAS[role]:
        shape = "no_pr"
    check(SCHEMAS[role][shape], value, "result", errors)
    if not errors:
        if stopped:
            check_options(value["needs_maintainer"], "result.needs_maintainer", errors)
        else:
            for i, q in enumerate(value.get("questions", [])):
                check_options(q, f"result.questions[{i}]", errors)
    return errors


def gh(*args):
    p = subprocess.run(["gh", *args], capture_output=True, text=True)
    return p.returncode, p.stdout


def verify(ticket, text, run=gh):
    """(ok, expected) for an implementer result: it names an open PR for the
    ticket, or it stops with a question for the maintainer."""
    expected = f"a pull request for #{ticket}, or a comment saying why it stopped"
    if validate("implementer", text):
        return False, expected
    value = json.loads(text)
    if "needs_maintainer" in value or value["questions"]:
        return True, expected
    code, out = run("pr", "view", str(value["pr"]), "--json", "state,headRefName,body")
    if code != 0:
        return False, expected
    pr = json.loads(out)
    closes = re.search(rf"(?i)\b(close[sd]?|fix(e[sd])?|resolve[sd]?) #{ticket}\b", pr.get("body") or "")
    mine = pr["headRefName"].startswith(f"claude/issue-{ticket}-") or closes
    return pr["state"] == "OPEN" and bool(mine), expected


# Branch each PR-opening role works on: a pattern for the head ref, given arg.
# The docs updater adds a suffix (-v2, -r2) when an old branch has the name.
OPENER_BRANCH = {
    "docs_updater": lambda arg: rf"docs/.*phase-done-{re.escape(arg)}(-[a-z0-9]+)?",
    "reconciler": lambda arg: r"design/.*",
    "spec_editor": lambda arg: r"spec/.*",
}


def verify_opener(role, arg, text, run=gh):
    """(ok, expected) for a docs updater, reconciler or spec editor result: it
    names a pull request on the role's branch, says why it opened none, or
    stops with a question for the maintainer."""
    expected = {
        "docs_updater": f"a pull request marking plan #{arg}'s phase done, or why none",
        "reconciler": f"a design-doc pull request for #{arg}, or why none",
        "spec_editor": f"a spec pull request for #{arg}, or why none",
    }.get(role, "")
    if role not in OPENER_BRANCH or validate(role, text):
        return False, expected
    value = json.loads(text)
    if "needs_maintainer" in value or "no_pr" in value:
        return True, expected
    code, out = run("pr", "view", str(value["pr"]), "--json", "headRefName")
    if code != 0:
        return False, expected
    ref = json.loads(out)["headRefName"]
    return bool(re.fullmatch(OPENER_BRANCH[role](arg), ref)), expected


def marker(ticket, title):
    return f"<!-- follow-up-of #{ticket} {hashlib.sha1(title.encode()).hexdigest()[:12]} -->"


def file_follow_ups(ticket, text, run=gh, create=None):
    """Create each follow-up of a valid implementer result once. The marker in
    its body is how a re-run finds it. Returns the titles created."""
    if validate("implementer", text):
        return []
    items = json.loads(text).get("follow_ups", [])
    code, out = run("issue", "list", "--state", "all", "--label", "planned",
                    "--limit", "500", "--json", "body")
    if code != 0:
        raise RuntimeError("could not list existing tickets")
    seen = "\n".join(i["body"] or "" for i in json.loads(out))
    made = []
    for item in items:
        m = marker(ticket, item["title"])
        if m in seen:
            continue
        body = f"{item['body'].rstrip()}\n\n{m}\n"
        (create or create_ticket)(item["title"], body)
        made.append(item["title"])
    return made


def create_ticket(title, body):
    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as f:
        f.write(body)
    try:
        subprocess.run([sys.executable, "-I", os.path.join(HERE, "planner_issue.py"),
                        "ticket", "--title", title, "--body-file", f.name], check=True)
    finally:
        os.unlink(f.name)


def main(argv):
    if len(argv) == 5 and argv[1] == "verify":
        text = open(argv[4], encoding="utf-8").read()
        if argv[2] == "implementer":
            ok, expected = verify(argv[3], text)
        else:
            ok, expected = verify_opener(argv[2], argv[3], text)
        lines = [f"ok={'true' if ok else 'false'}", f"expected={expected}"]
        print(("Found " if ok else "Missing: ") + expected, file=sys.stderr)
        out = os.environ.get("GITHUB_OUTPUT")
        if out:
            with open(out, "a", encoding="utf-8") as f:
                f.write("\n".join(lines) + "\n")
        else:
            print("\n".join(lines))
        return 0
    if len(argv) == 4 and argv[1] == "follow-ups":
        for title in file_follow_ups(argv[2], open(argv[3], encoding="utf-8").read()):
            print(f"created follow-up: {title}", file=sys.stderr)
        return 0
    if len(argv) != 2:
        print("usage: agent_results.py <role> < result.json", file=sys.stderr)
        return 2
    errors = validate(argv[1], sys.stdin.read())
    for e in errors:
        print(e, file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
