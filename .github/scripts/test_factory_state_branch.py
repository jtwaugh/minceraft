"""Tests for the state branch. python3 -I test_factory_state_branch.py"""
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_state_branch as sb  # noqa: E402
import factory_store  # noqa: E402

ENV = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.com",
       "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.com"}


def sh(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, capture_output=True, check=True).stdout


class Test(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        t = self.tmp.name
        self.bare = os.path.join(t, "remote.git")
        sh(t, "init", "--bare", "-q", self.bare)
        self.work = os.path.join(t, "work")
        sh(t, "clone", "-q", self.bare, self.work)
        self.calls = []

    def run_git(self, args, input=None):
        self.calls.append(args[0])
        p = subprocess.run(["git", *args], cwd=self.work, env=ENV, input=input, capture_output=True)
        if p.returncode:
            raise sb.GitError(p.stderr.decode())
        return p.stdout

    def make_store(self, name, plan_ids):
        path = os.path.join(self.tmp.name, name)
        conn = factory_store.open(path)
        for i in plan_ids:
            conn.execute("INSERT INTO plans (id) VALUES (?)", (i,))
        conn.commit()
        conn.close()
        return path

    def commits(self):
        try:
            return int(sh(self.bare, "rev-list", "--count", sb.BRANCH).strip())
        except subprocess.CalledProcessError:
            return 0

    def test_fetch_without_branch_is_empty_path(self):
        dest = os.path.join(self.tmp.name, "dest.db")
        self.assertEqual(sb.fetch(dest, run=self.run_git), dest)
        self.assertFalse(os.path.exists(dest))
        conn = factory_store.open(dest)
        self.addCleanup(conn.close)
        self.assertEqual(conn.execute("SELECT COUNT(*) FROM plans").fetchone()[0], 0)

    def test_publish_creates_branch_and_fetch_returns_file(self):
        path = self.make_store("a.db", [1])
        self.assertTrue(sb.publish(path, run=self.run_git))
        self.assertEqual(self.commits(), 1)
        dest = os.path.join(self.tmp.name, "dest.db")
        sb.fetch(dest, run=self.run_git)
        conn = factory_store.open(dest)
        self.addCleanup(conn.close)
        self.assertEqual(conn.execute("SELECT id FROM plans").fetchall(), [(1,)])

    def test_unchanged_store_makes_no_commit_or_push(self):
        path = self.make_store("a.db", [1])
        sb.publish(path, run=self.run_git)
        self.calls.clear()
        self.assertFalse(sb.publish(path, run=self.run_git))
        self.assertNotIn("push", self.calls)
        self.assertNotIn("commit-tree", self.calls)
        self.assertEqual(self.commits(), 1)

    def test_changed_store_makes_one_commit(self):
        sb.publish(self.make_store("a.db", [1]), run=self.run_git)
        self.assertTrue(sb.publish(self.make_store("b.db", [1, 2]), run=self.run_git))
        self.assertEqual(self.commits(), 2)

    def test_rejected_push_raises_and_is_not_forced(self):
        sb.publish(self.make_store("a.db", [1]), run=self.run_git)
        # Another writer advances the branch between our fetch and push.
        other = os.path.join(self.tmp.name, "other")
        sh(self.tmp.name, "clone", "-q", "-b", sb.BRANCH, self.bare, other)
        sh(other, "commit", "--allow-empty", "-q", "-m", "moved")
        moved = sh(other, "rev-parse", "HEAD").strip()

        def racing(args, input=None):
            if args[0] == "push":
                sh(other, "push", "-q", "origin", f"HEAD:refs/heads/{sb.BRANCH}")
            return self.run_git(args, input)

        with self.assertRaises(sb.PushRejected):
            sb.publish(self.make_store("b.db", [1, 2]), run=racing)
        self.assertEqual(sh(self.bare, "rev-parse", sb.BRANCH).strip(), moved)

    def test_fetch_asks_for_depth_one(self):
        sb.publish(self.make_store("a.db", [1]), run=self.run_git)
        seen = []

        def spy(args, input=None):
            seen.append(args)
            return self.run_git(args, input)

        sb.fetch(os.path.join(self.tmp.name, "out.db"), run=spy)
        fetches = [a for a in seen if a[0] == "fetch"]
        self.assertEqual(len(fetches), 1)
        self.assertEqual(fetches[0][1:3], ["--depth", "1"])


if __name__ == "__main__":
    unittest.main()
