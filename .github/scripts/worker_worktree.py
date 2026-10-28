#!/usr/bin/env python3
"""Per-ticket git worktrees for a persistent worker with a warm clone.

  worker_worktree.py [--repo DIR] [--root DIR] prepare <ticket>
      Fetch origin, then make (or reuse) the worktree for the ticket under
      the root, on a branch started from origin/master. A reused worktree is
      reset to origin/master and cleaned of untracked files, so unpushed
      commits in it are discarded. Prints two lines:
        worktree=<path>
        CARGO_TARGET_DIR=<root>/target     (shared by every ticket)
  worker_worktree.py [--repo DIR] [--root DIR] prune
      Fetch origin, then remove the worktrees (and branches) of tickets whose
      branch is finished: its commits are all in origin/master, or it was
      pushed and its remote branch has since been deleted. Prints each
      ticket removed. A worktree with no commits of its own is kept.

--repo is the warm clone (default: the current directory) and --root where
worktrees live (default: $WORKER_ROOT, else ../worktrees next to the clone).
The warm clone's own checkout is never changed: all work happens in
worktrees on new branches.
"""
import argparse
import os
import subprocess
import sys
import time


def git(repo, *args, check=True):
    p = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True)
    if check and p.returncode:
        sys.exit(f"git {' '.join(args)}: {p.stderr.strip()}")
    return p


def fetch(repo):
    git(repo, "fetch", "--prune", "origin")


def worktrees(repo, root):
    """{ticket: (path, branch)} for the worktrees under root."""
    out, found, path = git(repo, "worktree", "list", "--porcelain").stdout, {}, None
    for line in out.splitlines():
        if line.startswith("worktree "):
            path = line[9:]
        elif line.startswith("branch refs/heads/") and path:
            name = os.path.basename(path)
            if os.path.dirname(os.path.realpath(path)) == root and name.startswith("issue-"):
                found[name[6:]] = (path, line[18:])
    return found


def prepare(repo, root, ticket):
    fetch(repo)
    os.makedirs(root, exist_ok=True)
    path = os.path.join(root, f"issue-{ticket}")
    if ticket in worktrees(repo, root):
        git(path, "reset", "--hard", "origin/master")
        git(path, "clean", "-fdq")
        branch = git(path, "branch", "--show-current").stdout.strip()
    else:
        branch = f"claude/issue-{ticket}-{time.strftime('%Y%m%d-%H%M')}"
        git(repo, "worktree", "prune")
        git(repo, "worktree", "add", "--no-track", "-b", branch, path, "origin/master")
    git(repo, "config", f"branch.{branch}.workerbase", git(path, "rev-parse", "HEAD").stdout.strip())
    print(f"worktree={path}")
    print(f"CARGO_TARGET_DIR={os.path.join(root, 'target')}")


def finished(repo, branch):
    base = git(repo, "config", f"branch.{branch}.workerbase", check=False).stdout.strip()
    tip = git(repo, "rev-parse", branch).stdout.strip()
    if not base or tip == base:
        return False
    merged = git(repo, "merge-base", "--is-ancestor", tip, "origin/master", check=False)
    if merged.returncode == 0:
        return True
    pushed = git(repo, "config", f"branch.{branch}.remote", check=False).stdout.strip()
    gone = git(repo, "rev-parse", "--verify", "-q", f"origin/{branch}", check=False).returncode
    return bool(pushed) and gone != 0


def prune(repo, root):
    fetch(repo)
    for ticket, (path, branch) in sorted(worktrees(repo, root).items()):
        if finished(repo, branch):
            git(repo, "worktree", "remove", "--force", path)
            git(repo, "branch", "-D", branch)
            print(ticket)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--root")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("prepare").add_argument("ticket")
    sub.add_parser("prune")
    a = ap.parse_args()
    repo = os.path.realpath(a.repo)
    root = os.path.realpath(a.root or os.environ.get("WORKER_ROOT")
                            or os.path.join(os.path.dirname(repo), "worktrees"))
    if a.cmd == "prepare":
        prepare(repo, root, a.ticket)
    else:
        prune(repo, root)


if __name__ == "__main__":
    main()
