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
            self.assertEqual(list(json.load(f)), ["a"])

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


if __name__ == "__main__":
    unittest.main()
