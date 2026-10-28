"""Tests for worker_worktree.py. python3 -I test_worker_worktree.py (no network)."""
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "worker_worktree.py")
ENV = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
       "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


def git(cwd, *args):
    return subprocess.run(["git", "-C", cwd, *args], check=True, capture_output=True,
                          text=True, env=ENV).stdout.strip()


class Worktrees(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = os.path.realpath(self.tmp.name)
        self.origin, self.clone, self.root = (os.path.join(t, n) for n in ("origin", "clone", "wt"))
        subprocess.run(["git", "init", "-q", "-b", "master", self.origin], check=True)
        git(self.origin, "config", "receive.denyCurrentBranch", "ignore")
        self.commit(self.origin, "a")
        subprocess.run(["git", "clone", "-q", self.origin, self.clone], check=True)
        self.addCleanup(self.tmp.cleanup)

    def commit(self, repo, name):
        with open(os.path.join(repo, name), "w") as f:
            f.write(name)
        git(repo, "add", name)
        git(repo, "commit", "-qm", name)

    def run_script(self, *args):
        p = subprocess.run([sys.executable, "-I", SCRIPT, "--repo", self.clone, "--root", self.root, *args],
                           capture_output=True, text=True, env=ENV)
        self.assertEqual(p.returncode, 0, p.stderr)
        return dict(l.split("=", 1) for l in p.stdout.splitlines() if "=" in l) if args[0] == "prepare" \
            else p.stdout.split()

    def test_prepare_and_reuse(self):
        before = git(self.clone, "branch", "--show-current")
        out = self.run_script("prepare", "7")
        wt = out["worktree"]
        self.assertEqual(git(wt, "status", "--porcelain"), "")
        self.assertEqual(git(wt, "rev-parse", "HEAD"), git(self.origin, "rev-parse", "master"))
        self.assertTrue(git(wt, "branch", "--show-current").startswith("claude/issue-7-"))
        self.commit(self.origin, "b")            # master moves on
        with open(os.path.join(wt, "junk"), "w") as f:
            f.write("x")
        again = self.run_script("prepare", "7")
        self.assertEqual(again["worktree"], wt)
        self.assertEqual(git(wt, "status", "--porcelain"), "")
        self.assertEqual(git(wt, "rev-parse", "HEAD"), git(self.origin, "rev-parse", "master"))
        self.assertEqual(git(self.clone, "branch", "--show-current"), before)

    def test_two_tickets(self):
        a, b = self.run_script("prepare", "1"), self.run_script("prepare", "2")
        self.assertNotEqual(a["worktree"], b["worktree"])
        self.assertEqual(a["CARGO_TARGET_DIR"], b["CARGO_TARGET_DIR"])
        self.assertEqual(a["CARGO_TARGET_DIR"], os.path.join(self.root, "target"))

    def test_prune_only_finished(self):
        merged = self.run_script("prepare", "1")["worktree"]
        deleted = self.run_script("prepare", "2")["worktree"]
        open_ = self.run_script("prepare", "3")["worktree"]
        fresh = self.run_script("prepare", "4")["worktree"]
        for wt in (merged, deleted, open_):
            self.commit(wt, "w" + os.path.basename(wt))
        # 1: merged into master; 2: pushed, then remote branch deleted; 3: pushed, still open.
        git(merged, "push", "-q", "origin", "HEAD:master")
        git(self.origin, "reset", "-q", "--hard")
        for wt in (deleted, open_):
            git(wt, "push", "-q", "-u", "origin", "HEAD")
        git(self.origin, "branch", "-D", git(deleted, "branch", "--show-current"))
        self.assertEqual(self.run_script("prune"), ["1", "2"])
        self.assertFalse(os.path.exists(merged) or os.path.exists(deleted))
        self.assertTrue(os.path.exists(open_) and os.path.exists(fresh))


if __name__ == "__main__":
    unittest.main()
