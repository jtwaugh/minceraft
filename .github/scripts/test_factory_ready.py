"""Tests for the ready-ticket pass. python3 -I test_factory_ready.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_ready import ready  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def issue(n, body="", state="open", labels=("factory",), kind="ticket", **kw):
    return {"number": n, "title": f"T{n}", "state": state, "labels": list(labels),
            "kind": kind, "body": body, **kw}


def snap(issues, prs=()):
    return Snapshot.from_dict({"issues": issues, "prs": list(prs)})


def started(s):
    return [a.target for a in ready(s)]


class Test(unittest.TestCase):
    def test_ready(self):
        s = snap([issue(1, "**Depends on:** #2"), issue(2, state="closed", labels=())])
        self.assertEqual(started(s), ["issue#1"])

    def test_blocked_by_open_dependency(self):
        s = snap([issue(1, "**Depends on:** #2"), issue(2, labels=())])
        self.assertEqual(started(s), [])

    def test_blocked_by_comment_from_trusted_only(self):
        c = lambda u: [{"user": u, "body": "Blocked by #2", "created_at": "2026-01-01"}]
        two = issue(2, labels=())
        self.assertEqual(started(snap([issue(1, comments=c("jtwaugh")), two])), [])
        self.assertEqual(started(snap([issue(1, comments=c("stranger")), two])), ["issue#1"])

    def test_blocked_by_open_pr_files(self):
        pr = {"number": 9, "head": "other", "files": ["crates/game/src/walk.rs"]}
        s = snap([issue(1, "**Files:** `walk.rs`, `lib.rs`"), issue(3, "`lib.rs`")], [pr])
        self.assertEqual(started(s), ["issue#3"])

    def test_has_open_pr(self):
        pr = {"number": 9, "head": "claude/issue-1-2026", "files": []}
        self.assertEqual(started(snap([issue(1)], [pr])), [])

    def test_local_label(self):
        self.assertEqual(started(snap([issue(1, labels=("factory", "local"))])), [])

    def test_batch_file_overlap(self):
        s = snap([issue(1, "`a.rs`"), issue(2, "`a.rs`"), issue(3, "`b.rs`")])
        self.assertEqual(started(s), ["issue#1", "issue#3"])

    def test_plan_dependency_never_started(self):
        s = snap([issue(5, kind="plan", labels=()), issue(6, "**Depends on:** #5")])
        self.assertEqual(started(s), [])

    def test_unreadable_depends_never_started(self):
        # #719: every dependency closed, but the line named none of them
        s = snap([issue(1, "**Depends on:** the swimming tickets in this plan"),
                  issue(2, "**Depends on:** #3, and the steering ticket"),
                  issue(3, state="closed", labels=())])
        self.assertEqual(started(s), [])

    def test_fixture_519(self):
        s = Snapshot.from_fixture(os.path.join(HERE, "fixtures", "history.json"))
        self.assertNotIn("issue#519", started(s))

    def test_already_started_until_new_blocker(self):
        mark = {"user": "github-actions[bot]", "body": "<!-- factory-auto-start -->",
                "created_at": "2026-01-02"}
        s = snap([issue(1, comments=[mark])])
        self.assertEqual(started(s), [])
        s.issues[0]["comments"].append(
            {"user": "claude[bot]", "body": "Blocked by #2", "created_at": "2026-01-03"})
        s.issues.append(issue(2, state="closed", labels=()))
        self.assertEqual(started(s), ["issue#1"])

    def test_second_tick_starts_nothing(self):
        s = snap([issue(1), issue(2, "**Depends on:** #1")])
        self.assertEqual(len(tick(s, [ready], FakeBackend())), 1)
        self.assertEqual(tick(s, [ready], FakeBackend()), [])


if __name__ == "__main__":
    unittest.main()
