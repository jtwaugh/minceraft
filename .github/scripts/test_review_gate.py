"""Tests for review_gate.decide. python3 -I test_review_gate.py (no network)."""
import json
import os
import sys
import unittest

os.environ["GITHUB_REPOSITORY"] = "o/r"
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import review_gate as rg  # noqa: E402

HEAD = "a" * 40
PR = {"head": {"sha": HEAD}, "html_url": "http://pr"}


def comment(login, body, t="2026-01-02T00:00:00Z"):
    return {"user": {"login": login}, "body": body, "created_at": t,
            "updated_at": t, "html_url": "http://c"}


def verdict(word, sha=HEAD):
    return comment(rg.BOT, f"Verdict: {word}\nReviewed commit: `{sha[:7]}`")


def structured(word, sha=HEAD, t="2026-01-02T00:00:00Z"):
    body = json.dumps({"verdict": word, "reviewed_commit": sha[:7], "blockers": [], "nits": []})
    return comment(rg.BOT, f"<!-- factory-result role=reviewer -->\n```json\n{body}\n```", t)


class Decide(unittest.TestCase):
    def state(self, files, comments=()):
        return rg.decide(PR, files, list(comments), "2026-01-01T00:00:00Z")

    def test_empty_file_list_is_nothing_to_review(self):
        state, desc, _ = self.state([])
        self.assertEqual(state, "success")
        self.assertIn("No file changes", desc)

    def test_spec_and_design_only(self):
        self.assertEqual(self.state(["SPEC.md", "docs/design/x.md"])[0], "success")

    def test_code_waits_for_reviewer(self):
        self.assertEqual(self.state(["SPEC.md", "crates/a.rs"])[0], "pending")

    def test_ready(self):
        self.assertEqual(self.state(["a.rs"], [verdict("Ready")])[0], "success")

    def test_blocked(self):
        self.assertEqual(self.state(["a.rs"], [verdict("Blocked")])[0], "failure")

    def test_block_overridden(self):
        ov = comment(rg.MAINTAINER, "/override fine", "2026-01-03T00:00:00Z")
        self.assertEqual(self.state(["a.rs"], [verdict("Blocked"), ov])[0], "success")

    def test_review_skipped_by_override(self):
        ov = comment(rg.MAINTAINER, "/override skip", "2026-01-03T00:00:00Z")
        self.assertEqual(self.state(["a.rs"], [ov])[0], "success")

    def test_structured_verdicts(self):
        for word, want in (("ready", "success"), ("ready with nits", "success"), ("blocked", "failure")):
            self.assertEqual(self.state(["a.rs"], [structured(word)])[0], want, word)

    def test_structured_verdict_for_older_commit_is_pending(self):
        self.assertEqual(self.state(["a.rs"], [structured("ready", "b" * 40)])[0], "pending")

    def test_override_after_structured_block(self):
        ov = comment(rg.MAINTAINER, "/override fine", "2026-01-03T00:00:00Z")
        self.assertEqual(self.state(["a.rs"], [structured("blocked"), ov])[0], "success")

    def test_override_by_agent_ignored_after_structured_block(self):
        ov = comment(rg.BOT, "/override fine", "2026-01-03T00:00:00Z")
        self.assertEqual(self.state(["a.rs"], [structured("blocked"), ov])[0], "failure")

    def test_invalid_structured_result_is_ignored(self):
        bad = comment(rg.BOT, '<!-- factory-result role=reviewer -->\n```json\n{"verdict": "ready"}\n```')
        self.assertEqual(self.state(["a.rs"], [bad])[0], "pending")

    def test_structured_result_from_other_user_ignored(self):
        c = structured("ready")
        c["user"]["login"] = "someone"
        self.assertEqual(self.state(["a.rs"], [c])[0], "pending")


if __name__ == "__main__":
    unittest.main()
