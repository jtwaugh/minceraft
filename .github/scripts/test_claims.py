"""Tests for tickets.py claimed. python3 -I test_claims.py (no network)."""
import os
import sys
import unittest

os.environ["GITHUB_REPOSITORY"] = "o/r"
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
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


class Deps(unittest.TestCase):
    def test_header_only(self):
        body = ("**Depends on:** none\n\nThe Depends on: line said #12 and #1257.\n"
                "- [ ] Depends on: #3 is read from the header\n")
        self.assertEqual(tickets.deps(body), set())

    def test_header_and_blocked_by(self):
        body = "**Depends on:** #5, #6\n\nprose\nBlocked by #7\n"
        self.assertEqual(tickets.deps(body), {5, 6, 7})


class PredictedFiles(unittest.TestCase):
    def test_dot_directories_keep_their_dot(self):
        body = "`.github/scripts/tickets.py`, `./client/scene.js`"
        self.assertEqual(tickets.predicted_files(body),
                         {".github/scripts/tickets.py", "client/scene.js"})

    def test_overlap_finds_github_path(self):
        preds = tickets.predicted_files("`.github/scripts/tickets.py`")
        self.assertEqual(tickets.overlap(preds, [".github/scripts/tickets.py", "README.md"]),
                         [".github/scripts/tickets.py"])


if __name__ == "__main__":
    unittest.main()
