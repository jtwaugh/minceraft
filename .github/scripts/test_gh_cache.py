"""gh_cache: conditional requests that outlive the run, and one read per path."""
import json
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gh_cache  # noqa: E402


class Fake:
    """Stands in for subprocess.run: serves `pages` (path -> body) with an
    ETag per path, and answers 304 when If-None-Match matches."""

    def __init__(self, pages):
        self.pages, self.calls = pages, []

    def __call__(self, cmd, **kw):
        path = cmd[cmd.index("GET") + 1]
        sent = next((cmd[i + 1] for i, c in enumerate(cmd) if c == "-H"), "")
        self.calls.append((path, sent))
        body = self.pages[path]
        etag = f'W/"{hash(json.dumps(body))}"'
        if sent == f"If-None-Match: {etag}":
            return mock.Mock(stdout="HTTP/2.0 304 Not Modified\n\n", stderr="", returncode=1)
        return mock.Mock(stdout=f"HTTP/2.0 200 OK\nEtag: {etag}\n\n" + json.dumps(body),
                         stderr="", returncode=0)


class GhCacheTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.file = os.path.join(self.dir, "etags.json")
        self.env = mock.patch.dict(os.environ, {"FACTORY_HTTP_CACHE": self.file})
        self.env.start()
        self.addCleanup(self.env.stop)

    def run_with(self, fake, fn):
        with mock.patch("gh_cache.subprocess.run", fake):
            return fn()

    def test_a_path_is_read_once_per_run(self):
        fake = Fake({"repos/o/r/issues/1": {"n": 1}})
        gh_cache.begin()
        for _ in range(3):
            self.assertEqual(self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues/1")), {"n": 1})
        self.assertEqual(len(fake.calls), 1)

    def test_unchanged_read_next_run_is_a_free_304(self):
        fake = Fake({"repos/o/r/issues/1": {"n": 1}})
        gh_cache.begin()
        self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues/1"))
        gh_cache.save()
        gh_cache.begin()  # the next tick
        self.assertEqual(self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues/1")), {"n": 1})
        self.assertTrue(fake.calls[-1][1].startswith("If-None-Match: "))
        self.assertEqual(gh_cache.stats, {"requests": 1, "not_modified": 1})

    def test_changed_read_gets_the_new_body(self):
        pages = {"repos/o/r/issues/1": {"n": 1}}
        fake = Fake(pages)
        gh_cache.begin()
        self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues/1"))
        gh_cache.save()
        pages["repos/o/r/issues/1"] = {"n": 2}
        gh_cache.begin()
        self.assertEqual(self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues/1")), {"n": 2})
        self.assertEqual(gh_cache.stats["not_modified"], 0)

    def test_get_all_pages_and_shares_page_one_with_get(self):
        first = [{"i": i} for i in range(100)]
        fake = Fake({"repos/o/r/issues?state=all&per_page=100": first,
                     "repos/o/r/issues?state=all&per_page=100&page=2": [{"i": 100}]})
        gh_cache.begin()
        out = self.run_with(fake, lambda: gh_cache.get_all("repos/o/r/issues?state=all"))
        self.assertEqual(len(out), 101)
        self.run_with(fake, lambda: gh_cache.get("repos/o/r/issues?state=all&per_page=100"))
        self.assertEqual(len(fake.calls), 2)

    def test_save_keeps_only_what_this_run_read(self):
        fake = Fake({"a": [1], "b": [2]})
        gh_cache.begin()
        self.run_with(fake, lambda: (gh_cache.get("a"), gh_cache.get("b")))
        gh_cache.save()
        gh_cache.begin()
        self.run_with(fake, lambda: gh_cache.get("a"))
        gh_cache.save()
        with open(self.file) as f:
            self.assertEqual(list(json.load(f)["etags"]), ["a"])

    def test_state_is_kept_between_runs(self):
        gh_cache.begin()
        gh_cache.state["x"] = 1
        gh_cache.save()
        gh_cache.begin()
        self.assertEqual(gh_cache.state, {"x": 1})

    def test_the_first_format_still_loads(self):
        with open(self.file, "w") as f:
            json.dump({"a": ["W/\"1\"", [1]]}, f)
        gh_cache.begin()
        fake = Fake({"a": [1]})
        self.run_with(fake, lambda: gh_cache.get("a"))
        self.assertEqual(fake.calls[0][1], 'If-None-Match: W/"1"')

    def test_a_damaged_file_only_costs_full_reads(self):
        with open(self.file, "w") as f:
            f.write("{not json")
        fake = Fake({"a": [1]})
        gh_cache.begin()
        self.assertEqual(self.run_with(fake, lambda: gh_cache.get("a")), [1])
        self.assertEqual(fake.calls[0][1], "")

    def test_an_error_raises_with_ghs_message(self):
        def fail(cmd, **kw):
            return mock.Mock(stdout="HTTP/2.0 403 Forbidden\n\n{}", stderr="API rate limit exceeded", returncode=1)
        gh_cache.begin()
        with mock.patch("gh_cache.subprocess.run", fail):
            with self.assertRaisesRegex(RuntimeError, "rate limit"):
                gh_cache.get("a")


class LoaderTest(unittest.TestCase):
    def test_loader_reads_each_path_once_and_only_docs_branches(self):
        os.environ.pop("FACTORY_HTTP_CACHE", None)
        import factory_tick
        import tickets
        seen = []
        issues = [{"number": 77, "title": "t", "state": "open", "body": "", "labels": [{"name": "factory"}]}]

        def fake(cmd, **kw):
            path = cmd[cmd.index("GET") + 1]
            seen.append(path)
            body = issues if path.startswith("repos/o/r/issues?") else []
            return mock.Mock(stdout="HTTP/2.0 200 OK\n\n" + json.dumps(body), stderr="", returncode=0)
        with mock.patch.dict(os.environ, {"GITHUB_REPOSITORY": "o/r"}), \
                mock.patch.object(tickets, "REPO", "o/r"), mock.patch("gh_cache.subprocess.run", fake):
            snap = factory_tick.Snapshot.from_github("o/r")
            tickets.comments(77)  # the board loader asks again: no second read
        self.assertIn("repos/o/r/issues?state=all&sort=created&direction=asc&per_page=100", seen)
        self.assertEqual(seen.count("repos/o/r/issues/77/comments?per_page=100"), 1)
        self.assertIn("repos/o/r/git/matching-refs/heads/docs", seen)
        self.assertFalse(any("/branches" in p for p in seen))
        self.assertEqual(snap.requests, len(seen))


class IncrementalTest(unittest.TestCase):
    def issue(self, n, at, title="t"):
        return {"number": n, "title": title, "state": "open", "labels": [], "body": "",
                "updated_at": at, "user": {"login": "x"}}

    def test_full_then_since_then_full_again_after_an_hour(self):
        from datetime import datetime, timedelta, timezone
        import factory_tick
        asked = []

        def paged(path):
            asked.append(path)
            if "since=" in path:
                return [self.issue(2, "2026-10-21T10:05:00Z", "renamed"), self.issue(3, "2026-10-21T10:06:00Z")]
            return [self.issue(1, "2026-10-21T09:00:00Z"), self.issue(2, "2026-10-21T10:00:00Z")]
        state, t0 = {}, datetime(2026, 10, 21, 10, 0, tzinfo=timezone.utc)
        with mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": "workflow_run"}):
            first = factory_tick.read_issues("o/r", state, paged, t0)
            second = factory_tick.read_issues("o/r", state, paged, t0 + timedelta(minutes=10))
            self.assertEqual(state["issues_since"], "2026-10-21T10:06:00Z")
            factory_tick.read_issues("o/r", state, paged, t0 + timedelta(minutes=70))
        self.assertEqual([i["number"] for i in first], [1, 2])
        self.assertEqual(asked[1], "repos/o/r/issues?state=all&sort=updated&direction=asc"
                                   "&since=2026-10-21T10:00:00Z")
        self.assertEqual([(i["number"], i["title"]) for i in second], [(1, "t"), (2, "renamed"), (3, "t")])
        self.assertNotIn("user", second[0])  # only the fields the loader reads are kept
        self.assertNotIn("since=", asked[2])  # the hourly full re-read

    def test_scheduled_and_hand_started_ticks_read_everything(self):
        import factory_tick
        state = {"issues": {"1": self.issue(1, "x")}, "issues_since": "x",
                 "issues_full_at": "2999-01-01T00:00:00+00:00"}
        for event in factory_tick.FULL_EVENTS:
            asked = []
            with mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": event}):
                factory_tick.read_issues("o/r", dict(state), lambda p: asked.append(p) or [])
            self.assertNotIn("since=", asked[0])

    def test_unchanged_needs_all_304s_a_clean_last_tick_and_an_event(self):
        import factory_tick
        clean, free = {"last_tick_clean": True}, {"requests": 9, "not_modified": 9}
        with mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": "workflow_run"}):
            self.assertTrue(factory_tick.unchanged(free, clean))
            self.assertFalse(factory_tick.unchanged({"requests": 9, "not_modified": 8}, clean))
            self.assertFalse(factory_tick.unchanged(free, {"last_tick_clean": False}))
            self.assertFalse(factory_tick.unchanged(free, {}))
        for event in factory_tick.FULL_EVENTS:
            with mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": event}):
                self.assertFalse(factory_tick.unchanged(free, clean))


class SkipTest(unittest.TestCase):
    def test_an_unchanged_repo_after_a_clean_tick_skips_the_board(self):
        import factory_tick
        import tickets
        d = tempfile.mkdtemp()
        env = {"GITHUB_REPOSITORY": "o/r", "GITHUB_EVENT_NAME": "workflow_run",
               "FACTORY_HTTP_CACHE": os.path.join(d, "c.json"), "PROJECT_TOKEN": "p", "PROJECT_NUMBER": "1"}
        fake = Fake({"repos/o/r/issues?state=all&sort=created&direction=asc&per_page=100": [],
                     "repos/o/r/pulls?state=open&per_page=100": [],
                     "repos/o/r/contents/.github/CODEOWNERS": {},
                     "repos/o/r/actions/workflows/ci.yml/runs?branch=master&event=push&status=completed&per_page=30": {},
                     "repos/o/r/git/matching-refs/heads/docs": [],
                     "repos/o/r/actions/workflows/claude.yml/runs?per_page=100": {},
                     "repos/o/r/git/matching-refs/heads/claude/issue-": []})
        boards = []
        with mock.patch.dict(os.environ, env), mock.patch.object(tickets, "REPO", "o/r"), \
                mock.patch("gh_cache.subprocess.run", fake), \
                mock.patch("factory_board.load_board", lambda *a: boards.append(a)):
            first = factory_tick.Snapshot.from_github("o/r")   # cold: full read, board read
            gh_cache.state["last_tick_clean"] = True
            gh_cache.save()
            second = factory_tick.Snapshot.from_github("o/r")  # all 304: skip
        self.assertFalse(first.unchanged)
        self.assertTrue(second.unchanged)
        self.assertEqual(len(boards), 1)
        self.assertEqual(second.requests, second.not_modified)
        self.assertGreater(second.requests, 0)


if __name__ == "__main__":
    unittest.main()
