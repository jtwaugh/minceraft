"""Tests for the timing maths. python3 -I test_factory_timing.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_timing import format_report, parse_runs, report, summarise  # noqa: E402

CUT = "2026-10-20T00:00:00Z"


def run(n, start, end, conclusion="success"):
    return {"ticket": n, "start": start, "end": end, "conclusion": conclusion}


class Test(unittest.TestCase):
    def test_groups_and_medians(self):
        runs = [
            run(1, "2026-10-18T10:00:00Z", "2026-10-18T10:30:00Z"),
            run(2, "2026-10-19T10:00:00Z", "2026-10-19T11:00:00Z"),
            run(3, "2026-10-19T12:00:00Z", "2026-10-19T12:10:00Z"),
            run(4, "2026-10-20T00:00:00Z", "2026-10-20T00:10:00Z"),
            run(5, "2026-10-21T00:00:00+00:00", "2026-10-21T00:20:00+00:00"),
        ]
        commits = {
            1: "2026-10-18T10:10:00Z",
            2: "2026-10-19T10:20:00Z",
            4: "2026-10-20T00:04:00Z",
            5: "2026-10-21T00:08:00Z",
        }
        s = summarise(runs, commits, CUT)
        self.assertEqual(s["before"]["tickets"], 3)
        self.assertEqual(s["before"]["median_wall"], 1800)
        self.assertEqual(s["before"]["median_to_first_commit"], 900)
        self.assertEqual(s["before"]["no_commit"], 1)
        self.assertEqual(s["after"]["tickets"], 2)
        self.assertEqual(s["after"]["median_wall"], 900)
        self.assertEqual(s["after"]["median_to_first_commit"], 360)
        self.assertEqual(s["after"]["no_commit"], 0)

    def test_empty_group_is_none(self):
        s = summarise([run(1, "2026-10-18T10:00:00Z", "2026-10-18T10:30:00Z")], {}, CUT)
        self.assertEqual(s["after"], {"tickets": 0, "median_wall": None,
                                      "median_to_first_commit": None, "no_commit": 0})
        self.assertIsNone(s["before"]["median_to_first_commit"])
        self.assertEqual(s["before"]["no_commit"], 1)
        self.assertEqual(summarise([], {}, CUT)["before"]["median_wall"], None)

    def test_cancelled_ignored(self):
        runs = [run(1, "2026-10-18T10:00:00Z", "2026-10-18T10:30:00Z", "cancelled")]
        self.assertEqual(summarise(runs, {1: "2026-10-18T10:05:00Z"}, CUT)["before"]["tickets"], 0)

    def test_reruns_sum_wall_time(self):
        runs = [
            run(1, "2026-10-18T10:00:00Z", "2026-10-18T10:10:00Z", "failure"),
            run(1, "2026-10-18T11:00:00Z", "2026-10-18T11:20:00Z"),
        ]
        s = summarise(runs, {1: "2026-10-18T10:05:00Z"}, CUT)["before"]
        self.assertEqual((s["tickets"], s["median_wall"], s["median_to_first_commit"]), (1, 1800, 300))


class TestReport(unittest.TestCase):
    RAW = [
        {"display_title": "Claude implementer #7", "run_started_at": "2026-10-18T10:00:00Z",
         "updated_at": "2026-10-18T10:30:00Z", "conclusion": "success"},
        {"display_title": "Claude implementer #8", "run_started_at": "2026-10-21T10:00:00Z",
         "updated_at": "2026-10-21T10:10:00Z", "conclusion": "success"},
        {"display_title": "Some issue title", "run_started_at": "2026-10-21T10:00:00Z",
         "updated_at": "2026-10-21T10:10:00Z", "conclusion": "success"},
        {"display_title": "Claude implementer #9x", "run_started_at": "2026-10-21T10:00:00Z",
         "updated_at": "2026-10-21T10:10:00Z", "conclusion": "success"},
    ]

    def test_parse_ignores_unnamed_runs(self):
        runs = parse_runs(self.RAW)
        self.assertEqual([r["ticket"] for r in runs], [7, 8])
        self.assertEqual(runs[0]["start"], "2026-10-18T10:00:00Z")

    def test_report_text(self):
        seen = []

        def commits(tickets):
            seen.append(tickets)
            return {7: "2026-10-18T10:10:00Z", 8: "2026-10-21T10:04:00Z"}

        text = report(CUT, lambda: parse_runs(self.RAW), commits)
        self.assertEqual(seen, [{7, 8}])
        self.assertEqual(text.splitlines(), [
            "Implementer timing, split at 2026-10-20T00:00:00Z",
            "        tickets  median wall  median to first commit  no commit",
            "before  1        30m          10m                     0",
            "after   1        10m          4m                      0",
        ])

    def test_empty_group_shows_dash(self):
        text = format_report(summarise([], {}, CUT), CUT)
        self.assertIn("before  0        -", text)


if __name__ == "__main__":
    unittest.main()
