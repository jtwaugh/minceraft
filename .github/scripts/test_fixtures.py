"""Tests that the factory fixtures parse and cover the stall history; no build needed. python3 -I test_fixtures.py"""
import json
import os
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, "fixtures", "history.json")
README = os.path.join(HERE, "fixtures", "README.md")
NAMED = [452, 500, 470, 507, 521, 513, 519]


def load():
    with open(PATH) as f:
        return json.load(f)


class Test(unittest.TestCase):
    def test_named_issues_present(self):
        have = {i["number"] for i in load()["issues"]}
        for n in NAMED:
            self.assertIn(n, have)

    def test_each_issue_shape(self):
        for i in load()["issues"]:
            for k in ("number", "title", "state", "labels", "parent", "depends_on"):
                self.assertIn(k, i, i["number"])

    def test_orphans_and_cycle(self):
        by = {i["number"]: i for i in load()["issues"]}
        for n in (452, 500, 521):
            self.assertIsNone(by[n]["parent"])
            self.assertIn(by[n]["expected_parent"], by)
        self.assertIn(513, by[519]["depends_on"])
        self.assertEqual(by[513]["kind"], "plan")

    def test_docs_and_branches(self):
        d = load()
        doc = {x["file"]: x for x in d["docs"]}
        self.assertEqual(doc["docs/design/raft-scene-feedback.md"]["phases"], {})
        self.assertEqual(doc["docs/design/raft-scene-feedback.md"]["status"], "accepted")
        self.assertTrue(d["branches"])
        for b in d["branches"]:
            self.assertRegex(b, r"^docs/.+-phase-done-\d+")

    def test_pr_files(self):
        for p in load()["prs"]:
            self.assertTrue(p["head"] and p["files"])

    def test_readme_lists_cases(self):
        text = open(README).read()
        for token in ["#452", "#500", "#470", "#507", "#521", "#513", "#519",
                      "raft-scene-feedback.md", "phase-done"]:
            self.assertIn(token, text)


if __name__ == "__main__":
    unittest.main()
