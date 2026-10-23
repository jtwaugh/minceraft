"""Tests for the stalled-tickets pass. python3 -I test_factory_stalled_tickets.py (no network)."""
import os
import sys
import unittest
from datetime import datetime, timedelta, timezone
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_config import RETRY_BUDGET  # noqa: E402
from factory_ready import MARKER  # noqa: E402
from factory_stalled_tickets import key_of, stalled_tickets, state_of  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

NOW = datetime(2026, 10, 23, 20, 0, tzinfo=timezone.utc)


def at(minutes_ago):
    return (NOW - timedelta(minutes=minutes_ago)).strftime("%Y-%m-%dT%H:%M:%SZ")


def ticket(starts=(60,), labels=("factory", "planned")):
    return {"number": 777, "title": "Play critic B", "state": "open", "labels": list(labels),
            "kind": "ticket", "body": "", "depends_on": [],
            "comments": [{"user": "github-actions[bot]", "body": f"{MARKER} start",
                          "created_at": at(m)} for m in starts]}


def snap(t, runs=(), stray=None, prs=()):
    s = Snapshot(issues=[t], prs=list(prs))
    s.implementer_runs, s.stray = list(runs), dict(stray or {})
    return s


def run(s):
    with mock.patch("factory_stalled_tickets.datetime") as d:
        d.now.return_value, d.fromisoformat = NOW, datetime.fromisoformat
        return tick(s, [stalled_tickets], FakeBackend())


class Test(unittest.TestCase):
    def test_branch_with_work_gets_its_pr_once(self):
        s = snap(ticket(), stray={777: ["claude/issue-777-20261022-0240"]})
        acts = run(s)
        self.assertEqual([a.kind for a in acts], ["open_pr"])
        self.assertIn("Closes #777", acts[0].payload["body"])
        self.assertEqual(run(s), [])  # the PR exists now

    def test_no_pr_no_branch_is_restarted_then_asks(self):
        s = snap(ticket(starts=[90]), runs=[{"display_title": "Claude implementer #777",
                                             "status": "completed", "created_at": at(89)}])
        self.assertEqual([a.kind for a in run(s)], ["start_ticket"])
        s = snap(ticket(starts=[90 - 5 * i for i in range(RETRY_BUDGET)]),
                 runs=[{"display_title": "Play critic B", "status": "completed", "created_at": at(1)}])
        acts = run(s)
        self.assertEqual([a.kind for a in acts], ["open_question"])
        self.assertEqual(acts[0].payload["key"], key_of(777))
        self.assertEqual(run(s), [])  # asked once

    def test_waits_while_a_run_is_going_or_starting(self):
        busy = [{"display_title": "Play critic B", "status": "in_progress", "created_at": at(5)}]
        self.assertEqual(run(snap(ticket(), runs=busy)), [])
        self.assertEqual(run(snap(ticket(starts=[3]))), [])  # dispatched 3 min ago
        self.assertEqual([a.kind for a in run(snap(ticket(starts=[30])))], ["start_ticket"])  # never ran

    def test_left_alone(self):
        self.assertIsNone(state_of(snap(ticket(starts=[])), ticket(starts=[]), NOW))  # never started
        t = ticket(labels=("factory", "local"))
        self.assertIsNone(state_of(snap(t), t, NOW))
        t = ticket()
        pr = {"number": 960, "head": "claude/issue-777-20261022-0240", "body": "", "files": []}
        self.assertIsNone(state_of(snap(t, prs=[pr]), t, NOW))


if __name__ == "__main__":
    unittest.main()
