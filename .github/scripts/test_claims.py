"""Tests for tickets.py claimed and conflicts.py still. python3 -I test_claims.py (no network)."""
import os
import sys
import unittest

os.environ["GITHUB_REPOSITORY"] = "o/r"
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import conflicts  # noqa: E402
import tickets  # noqa: E402


class Claimed(unittest.TestCase):
    def setUp(self):
        tickets.fetch.__defaults__[0].clear()
        self.prs, self.labels, self.refs, self.branch_prs, self.ahead = [], [], [], {}, {}

        def gh(path):
            if path.startswith("repos/o/r/pulls?state=open"):
                return self.prs
            if path == "repos/o/r/issues/900":
                return {"number": 900, "labels": [{"name": x} for x in self.labels]}
            if path.startswith("repos/o/r/git/matching-refs/heads/claude/issue-900-"):
                return [{"ref": f"refs/heads/{b}"} for b in self.refs]
            if path.startswith("repos/o/r/pulls?state=all&head=o:"):
                return self.branch_prs.get(path.rsplit(":", 1)[1], [])
            if path.startswith("repos/o/r/compare/master..."):
                return {"ahead_by": self.ahead.get(path.rsplit("...", 1)[1], 0)}
            raise AssertionError(path)
        self._gh, tickets.gh = tickets.gh, gh

    def tearDown(self):
        tickets.gh = self._gh

    def test_free(self):
        self.assertIsNone(tickets.claimed(900))

    def test_open_pr(self):
        self.prs = [{"number": 903, "head": {"ref": "claude/issue-900-sea-ice"}, "body": ""}]
        self.assertIn("open pull request", tickets.claimed(900))

    def test_local_label(self):
        self.labels = ["factory", "local"]
        self.assertIn("`local`", tickets.claimed(900))

    def test_pushed_branch_without_pr(self):
        """The 33 seconds between the local session's push and its PR."""
        self.refs, self.ahead = ["claude/issue-900-sea-ice"], {"claude/issue-900-sea-ice": 1}
        self.assertIn("`claude/issue-900-sea-ice`", tickets.claimed(900))
        self.assertIsNone(tickets.claimed(900, mine="claude/issue-900-sea-ice"))

    def test_branches_that_dont_count(self):
        self.refs = ["claude/issue-900-old", "claude/issue-900-empty"]
        self.branch_prs = {"claude/issue-900-old": [{"number": 1}]}  # its PR was closed or merged
        self.ahead = {"claude/issue-900-old": 3}  # -empty: nothing master lacks
        self.assertIsNone(tickets.claimed(900))

    def test_ticket_numbers(self):
        self.assertEqual(tickets.ticket_numbers("claude/issue-900-sea-ice", "Closes #901"), {900})
        self.assertEqual(tickets.ticket_numbers("claude/x", "Closes #900. Fixes #2"), {900, 2})
        self.assertEqual(tickets.ticket_numbers("claude/x", "see #900"), set())


class Still(unittest.TestCase):
    """#903 got three identical stop comments, one per queued resolve job."""

    def setUp(self):
        self.pr = {"state": "open", "draft": False, "head": {"sha": "ec8aeb7", "ref": "claude/issue-900-x"},
                   "body": "", "labels": []}
        self.ticket, self.comments = "open", []

        def gh(path):
            if path == "repos/o/r/pulls/903":
                return self.pr
            if path == "repos/o/r/issues/900":
                return {"state": self.ticket}
            if path == "repos/o/r/commits/ec8aeb7":
                return {"commit": {"committer": {"date": "2026-10-22T02:39:52Z"}}}
            if path.startswith("repos/o/r/issues/903/comments"):
                return self.comments
            raise AssertionError(path)
        self._gh, conflicts.gh = conflicts.gh, gh

    def tearDown(self):
        conflicts.gh = self._gh

    def still(self, only=""):
        return conflicts.still(903, "ec8aeb7", only)

    def test_first_run_goes(self):
        self.assertTrue(self.still())

    def test_queued_run_after_a_stop_skips(self):
        self.comments = [{"body": "<!-- conflict-stop -->\\nboth sides", "created_at": "2026-10-22T03:11:25Z"}]
        self.assertFalse(self.still())
        self.assertTrue(self.still(only="903"))  # a hand run with pr=903 retries anyway

    def test_moved_draft_closed_or_superseded_skips(self):
        for change in ({"head": {"sha": "fffffff", "ref": "claude/issue-900-x"}}, {"draft": True},
                       {"state": "closed"}, {"labels": [{"name": "superseded"}]}):
            with self.subTest(change):
                saved = dict(self.pr)
                self.pr.update(change)
                self.assertFalse(self.still())
                self.pr = saved
        self.ticket = "closed"
        self.assertFalse(self.still())


if __name__ == "__main__":
    unittest.main()
