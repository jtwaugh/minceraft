"""Tests for the agent result schemas. python3 -I test_agent_results.py (no network)."""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from agent_results import validate  # noqa: E402

NM = {"kind": "decision", "question": "Which?", "options": ["a", "b"]}
IMPL = {"pr": 5, "follow_ups": [{"title": "t", "body": "b"}], "questions": [NM], "blocked_on": [3]}
FIX = {"pushed": True, "commit": "abc1234"}


def bad(role, value, field):
    errs = validate(role, json.dumps(value))
    assert errs, f"expected errors for {value}"
    assert any(field in e for e in errs), (field, errs)


class Implementer(unittest.TestCase):
    def test_valid(self):
        self.assertEqual(validate("implementer", json.dumps(IMPL)), [])

    def test_empty_lists(self):
        self.assertEqual(validate("implementer", json.dumps(
            {"pr": 1, "follow_ups": [], "questions": [], "blocked_on": []})), [])

    def test_stopped(self):
        self.assertEqual(validate("implementer", {"needs_maintainer": NM}), [])

    def test_missing_field(self):
        for name in IMPL:
            bad("implementer", {k: v for k, v in IMPL.items() if k != name}, f"result.{name}")

    def test_wrong_types(self):
        bad("implementer", {**IMPL, "pr": "5"}, "result.pr")
        bad("implementer", {**IMPL, "pr": True}, "result.pr")
        bad("implementer", {**IMPL, "blocked_on": [0]}, "result.blocked_on[0]")
        bad("implementer", {**IMPL, "follow_ups": [{"title": "t"}]}, "result.follow_ups[0].body")

    def test_unexpected_field(self):
        bad("implementer", {**IMPL, "extra": 1}, "result.extra")

    def test_question_checks(self):
        bad("implementer", {**IMPL, "questions": [{**NM, "kind": "idea"}]},
            "result.questions[0].kind")
        bad("implementer", {**IMPL, "questions": [{**NM, "options": []}]},
            "result.questions[0].options")


class Fix(unittest.TestCase):
    def test_pushed(self):
        self.assertEqual(validate("fix", json.dumps(FIX)), [])

    def test_stopped_each_kind(self):
        for kind in ("decision", "research", "needs-you"):
            self.assertEqual(validate("fix", {"needs_maintainer": {**NM, "kind": kind}}), [])

    def test_options_optional_unless_decision(self):
        for kind in ("research", "needs-you"):
            self.assertEqual(validate(
                "fix", {"needs_maintainer": {**NM, "kind": kind, "options": []}}), [])

    def test_wrong_kind(self):
        bad("fix", {"needs_maintainer": {**NM, "kind": "maybe"}}, "result.needs_maintainer.kind")

    def test_decision_without_options(self):
        bad("fix", {"needs_maintainer": {**NM, "options": []}}, "result.needs_maintainer.options")

    def test_missing_fields(self):
        bad("fix", {"pushed": True}, "result.commit")
        bad("fix", {"needs_maintainer": {"kind": "decision"}}, "result.needs_maintainer.question")
        bad("fix", {"needs_maintainer": {"kind": "decision", "question": "q"}},
            "result.needs_maintainer.options")
        bad("fix", {}, "result.pushed")

    def test_not_pushed(self):
        bad("fix", {"pushed": False, "commit": "abc"}, "result.pushed")

    def test_empty_question(self):
        bad("fix", {"needs_maintainer": {**NM, "question": ""}}, "result.needs_maintainer.question")


REV = {"verdict": "ready with nits", "reviewed_commit": "abc1234",
       "blockers": [], "nits": [{"number": 1, "text": "rename"}]}
PLAN = {"plan": 9, "tickets": [{"title": "t", "body": "b", "files": ["a.rs"]}],
        "questions": [{"kind": "research", "title": "q", "body": "b"}]}


class Reviewer(unittest.TestCase):
    def test_valid(self):
        self.assertEqual(validate("reviewer", REV), [])
        for v in ("blocked", "changes needed", "ready", "ready to merge"):
            self.assertEqual(validate("reviewer", {**REV, "verdict": v}), [])
        full = "a" * 40
        self.assertEqual(validate("reviewer", {**REV, "reviewed_commit": full}), [])
        blocker = {"number": 1, "text": "bug"}
        self.assertEqual(validate("reviewer", {**REV, "blockers": [blocker]}), [])

    def test_invalid(self):
        bad("reviewer", {**REV, "verdict": "lgtm"}, "result.verdict")
        for sha in ("abc12", "ABC1234", "g" * 8, "a" * 41, "", "abc1234\n"):
            bad("reviewer", {**REV, "reviewed_commit": sha}, "result.reviewed_commit")
        bad("reviewer", {**REV, "blockers": [{"text": "x"}]}, "result.blockers[0].number")
        bad("reviewer", {**REV, "nits": [{"number": 0, "text": "x"}]}, "result.nits[0].number")
        bad("reviewer", {k: v for k, v in REV.items() if k != "nits"}, "result.nits")

    def test_stopped(self):
        self.assertEqual(validate("reviewer", {"needs_maintainer": NM}), [])


class Planner(unittest.TestCase):
    def test_valid(self):
        self.assertEqual(validate("planner", PLAN), [])
        self.assertEqual(validate("planner", {**PLAN, "tickets": [], "questions": []}), [])

    def test_invalid(self):
        t = PLAN["tickets"][0]
        bad("planner", {**PLAN, "tickets": [{**t, "files": []}]}, "result.tickets[0].files")
        bad("planner", {**PLAN, "tickets": [{"title": "t", "body": "b"}]},
            "result.tickets[0].files")
        bad("planner", {**PLAN, "questions": [{"kind": "idea", "title": "q", "body": "b"}]},
            "result.questions[0].kind")
        bad("planner", {k: v for k, v in PLAN.items() if k != "plan"}, "result.plan")


class PrOpeners(unittest.TestCase):
    ROLES = ("docs_updater", "reconciler", "spec_editor")

    def test_valid(self):
        for role in self.ROLES:
            self.assertEqual(validate(role, {"pr": 12}), [])
            self.assertEqual(validate(role, {"no_pr": "nothing to change"}), [])
            self.assertEqual(validate(role, {"needs_maintainer": NM}), [])

    def test_invalid(self):
        for role in self.ROLES:
            bad(role, {}, "result.pr")
            bad(role, {"pr": "12"}, "result.pr")
            bad(role, {"no_pr": ""}, "result.no_pr")
            bad(role, {"pr": 12, "no_pr": "why"}, "result.pr")


class Input(unittest.TestCase):
    def test_not_json(self):
        for text in ("", "not json", "{'pr': 1}", b"{"):
            errs = validate("fix", text)
            self.assertEqual(len(errs), 1)
            self.assertIn("not valid JSON", errs[0])

    def test_not_an_object(self):
        for text in ("[]", "null", "3", '"x"'):
            bad("implementer", json.loads(text), "result")

    def test_unknown_role(self):
        self.assertIn("unknown role", validate("nobody", "{}")[0])


if __name__ == "__main__":
    unittest.main()
