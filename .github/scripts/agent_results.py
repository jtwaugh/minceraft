#!/usr/bin/env python3
"""Result schemas for agent roles, and a stdlib-only validator.

Each role has two shapes: `done` (the work finished) and `stopped` (the run
needs the maintainer: a `needs_maintainer` object). A result holding
`needs_maintainer` is checked against `stopped`, anything else against `done`.
`validate(role, text)` returns a list of readable errors, empty when valid.
The schemas use a small JSON-Schema subset: type, required, properties, items,
enum, minItems, minimum, additionalProperties.
(docs/design/factory-orchestrator.md Phase C, rule 4)

    python3 -I agent_results.py <role> < result.json   # exit 1 and errors on stderr
"""
import json
import sys

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
}


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
    check(SCHEMAS[role]["stopped" if stopped else "done"], value, "result", errors)
    if not errors:
        if stopped:
            check_options(value["needs_maintainer"], "result.needs_maintainer", errors)
        else:
            for i, q in enumerate(value.get("questions", [])):
                check_options(q, f"result.questions[{i}]", errors)
    return errors


def main(argv):
    if len(argv) != 2:
        print("usage: agent_results.py <role> < result.json", file=sys.stderr)
        return 2
    errors = validate(argv[1], sys.stdin.read())
    for e in errors:
        print(e, file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
