"""Tests for factory_runner.py. python3 -I test_factory_runner.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import contextlib  # noqa: E402
import io  # noqa: E402

import factory_runner  # noqa: E402
import gh_cache  # noqa: E402
from factory_runner import FALLBACK, choose, load  # noqa: E402


def runner(status="online", labels=("self-hosted", "macOS", "warm"), busy=False):
    return {"status": status, "busy": busy, "labels": [{"name": n} for n in labels]}


def getter(found):
    calls = []

    def get(path):
        calls.append(path)
        if isinstance(found, Exception):
            raise found
        return found
    return get, calls


class Choose(unittest.TestCase):
    def test_unset(self):
        self.assertEqual(choose("", [runner()]), (FALLBACK, ""))
        self.assertEqual(choose(None, None), (FALLBACK, ""))

    def test_matching_runner_online(self):
        self.assertEqual(choose('"warm"', [runner()]), ('"warm"', ""))
        self.assertEqual(choose('["self-hosted", "warm"]', [runner()]), ('["self-hosted", "warm"]', ""))

    def test_busy_runner_counts(self):
        self.assertEqual(choose('"warm"', [runner(busy=True)]), ('"warm"', ""))

    def test_matching_runner_offline(self):
        out, note = choose('"warm"', [runner(status="offline")])
        self.assertEqual(out, FALLBACK)
        self.assertIn("no online runner has warm", note)

    def test_online_runner_with_the_wrong_labels(self):
        out, note = choose('["self-hosted", "linux"]', [runner()])
        self.assertEqual(out, FALLBACK)
        self.assertIn("linux", note)

    def test_one_match_among_many(self):
        rs = [runner(status="offline"), runner(labels=("self-hosted", "x")), runner()]
        self.assertEqual(choose('"warm"', rs)[0], '"warm"')

    def test_list_unreadable(self):
        out, note = choose('"warm"', None)
        self.assertEqual(out, FALLBACK)
        self.assertIn("couldn't read", note)

    def test_bad_json(self):
        out, note = choose("warm", [runner()])
        self.assertEqual(out, FALLBACK)
        self.assertIn("not a JSON", note)


class Load(unittest.TestCase):
    def test_unset_reads_nothing(self):
        get, calls = getter({"runners": [runner()]})
        self.assertEqual(load("o/r", get, {}), (FALLBACK, ""))
        self.assertEqual(calls, [])

    def test_reads_the_runner_list(self):
        get, calls = getter({"total_count": 1, "runners": [runner()]})
        self.assertEqual(load("o/r", get, {"FACTORY_RUNNER": '"warm"'}), ('"warm"', ""))
        self.assertEqual(calls, ["repos/o/r/actions/runners?per_page=100"])

    def test_unreadable_list_falls_back_and_says_so(self):
        for found in (RuntimeError("gh api: HTTP 403"), {"message": "Not Found"}, None):
            get, _ = getter(found)
            out, note = load("o/r", get, {"FACTORY_RUNNER": '"warm"'})
            self.assertEqual(out, FALLBACK, found)
            self.assertIn("couldn't read", note)


class Main(unittest.TestCase):
    """The command claude.yml's runner job runs: the choice on stdout, alone."""
    def run_main(self, found, variable):
        get, calls = getter(found)
        out, err = io.StringIO(), io.StringIO()
        old_get, old_env = gh_cache.get, os.environ.get("FACTORY_RUNNER")
        gh_cache.get, os.environ["FACTORY_RUNNER"] = get, variable
        try:
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = factory_runner.main(["factory_runner.py", "o/r"])
        finally:
            gh_cache.get = old_get
            if old_env is None:
                del os.environ["FACTORY_RUNNER"]
            else:
                os.environ["FACTORY_RUNNER"] = old_env
        return code, out.getvalue(), err.getvalue(), calls

    def test_online(self):
        code, out, err, calls = self.run_main({"runners": [runner()]}, '"warm"')
        self.assertEqual((code, out, err), (0, '"warm"\n', ""))
        self.assertEqual(calls, ["repos/o/r/actions/runners?per_page=100"])

    def test_offline_note_goes_to_stderr(self):
        code, out, err, _ = self.run_main({"runners": [runner(status="offline")]}, '"warm"')
        self.assertEqual((code, out), (0, FALLBACK + "\n"))
        self.assertIn("no online runner has warm", err)


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False).result.wasSuccessful() else 1)
