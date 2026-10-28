"""Tests for the agent result schemas. python3 -I test_agent_results.py (no network)."""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from agent_results import SCHEMAS, file_follow_ups, flag_schema, validate, verify, verify_opener  # noqa: E402

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
PLAN = {"plan": {"title": "Plan: x", "file": "SPEC.md", "section": "1", "notes": ""},
        "tickets": [{"title": "t", "body": "b", "files": ["a.rs"], "depends_on": []}],
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
        bad("planner", {**PLAN, "tickets": [{**t, "depends_on": [1]}]},
            "result.tickets[0].depends_on")


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


def fake_gh(pr=None, existing=()):
    def run(*args):
        if args[:2] == ("pr", "view"):
            return (0, json.dumps(pr)) if pr else (1, "")
        if args[:2] == ("issue", "list"):
            return 0, json.dumps([{"body": b} for b in existing])
        raise AssertionError(args)
    return run


OPEN_PR = {"state": "OPEN", "headRefName": "claude/issue-7-20261028-0000", "body": ""}


PR_ONLY = {**IMPL, "questions": []}


class Verify(unittest.TestCase):
    def ok(self, value, **kw):
        return verify("7", value if isinstance(value, str) else json.dumps(value), fake_gh(**kw))[0]

    def test_pr_result(self):
        self.assertTrue(self.ok(PR_ONLY, pr=OPEN_PR))
        self.assertTrue(self.ok(PR_ONLY, pr={**OPEN_PR, "headRefName": "x", "body": "Closes #7"}))

    def test_pr_not_for_ticket_or_not_open(self):
        self.assertFalse(self.ok(PR_ONLY, pr={**OPEN_PR, "headRefName": "x"}))
        self.assertFalse(self.ok(PR_ONLY, pr={**OPEN_PR, "state": "MERGED"}))
        self.assertFalse(self.ok(PR_ONLY))

    def test_stopped(self):
        self.assertTrue(self.ok({"needs_maintainer": NM}))
        self.assertTrue(self.ok({**IMPL, "pr": 99, "questions": [NM]}))

    def test_empty_or_invalid(self):
        for text in ("", "{}", "not json", '{"pr": "x"}'):
            self.assertFalse(self.ok(text, pr=OPEN_PR))


def head(ref):
    return {"state": "MERGED", "headRefName": ref, "body": ""}


class VerifyOpener(unittest.TestCase):
    def ok(self, role, arg, value, ref=None):
        run = fake_gh(pr=head(ref) if ref else None)
        return verify_opener(role, arg, json.dumps(value), run)[0]

    def test_docs_updater_branches(self):
        for ref in ("docs/x-phase-done-5", "docs/phase-done-5-v2", "docs/phase-done-5-r2"):
            self.assertTrue(self.ok("docs_updater", "5", {"pr": 9}, ref), ref)
        for ref in ("docs/phase-done-55", "docs/phase-done-6", "docs/phase-done-5-a.b", "feature/phase-done-5"):
            self.assertFalse(self.ok("docs_updater", "5", {"pr": 9}, ref), ref)

    def test_reconciler_and_spec_editor(self):
        self.assertTrue(self.ok("reconciler", "3", {"pr": 9}, "design/foo"))
        self.assertFalse(self.ok("reconciler", "3", {"pr": 9}, "spec/foo"))
        self.assertTrue(self.ok("spec_editor", "3", {"pr": 9}, "spec/q3"))
        self.assertFalse(self.ok("spec_editor", "3", {"pr": 9}, "design/foo"))

    def test_no_pr_or_stopped(self):
        for role in ("docs_updater", "reconciler", "spec_editor"):
            self.assertTrue(self.ok(role, "3", {"no_pr": "nothing to do"}))
            self.assertTrue(self.ok(role, "3", {"needs_maintainer": NM}))

    def test_missing_or_invalid(self):
        for role in ("docs_updater", "reconciler", "spec_editor"):
            self.assertFalse(self.ok(role, "3", {"pr": 9}))
            self.assertFalse(self.ok(role, "3", {}, "design/x"))
            self.assertFalse(self.ok(role, "3", {"pr": 9, "no_pr": "x"}, "design/x"))
        self.assertFalse(self.ok("reviewer", "3", {"pr": 9}, "design/x"))


class FollowUps(unittest.TestCase):
    def test_created_once(self):
        made, bodies = [], []

        def create(title, body):
            made.append(title)
            bodies.append(body)
        text = json.dumps({**IMPL, "follow_ups": [{"title": "a", "body": "B"}, {"title": "b", "body": "C"}]})
        self.assertEqual(file_follow_ups("7", text, fake_gh(), create), ["a", "b"])
        self.assertEqual(file_follow_ups("7", text, fake_gh(existing=bodies), create), [])
        self.assertEqual(made, ["a", "b"])

    def test_same_title_other_ticket(self):
        made = []
        text = json.dumps(IMPL)
        file_follow_ups("7", text, fake_gh(), lambda t, b: made.append(b))
        self.assertEqual(file_follow_ups("8", text, fake_gh(existing=made), lambda t, b: None), ["t"])

    def test_invalid_result_creates_nothing(self):
        self.assertEqual(file_follow_ups("7", "{}", fake_gh(), None), [])


class FlagSchema(unittest.TestCase):
    def test_every_shape_fits(self):
        """Each shape's fields are in the one flag schema, and it quotes safely."""
        for role, shapes in SCHEMAS.items():
            props = flag_schema(role)["properties"]
            for shape in shapes.values():
                for name, field in shape["properties"].items():
                    self.assertEqual(props[name], field, (role, name))
            self.assertNotIn("'", json.dumps(flag_schema(role)))


if __name__ == "__main__":
    unittest.main()
