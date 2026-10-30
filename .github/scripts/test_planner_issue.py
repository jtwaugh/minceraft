"""Tests for the planner's ticket check. python3 -I test_planner_issue.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")
from planner_issue import ticket_errors  # noqa: E402
import tickets  # noqa: E402

ISSUES = {
    10: {"title": "Raft C: animals swim", "body": "**Files:** `crates/game/src/swim.rs` (new)"},
    11: {"title": "Plan: Raft C", "body": ""},
    12: {"title": "Some PR", "pull_request": {}, "body": ""},
    # a chain like #1496 -> #1497 in plan #1493: 21 creates a file, 22 depends on 21
    21: {"title": "Mesher F: shapes", "body": "**Depends on:** none\n**Files:** `crates/mesh/src/shapes.rs` (new)"},
    22: {"title": "Mesher G: plants", "body": "**Depends on:** #21\n**Files:** `crates/mesh/src/plants.rs` (new)"},
    # a cycle, which the walk must not loop on
    31: {"title": "A", "body": "**Depends on:** #32\n**Files:** `a.rs` (new)"},
    32: {"title": "B", "body": "**Depends on:** #31\n**Files:** `b.rs` (new)"},
}
for _n, _i in ISSUES.items():
    _i["number"] = _n
EXISTING = {"crates/game/src/animals.rs"}


def body(depends="none", files="`crates/game/src/animals.rs`"):
    return (f"**Spec:** docs/design/raft.md Phase C (Item 2)\n**Depends on:** {depends}\n"
            f"**Files:** {files}\n\n### What\nThings.\n\n### Done when\n- [ ] it works\n")


def errors(b):
    return ticket_errors(b, ISSUES.get, EXISTING.__contains__)


class DependsLine(unittest.TestCase):
    def test_readable(self):
        for v in ("none", '"none"', "None.", "#10", "#10, #11", "#10,#11"):
            self.assertIsNone(tickets.depends_error(f"**Depends on:** {v}"), v)

    def test_prose_is_unreadable(self):
        # #719's line, and #716's half-numbered one
        for v in ("the swimming, drowning and steering tickets in this plan",
                  "#713, and the swimming ticket in this plan",
                  "#712 (Blocked by #712: the numbers must be decided first)",
                  "#10 and #11", ""):
            self.assertIsNotNone(tickets.depends_error(f"**Depends on:** {v}"), v)

    def test_no_line_is_readable(self):
        self.assertIsNone(tickets.depends_error("Blocked by #3"))


class TicketErrors(unittest.TestCase):
    def test_good_ticket(self):
        self.assertEqual(errors(body()), [])
        self.assertEqual(errors(body("#10", "`crates/game/src/swim.rs`")), [])

    def test_prose_depends(self):
        e = errors(body("the swimming ticket in this plan"))
        self.assertEqual(len(e), 1)
        self.assertIn("Depends on", e[0])

    def test_unknown_plan_or_pr_dependency(self):
        e = errors(body("#99, #11, #12"))
        self.assertEqual([x.split(":")[0] for x in e],
                         ["Depends on #99", "Depends on #11", "Depends on #12"])

    def test_files(self):
        self.assertEqual(errors(body(files="`crates/x.rs` (new)")), [])
        self.assertIn("doesn't exist", errors(body(files="`crates/x.rs`"))[0])
        self.assertIn("no backticked", errors(body(files="the animals module"))[0])

    def test_files_from_the_dependency_chain(self):
        # shapes.rs comes from #21, which 22 depends on: allowed through the chain
        self.assertEqual(errors(body("#22", "`crates/mesh/src/shapes.rs`")), [])
        self.assertEqual(errors(body("#22", "`crates/mesh/src/plants.rs`")), [])
        # a ticket outside the chain still doesn't count
        self.assertIn("doesn't exist", errors(body("#10", "`crates/mesh/src/shapes.rs`"))[0])

    def test_chain_survives_a_cycle(self):
        self.assertEqual(errors(body("#31", "`b.rs`")), [])

    def test_missing_lines_and_sections(self):
        e = errors("**Depends on:** none\n")
        self.assertIn("needs exactly one `**Spec:**` line, found 0", e)
        self.assertIn("needs exactly one `**Files:**` line, found 0", e)
        self.assertIn("needs a `### What` section", e)
        self.assertIn("needs a `### Done when` section", e)


if __name__ == "__main__":
    unittest.main()
