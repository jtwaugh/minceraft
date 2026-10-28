"""Tests for the auto-merge pass. python3 -I test_factory_automerge.py (no network)."""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_automerge import auto_merge, codeowner_patterns, owned, verdict_of, why_not  # noqa: E402
from factory_backend import GhBackend  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

CODEOWNERS = """# comment
/SPEC.md                @jtwaugh
/.github/               @jtwaugh
/art/PROMPT.md          @jtwaugh
/docs/design/           @jtwaugh
*.jar                   @jtwaugh
"""
HEAD = "abcdef1234567890"


def pr(**over):
    p = {"number": 9, "head": "claude/issue-7-20260101-0000", "sha": HEAD, "body": "",
         "labels": [], "draft": False, "files": ["crates/game/src/walk.rs"],
         "verdict": {"kind": "ready", "current": True}, "mergeable_state": "clean"}
    p.update(over)
    return p


def snap(*prs, master_ci="success"):
    return Snapshot(
        issues=[{"number": 7, "title": "t", "state": "open", "labels": ["factory"], "body": ""}],
        prs=list(prs), codeowners=codeowner_patterns(CODEOWNERS), master_ci=master_ci)


class Pass(unittest.TestCase):
    def test_ready_and_clean_is_merged_once(self):
        s = snap(pr())
        a = tick(s, [auto_merge], FakeBackend())
        self.assertEqual([(x.kind, x.target, x.payload) for x in a], [("merge_pr", "pr#9", {"sha": HEAD})])
        self.assertEqual(tick(s, [auto_merge], FakeBackend()), [])

    def test_everything_else_waits(self):
        cases = {
            "verdict is nits": pr(verdict={"kind": "nits", "current": True}),
            "verdict is blocked": pr(verdict={"kind": "blocked", "current": True}),
            "no verdict on the head commit": pr(verdict={"kind": "ready", "current": False}),
            "not clean (blocked)": pr(mergeable_state="blocked"),
            "not clean (unknown)": pr(mergeable_state=None),
            "held": pr(labels=["hold"]),
            "draft": pr(draft=True),
            "touches a CODEOWNERS path": pr(files=["crates/a.rs", ".github/scripts/x.py"]),
        }
        for reason, p in cases.items():
            with self.subTest(reason):
                self.assertEqual(why_not(snap(p), p), reason)
                self.assertEqual(auto_merge(snap(p)), [])
        p = pr(verdict=None)
        self.assertEqual(why_not(snap(p), p), "no verdict on the head commit")

    def test_red_master_or_unknown_owners_stop_it(self):
        self.assertEqual(auto_merge(snap(pr(), master_ci="failure")), [])
        s = snap(pr())
        s.codeowners = None
        self.assertEqual(auto_merge(s), [])

    def test_two_prs_for_one_ticket_both_wait(self):
        """#903 and #905 both built #900 and both got Ready: merge neither."""
        twin = pr(number=10, head="claude/issue-7-sea-ice")
        self.assertEqual(auto_merge(snap(pr(), twin)), [])
        local = pr(number=10, head="jtwaugh/sea-ice", body="Closes #7")
        self.assertEqual(why_not(snap(pr(), local), pr()), "another open PR is for the same ticket (#10)")
        self.assertEqual([a.target for a in auto_merge(snap(pr(), pr(number=10, head="claude/issue-8-x")))],
                         ["pr#9"])

    def test_only_ticket_prs(self):
        self.assertEqual(auto_merge(snap(pr(head="jtwaugh/some-fix"))), [])
        self.assertEqual(auto_merge(snap(pr(head="claude/issue-8-20260101-0000"))), [])  # not a factory ticket


class Owners(unittest.TestCase):
    def test_this_repos_patterns(self):
        pats = codeowner_patterns(CODEOWNERS)
        for path, want in [("SPEC.md", True), ("crates/SPEC.md", False), (".github/workflows/ci.yml", True),
                           ("docs/design/x.md", True), ("docs/state-machines.md", False),
                           ("art/PROMPT.md", True), ("art/batches_1_6.json", False),
                           ("lib/x.jar", True), ("crates/game/src/walk.rs", False)]:
            with self.subTest(path):
                self.assertEqual(owned(path, pats), want)


class Verdict(unittest.TestCase):
    def c(self, body, at="2026-10-21T10:00:00Z", user="claude[bot]"):
        return {"user": {"login": user}, "body": body, "updated_at": at}

    def test_kinds_and_head(self):
        ok = f"Verdict: Ready\nReviewed commit: `{HEAD[:7]}`"
        self.assertEqual(verdict_of([self.c(ok)], HEAD), {"kind": "ready", "current": True})
        self.assertEqual(verdict_of([self.c(ok.replace("Ready", "Ready, with nits"))], HEAD)["kind"], "nits")
        self.assertEqual(verdict_of([self.c(ok.replace("Ready", "Blocked"))], HEAD)["kind"], "blocked")
        self.assertFalse(verdict_of([self.c(ok)], "0000000fff")["current"])
        self.assertIsNone(verdict_of([self.c("Verdict: Ready", user="mallory")], HEAD))

    def test_workflow_comment_needs_structured_result(self):
        lines = f"Verdict: Ready, with nits\nReviewed commit: `{HEAD[:7]}`\n"
        result = json.dumps({"verdict": "ready with nits", "reviewed_commit": HEAD[:7],
                             "blockers": [], "nits": [{"number": 1, "text": "x"}]})
        block = f"<!-- factory-result role=reviewer pr=1 sha={HEAD} -->\n```json\n{result}\n```"
        bot = "github-actions[bot]"
        self.assertEqual(verdict_of([self.c(lines + block, user=bot)], HEAD)["kind"], "nits")
        self.assertIsNone(verdict_of([self.c(lines, user=bot)], HEAD))

    def test_newest_wins(self):
        old = self.c(f"Verdict: Ready\nReviewed commit: `{HEAD[:7]}`", "2026-10-21T10:00:00Z")
        new = self.c(f"Verdict: Blocked\nReviewed commit: `{HEAD[:7]}`", "2026-10-21T11:00:00Z")
        self.assertEqual(verdict_of([new, old], HEAD)["kind"], "blocked")


class Backend(unittest.TestCase):
    def test_merges_as_the_maintainer_on_the_reviewed_commit(self):
        calls = []

        def run(args, project=False):
            calls.append((args, project))
            return ""
        s = snap(pr())
        b = GhBackend("o/r", run)
        tick(s, [auto_merge], b)
        self.assertEqual(calls, [(["pr", "merge", "9", "--repo", "o/r", "--merge",
                                   "--match-head-commit", HEAD], True)])
        self.assertTrue(s.prs[0]["merged"])
        tick(s, [auto_merge], b)
        self.assertEqual(len(calls), 1)

    def test_a_refused_merge_is_reported_and_the_tick_goes_on(self):
        def run(args, project=False):
            raise RuntimeError("gh pr merge 9: Head branch was modified")
        s = snap(pr())
        b = GhBackend("o/r", run)
        tick(s, [auto_merge], b)
        self.assertEqual([(a.target, e) for a, e in b.failed], [("pr#9", "gh pr merge 9: Head branch was modified")])
        self.assertFalse(s.prs[0].get("merged", False))


if __name__ == "__main__":
    unittest.main()
