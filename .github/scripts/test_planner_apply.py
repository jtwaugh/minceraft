"""Tests for planner_apply.py with a fake planner_issue.py. python3 -I test_planner_apply.py"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planner_apply  # noqa: E402
from agent_results import verify_planner  # noqa: E402

FAKE = '''
import json, os, sys
state = os.environ["FAKE_STATE"]
log = json.load(open(state)) if os.path.exists(state) else []
a = sys.argv[1:]
body = open(a[a.index("--body-file") + 1], encoding="utf-8").read()
log.append({"mode": a[0], "args": a[1:-2], "body": body})
json.dump(log, open(state, "w"))
print("https://github.com/o/r/issues/%d" % (100 + len(log)))
'''
T1 = {"title": "A", "body": "**Spec:** s\n**Depends on:** none\n**Files:** `a`\n", "files": ["a"],
      "depends_on": []}
T2 = {**T1, "title": "B", "body": "**Spec:** s\n**Depends on:** x\n**Files:** `b`\n",
      "files": ["b"], "depends_on": [1]}
Q = {"kind": "research", "title": "Q", "body": "b"}
PLAN = {"plan": {"title": "Plan: P", "file": "SPEC.md", "section": "3", "notes": "Why."},
        "tickets": [T1, T2], "questions": [Q]}


class Apply(unittest.TestCase):
    def setUp(self):
        d = tempfile.mkdtemp()
        self.script = os.path.join(d, "fake.py")
        with open(self.script, "w") as f:
            f.write(FAKE)
        os.environ["FAKE_STATE"] = os.path.join(d, "state.json")
        self.open_plans = []

    def run_apply(self, value):
        run = lambda *a: (0, json.dumps(self.open_plans))  # noqa: E731
        return planner_apply.apply(json.dumps(value), run, self.script)

    def log(self):
        p = os.environ["FAKE_STATE"]
        return json.load(open(p)) if os.path.exists(p) else []

    def test_creates_in_order_with_mapped_dependencies(self):
        self.assertEqual(self.run_apply(PLAN), 101)
        log = self.log()
        self.assertEqual([e["mode"] for e in log],
                         ["plan", "ticket", "ticket", "question", "edit-plan"])
        self.assertIn("**Depends on:** none", log[1]["body"])
        self.assertIn("**Depends on:** #102\n", log[2]["body"])
        self.assertEqual(log[3]["args"], ["--title", "Q", "--kind", "research"])
        self.assertEqual(log[4]["args"], ["101"])
        self.assertEqual(log[4]["body"].split("\n")[0],
                         "<!-- plan-source file=SPEC.md section=3 -->")
        body = log[4]["body"]
        self.assertLess(body.index("#102 A"), body.index("#103 B"))
        self.assertLess(body.index("#103 B"), body.index("#104 Q"))

    def test_open_plan_with_same_source_creates_nothing(self):
        self.open_plans = [{"title": "Plan: P",
                            "body": "<!-- plan-source file=SPEC.md section=3 -->\nx"}]
        self.assertIsNone(self.run_apply(PLAN))
        self.assertEqual(self.log(), [])

    def test_other_section_is_not_a_duplicate(self):
        self.open_plans = [{"title": "Plan: P",
                            "body": "<!-- plan-source file=SPEC.md section=4 -->"}]
        self.assertEqual(self.run_apply(PLAN), 101)

    def test_invalid_creates_nothing(self):
        with self.assertRaises(ValueError):
            self.run_apply({**PLAN, "tickets": [{**T1, "files": []}]})
        with self.assertRaises(ValueError):
            self.run_apply({**PLAN, "tickets": [{**T1, "depends_on": [1]}]})
        self.assertEqual(self.log(), [])

    def test_stopped_creates_nothing(self):
        nm = {"kind": "needs-you", "question": "?", "options": []}
        self.assertIsNone(self.run_apply({"needs_maintainer": nm}))
        self.assertEqual(self.log(), [])

    def test_no_tickets(self):
        self.run_apply({**PLAN, "tickets": [], "questions": []})
        self.assertIn("Nothing to build", self.log()[-1]["body"])


class Verify(unittest.TestCase):
    def test_plan_exists(self):
        issues = [{"title": "Plan: P", "body": "<!-- plan-source file=SPEC.md section=3 -->"}]
        run = lambda *a: (0, json.dumps(issues))  # noqa: E731
        text = json.dumps(PLAN)
        self.assertTrue(verify_planner("SPEC.md", text, run)[0])
        self.assertFalse(verify_planner("docs/x.md", text, run)[0])
        self.assertFalse(verify_planner("SPEC.md", "{}", run)[0])


if __name__ == "__main__":
    unittest.main()
