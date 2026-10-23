"""Tests for the needs-you pass. python3 -I test_factory_needs_you.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_needs_you import LABEL, QUEUE_TITLE, needs_you  # noqa: E402
from factory_tick import REPORT_TITLE, FakeBackend, Snapshot, tick  # noqa: E402

SHA = "a" * 40
OK = [{"id": 1, "name": n, "conclusion": "success", "run_attempt": 1, "created_at": ""}
      for n in ("CI", "PR size", "Review gate")]


def ticket(n, labels=("planned", "factory"), **kw):
    return {"number": n, "title": f"t{n}", "state": "open", "labels": list(labels),
            "kind": "ticket", "body": "", **kw}


def pr(n, t, kind="ready", runs=OK, labels=(), **kw):
    return {"number": n, "head": f"claude/issue-{t}-20261022-0000", "sha": SHA, "body": "",
            "labels": list(labels), "draft": False, "runs": list(runs), "files": ["a.rs"],
            "mergeable_state": "clean",
            "verdict": {"kind": kind, "current": True} if kind else None, **kw}


def snap(issues, prs, codeowners=("/.github/",)):
    s = Snapshot(issues=issues, prs=prs)
    s.codeowners, s.master_ci = list(codeowners), "success"
    return s


def labelled(s):
    return ({f"issue#{i['number']}" for i in s.issues if LABEL in i["labels"]}
            | {f"pr#{p['number']}" for p in s.prs if LABEL in p["labels"]})


def run(s):
    return tick(s, [needs_you], FakeBackend())


class Test(unittest.TestCase):
    def test_blocked_pr_and_its_ticket_are_flagged(self):
        s = snap([ticket(7)], [pr(9, 7, "blocked")])
        run(s)
        self.assertEqual(labelled(s), {"pr#9", "issue#7"})
        queue = next(i for i in s.issues if i["title"] == QUEUE_TITLE)
        self.assertIn("#9: Reviewer blocked it", queue["body"])
        self.assertIn("#7: PR #9: Reviewer blocked it", queue["body"])

    def test_blocked_pr_waits_while_claude_can_fix_it(self):
        from factory_fix import marker
        p = pr(9, 7, "blocked", blockers=["a test still passes with the fix undone"])
        s = snap([ticket(7)], [p])
        self.assertEqual(run(s), [])  # the Blocked-fix pass has it
        p["comments"] = [{"user": "github-actions[bot]", "body": marker(9, SHA, "review")}]
        self.assertEqual(run(s), [])  # a fix is running
        p["fix_finished"] = [SHA]  # it ended and pushed nothing
        run(s)
        self.assertEqual(labelled(s), {"pr#9", "issue#7"})

    def test_blocked_pr_flagged_once_the_budget_is_spent(self):
        from factory_blocked import RETRY_BUDGET
        from factory_fix import marker
        heads = [f"{n:x}" * 40 for n in range(1, RETRY_BUDGET + 1)]
        p = pr(9, 7, "blocked", blockers=["x"])
        p["sha"] = heads[-1][:40]
        p["comments"] = [{"user": "github-actions[bot]", "body": marker(9, h[:40], "review")}
                         for h in heads[:-1]] + [
            {"user": "github-actions[bot]", "body": marker(9, p["sha"], "review")}]
        p["fix_finished"] = []
        s = snap([ticket(7)], [p])
        s.prs[0]["verdict"]["current"] = True
        run(s)
        self.assertIn("pr#9", labelled(s))

    def test_second_tick_does_nothing(self):
        s = snap([ticket(7)], [pr(9, 7, "blocked")])
        run(s)
        self.assertEqual(run(s), [])

    def test_auto_mergeable_and_running_prs_are_not_flagged(self):
        s = snap([ticket(7), ticket(8)], [pr(9, 7, "ready"), pr(10, 8, None)])
        self.assertEqual(run(s), [])

    def test_ready_but_codeowners_path(self):
        p = pr(9, 7, "ready", files=[".github/x.py"], mergeable_state="blocked")
        s = snap([ticket(7)], [p])
        run(s)
        self.assertIn("pr#9", labelled(s))

    def test_nits_flagged_until_the_nits_pass_can_run(self):
        s = snap([ticket(7)], [pr(9, 7, "nits")])
        run(s)
        self.assertIn("pr#9", labelled(s))
        s = snap([ticket(7)], [pr(9, 7, None)])
        s.prs[0]["verdict"] = {"kind": "nits", "current": True, "commit": SHA[:7], "nits": ["x"]}
        self.assertEqual(run(s), [])  # the nits pass owns it

    def test_check_failed_after_rerun(self):
        runs = [{**OK[0], "conclusion": "failure", "run_attempt": 2}]
        s = snap([ticket(7)], [pr(9, 7, None, runs=runs)])
        self.assertEqual(run(s), [])  # failed twice: Claude's fix run goes first
        s.prs[0]["fix_finished"] = [SHA]
        s.prs[0]["comments"] = [{"user": "github-actions[bot]",
                                 "body": f"<!-- factory-fix pr=9 sha={SHA} cause=check -->"}]
        run(s)  # the fix pushed nothing: now it's yours
        self.assertIn("pr#9", labelled(s))
        s = snap([ticket(7)], [pr(9, 7, None, runs=[{**OK[0], "conclusion": "failure"}])])
        self.assertEqual(run(s), [])  # first failure: the rerun pass has it

    def test_non_ticket_pr_waits_for_merge(self):
        s = snap([], [{"number": 5, "head": "docs/x", "sha": SHA, "body": "", "labels": [],
                       "draft": False}])
        run(s)
        self.assertIn("pr#5", labelled(s))

    def test_parked_items_are_left_alone(self):
        s = snap([ticket(7)], [pr(9, 7, "blocked", labels=["hold"]),
                               {**pr(10, 7, "blocked"), "draft": True}])
        self.assertEqual(run(s), [])

    def test_questions_unapproved_tickets_and_one_report(self):
        q = ticket(3, labels=["spec-question", "decision"], kind="decision")
        inbox = ticket(4, labels=["planned"])
        r1 = {**ticket(20, labels=[]), "title": REPORT_TITLE, "kind": "report"}
        r2 = {**r1, "number": 21, "labels": []}
        s = snap([q, inbox, r1, r2], [])
        run(s)
        self.assertEqual(labelled(s), {"issue#3", "issue#4", "issue#20"})

    def test_label_removed_once_it_can_move(self):
        s = snap([ticket(7, labels=["planned", "factory", LABEL])],
                 [pr(9, 7, "ready", labels=[LABEL])])
        s.issues.append({**ticket(30, labels=[]), "title": QUEUE_TITLE, "body": "old"})
        acts = run(s)
        self.assertEqual(labelled(s), set())
        self.assertIn("close_issue", [a.kind for a in acts])

    def test_most_blocking_first(self):
        a, b = ticket(7), ticket(8)
        dep = ticket(11, depends_on=[8])
        s = snap([a, b, dep], [pr(9, 7, "blocked"), pr(10, 8, "blocked")])
        run(s)
        body = next(i for i in s.issues if i["title"] == QUEUE_TITLE)["body"]
        self.assertLess(body.index("#10:"), body.index("#9:"))
        self.assertIn("(blocks 1 ticket)", body)


if __name__ == "__main__":
    unittest.main()
