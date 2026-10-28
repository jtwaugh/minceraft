"""Tests for the nits pass. python3 -I test_factory_nits.py (no network)."""
import json
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
        self.assertEqual([m for m in markers(s.prs[0]["comments"])], [(9, H1, "nits")])

    def test_answered_question_is_not_reopened(self):
        s = snap()
        run(s)
        s.prs[0]["sha"] = H2
        s.prs[0]["verdict"]["commit"] = H2[:7]
        run(s)
        for i in s.issues:
            if "needs-you" in i["labels"]:
                i["state"] = "closed"
        self.assertEqual(run(s), [])
        self.assertEqual(len([i for i in s.issues if "needs-you" in i["labels"]]), 1)

    def test_moved_head_does_not_ask_again(self):
        s = snap()
        run(s)
        for sha in (H2, "c" * 40):
            s.prs[0]["sha"] = sha
            s.prs[0]["verdict"]["commit"] = sha[:7]
            run(s)
        self.assertEqual(len([i for i in s.issues if "needs-you" in i["labels"]]), 1)

    def test_ready_after_fix_is_quiet(self):
        s = snap(sha=H2, kind="ready", comments=[
            {"user": "github-actions[bot]", "body": marker(9, H1, "nits")}])
        self.assertEqual(run(s), [])

    def test_stale_verdict_waits(self):
        self.assertEqual(run(snap(sha=H2, commit=H1[:7])), [])

    def test_not_a_ticket_pr(self):
        self.assertEqual(run(snap(head="spec/issue-7-x")), [])
        s = snap()
        s.issues[0]["labels"] = []
        self.assertEqual(run(s), [])

    def test_registered_after_rerun(self):
        names = [p.__name__ for p in passes()]
        self.assertEqual(names.index("nits"), names.index("rerun_failed") + 1)

    def test_verdict_from_the_real_loader(self):
        # The pass must accept what factory_automerge.verdict_of actually
        # returns, not only the hand-built verdicts above. A KeyError here
        # stopped every tick from 2026-10-22 19:31Z.
        from factory_automerge import verdict_of
        comments = [{"user": {"login": "claude[bot]"}, "updated_at": "2026-10-22T00:00:00Z",
                     "body": review("ready with nits")["body"]}]
        s = snap()
        s.prs[0]["verdict"] = verdict_of(comments, H1)
        run(s)  # must not raise


def review(word, tail="", sha=H1, user="claude[bot]", at="2026-01-01T00:00:00Z"):
    value = json.dumps({"verdict": word, "reviewed_commit": sha[:7], "blockers": [], "nits": []})
    return {"user": {"login": user}, "updated_at": at, "created_at": at,
            "body": f"{tail}\n<!-- factory-result role=reviewer -->\n```json\n{value}\n```"}


class Loading(unittest.TestCase):
    def test_parse_nits(self):
        body = ("**Blockers**\n1. not this\n\n**Nits**\n- Rename `x`\n  in a.py\n"
                "2. Stale comment\n\n**Notes**\n- not this either\n")
        self.assertEqual(parse_nits(body), ["Rename `x` in a.py", "Stale comment"])
        self.assertEqual(parse_nits("### Nits\n* one\n* two"), ["one", "two"])
        self.assertEqual(parse_nits("**Notes**\n- a"), [])
        self.assertEqual(
            parse_nits("**Nits**\n- Rename the helper so it\n  reads better everywhere\n- second\n"),
            ["Rename the helper so it reads better everywhere", "second"])
        self.assertEqual(parse_nits("**Nits**\n- one\n\nNone of these block.\n"), ["one"])

    def test_verdict_kinds(self):
        nit = review("ready with nits", "**Nits**\n- fix a\n")
        v = verdict([nit], H1)
        self.assertEqual((v["kind"], v["commit"], v["nits"]), ("nits", H1[:7], ["fix a"]))
        self.assertEqual(verdict([review("ready")], H1)["kind"], "ready")
        self.assertEqual(verdict([review("blocked")], H1)["kind"], "blocked")
        self.assertIsNone(verdict([], H1))
        self.assertIsNone(verdict([review("ready", user="mallory")], H1))
        self.assertFalse(verdict([nit], H2)["current"])


if __name__ == "__main__":
    unittest.main()
