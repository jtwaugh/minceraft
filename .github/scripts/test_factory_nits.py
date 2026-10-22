"""Tests for the nits pass. python3 -I test_factory_nits.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_fix import marker, markers, parse_nits, verdict  # noqa: E402
from factory_nits import nits  # noqa: E402
from factory_tick import FakeBackend, Snapshot, passes, tick  # noqa: E402

NITS = ["Rename `x` in a.py", "Doc comment is stale"]
H1, H2 = "a" * 40, "b" * 40


def snap(sha=H1, kind="nits", commit=None, comments=(), head="claude/issue-7-20260101-0000"):
    return Snapshot(
        issues=[{"number": 7, "title": "t", "state": "open", "labels": ["factory"], "body": ""}],
        prs=[{"number": 9, "head": head, "sha": sha, "body": "",
              "verdict": {"kind": kind, "commit": commit or sha[:7], "nits": NITS},
              "comments": list(comments)}])


def run(s):
    return tick(s, [nits], FakeBackend())


class Test(unittest.TestCase):
    def test_one_fix_per_head(self):
        s = snap()
        a = run(s)
        self.assertEqual([(x.kind, x.target) for x in a], [("start_fix", "pr#9")])
        self.assertIn("1. Rename `x` in a.py", a[0].payload["task"])
        self.assertIn("2. Doc comment is stale", a[0].payload["task"])
        self.assertEqual(run(s), [])

    def test_still_nits_asks_once(self):
        s = snap()
        run(s)
        s.prs[0]["sha"] = H2  # the fix push, reviewed again with nits
        s.prs[0]["verdict"]["commit"] = H2[:7]
        for _ in range(3):
            a = run(s)
            self.assertEqual([(x.kind, x.target) for x in a][:1], [("open_question", "issue#7")] * len(a))
        qs = [i for i in s.issues if "needs-you" in i["labels"]]
        self.assertEqual(len(qs), 1)
        for nit in NITS:
            self.assertIn(nit, qs[0]["body"])
        self.assertIn("follow-up", qs[0]["body"])
        self.assertEqual([m for m in markers(s.prs[0]["comments"])], [("nits", H1)])

    def test_ready_after_fix_is_quiet(self):
        s = snap(sha=H2, kind="ready", comments=[
            {"user": "github-actions[bot]", "body": marker("nits", H1)}])
        self.assertEqual(run(s), [])

    def test_stale_verdict_waits(self):
        self.assertEqual(run(snap(sha=H2, commit=H1[:7])), [])

    def test_not_a_ticket_pr(self):
        self.assertEqual(run(snap(head="spec/issue-7-x")), [])
        s = snap()
        s.issues[0]["labels"] = []
        self.assertEqual(run(s), [])

    def test_parse(self):
        body = "Verdict: Ready, with nits\nReviewed commit: `abc1234`\n\n**Nits**\n1. one\n   wrapped\n- two\n"
        self.assertEqual(parse_nits(body), ["one wrapped", "two"])
        v = verdict([{"user": {"login": "claude[bot]"}, "body": body, "updated_at": "1"}])
        self.assertEqual(v, {"kind": "nits", "commit": "abc1234", "nits": ["one wrapped", "two"]})

    def test_registered_after_rerun(self):
        names = [p.__name__ for p in passes()]
        self.assertEqual(names.index("nits"), names.index("rerun_failed") + 1)


if __name__ == "__main__":
    unittest.main()
