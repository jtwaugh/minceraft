"""Keep the factory store on the `factory-state` branch.

`fetch(dest)` writes the committed store to a working path before the tick;
`publish(path)` commits and pushes it afterwards, only when its `dump()`
differs from the committed one. The tick is the store's only writer, so a
push refused because the branch moved raises PushRejected and is never forced.
Git goes through an injected runner `run(args, input=None) -> bytes`.

Steps only the maintainer can do (agents cannot edit workflows or branch
protection; design reconciler note 2):
  1. Create the `factory-state` branch (it may start empty; `publish` creates
     it with a root commit if it does not exist).
  2. Exempt the tick's identity from branch protection on `factory-state`.
  3. Edit `factory-tick.yml`: check out the repo with `contents: write` for
     that branch, call `fetch(dest)` before the tick and `publish(dest)`
     after it.
"""
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402

BRANCH = "factory-state"
REMOTE = "origin"
NAME = "factory-state.db"
MESSAGE = "Factory state: update store"


class GitError(RuntimeError):
    pass


class PushRejected(GitError):
    """The branch moved since fetch; the tick is the only writer, so this is a failure."""


def git(args, input=None):
    p = subprocess.run(["git", *args], input=input, capture_output=True)
    if p.returncode:
        raise GitError(f"git {' '.join(args)}: {p.stderr.decode(errors='replace').strip()}")
    return p.stdout


def _has_branch(run, remote, branch):
    return bool(run(["ls-remote", "--heads", remote, branch]).strip())


def _fetch_tip(run, remote, branch):
    """Fetch the branch tip only; return its commit id, or None when it does not exist."""
    if not _has_branch(run, remote, branch):
        return None
    ref = f"refs/remotes/{remote}/{branch}"
    run(["fetch", "--depth", "1", remote, f"+refs/heads/{branch}:{ref}"])
    return run(["rev-parse", ref]).decode().strip()


def fetch(dest, run=git, remote=REMOTE, branch=BRANCH, name=NAME):
    """Write the committed store to dest and return dest.

    With no branch, or no store on it, dest is left absent: an empty path for
    factory_store.open to create."""
    tip = _fetch_tip(run, remote, branch)
    if tip is not None:
        try:
            data = run(["show", f"{tip}:{name}"])
        except GitError:
            data = None
        if data is not None:
            with open(dest, "wb") as f:
                f.write(data)
            return dest
    if os.path.exists(dest):
        os.remove(dest)
    return dest


def _dump_bytes(data):
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "committed.db")
        with open(p, "wb") as f:
            f.write(data)
        return _dump_file(p)


def _dump_file(path):
    conn = factory_store.open(path)
    try:
        return factory_store.dump(conn)
    finally:
        conn.close()


def publish(path, run=git, remote=REMOTE, branch=BRANCH, name=NAME, message=MESSAGE):
    """Commit and push the store at path if it changed. Returns True when it pushed."""
    tip = _fetch_tip(run, remote, branch)
    if tip is not None:
        try:
            committed = _dump_bytes(run(["show", f"{tip}:{name}"]))
        except GitError:
            committed = None
        if committed == _dump_file(path):
            return False
    blob = run(["hash-object", "-w", path]).decode().strip()
    tree = run(["mktree"], input=f"100644 blob {blob}\t{name}\n".encode()).decode().strip()
    parent = ["-p", tip] if tip else []
    commit = run(["commit-tree", tree, *parent, "-m", message]).decode().strip()
    try:
        run(["push", remote, f"{commit}:refs/heads/{branch}"])
    except GitError as e:
        raise PushRejected(str(e)) from e
    return True
