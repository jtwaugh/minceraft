"""Tests for the refresh-after-red-master pass. python3 -I test_factory_refresh.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_refresh import red_spells, refresh_after_red_master  # noqa: E402
from factory_rerun import rerun_failed  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

# Master's CI on Oct 22: green at #905, broken by the 03:09 batch, fixed by #911.
TONIGHT = [
    ("2026-10-22T03:48:48Z", "success"),
    ("2026-10-22T03:36:59Z", "failure"),
    ("2026-10-22T03:09:49Z", "failure"),
    ("2026-10-22T03:09:46Z", "cancelled"),
    ("2026-10-22T03:09:42Z", "cancelled"),
    ("2026-10-22T03:09:39Z", "cancelled"),
    ("2026-10-22T03:09:35Z", "success"),
    ("2026-10-22T00:10:35Z", "cancelled"),  # cancelled alone isn't red
    ("2026-10-22T00:08:21Z", "success"),
]
SPELL = ("2026-10-22T03:09:39Z", "2026-10-22T03:48:48Z")


def runs(rows):
    return [{"created_at": t, "conclusion": c} for t, c in rows]


def pr(n=878, ci_at="2026-10-22T03:41:00Z", conclusion="failure", **kw):
    p = {"number": n, "head": f"claude/issue-{n}-x", "sha": f"{n}abcdef", "body": "",
         "labels": [], "draft": False, "verdict": {"kind": "ready", "current": True},
         "runs": [{"id": 1, "name": "CI", "conclusion": conclusion, "run_attempt": 1, "created_at": ci_at},
                  {"id": 2, "name": "PR size", "conclusion": "success", "run_attempt": 1,
                   "created_at": ci_at}]}
    p.update(kw)
    return p


def snap(*prs, master=TONIGHT):
    rows = runs(master)
    return Snapshot(issues=[{"number": p["number"], "title": "t", "state": "open", "labels": ["factory"],
                             "body": ""} for p in prs],
                    prs=list(prs), master_runs=rows, master_ci=rows[0]["conclusion"] if rows else "")


class Spells(unittest.TestCase):
    def test_tonight(self):
        self.assertEqual(red_spells(runs(TONIGHT)), [SPELL])

    def test_still_red_is_open(self):
        self.assertEqual(red_spells(runs(TONIGHT[1:])), [(SPELL[0], None)])


class Pass(unittest.TestCase):
    def test_pr_that_failed_during_the_spell_is_updated_once(self):
        s = snap(pr())
        a = tick(s, [refresh_after_red_master, rerun_failed], FakeBackend())
        self.assertEqual([(x.kind, x.target, x.payload) for x in a],
                         [("update_branch", "pr#878", {"sha": "878abcdef"})])  # and no re-run
        self.assertEqual(tick(s, [refresh_after_red_master], FakeBackend()), [])

    def test_left_alone(self):
        cases = {
            "master still red": snap(pr(), master=TONIGHT[1:]),
            "failed after master was fixed": snap(pr(ci_at="2026-10-22T03:50:00Z")),
            "failed before master broke": snap(pr(ci_at="2026-10-22T03:00:00Z")),
            "CI passed": snap(pr(conclusion="success")),
            "a draft": snap(pr(draft=True)),
            "superseded": snap(pr(labels=["superseded"])),
            "not a ticket PR": Snapshot(prs=[pr()], master_runs=runs(TONIGHT), master_ci="success"),
        }
        for why, s in cases.items():
            with self.subTest(why):
                self.assertEqual(refresh_after_red_master(s), [])

    def test_only_ci_counts(self):
        p = pr()
        p["runs"][0]["name"] = "Claude reviewer"
        self.assertEqual(refresh_after_red_master(snap(p)), [])


class Backend(unittest.TestCase):
    def test_updates_as_the_maintainer_on_the_head_it_saw(self):
        calls = []

        def run(args, project=False):
            calls.append((args, project))
            return ""
        s = snap(pr())
        tick(s, [refresh_after_red_master], GhBackend("o/r", run))
        self.assertEqual(calls, [(["api", "-X", "PUT", "repos/o/r/pulls/878/update-branch",
                                   "-f", "expected_head_sha=878abcdef"], True)])
        tick(s, [refresh_after_red_master], GhBackend("o/r", run))
        self.assertEqual(len(calls), 1)

    def test_a_refused_update_is_recorded_and_the_tick_goes_on(self):
        def run(args, project=False):
            raise RuntimeError("gh api: merge conflict")
        s = snap(pr())
        b = GhBackend("o/r", run)
        tick(s, [refresh_after_red_master], b)
        self.assertEqual([a.target for a, _ in b.failed], ["pr#878"])
        self.assertEqual(s.prs[0]["sha"], "878abcdef")


if __name__ == "__main__":
    unittest.main()
