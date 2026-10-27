"""The tick's store: open it, sync it from the snapshot, publish it.

`$FACTORY_STORE` names the store file; unset, every store pass is skipped and
the tick behaves as if there were no store. A dry run or a fixture tick works
on a copy of the file and never publishes. Later tickets add steps to
`store_sync`. The docs table mirrors the headers of `docs/design/*.md`
(status and per-phase state); the header stays the source of truth and no doc
is ever written.
(docs/design/factory-orchestrator.md Phase B)
"""
import os
import shutil
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import plan_targets  # noqa: E402
from factory_ingest import ingest  # noqa: E402
from factory_store_states import sync_states  # noqa: E402


def open_store(path, live):
    """Open the store at path, or None when path is unset. Not live: open a
    temporary copy, so the file itself is never modified. Returns (conn, file
    the connection uses)."""
    if not path:
        return None, None
    if not live:
        copy = os.path.join(tempfile.mkdtemp(prefix="factory-store-"), os.path.basename(path))
        if os.path.exists(path):
            shutil.copyfile(path, copy)
        path = copy
    return factory_store.open(path), path


DESIGN_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "docs", "design")


def sync_docs(conn, design_dir=DESIGN_DIR):
    """Overwrite the docs and phases tables from the design-doc headers. A doc
    that is gone is removed, or kept as an 'unknown' placeholder while a plan
    still points at it."""
    found = {}
    for name in sorted(os.listdir(design_dir)) if os.path.isdir(design_dir) else []:
        if not name.endswith(".md"):
            continue
        status, phases = plan_targets.header(os.path.join(design_dir, name))
        if status:
            found[f"docs/design/{name}"] = (status, phases)
    with conn:
        for file, (status, phases) in found.items():
            conn.execute("INSERT INTO docs (file, status) VALUES (?, ?) "
                         "ON CONFLICT(file) DO UPDATE SET status = excluded.status", (file, status))
            conn.execute("DELETE FROM phases WHERE doc = ?", (file,))
            conn.executemany("INSERT OR REPLACE INTO phases VALUES (?, ?, ?)",
                             [(file, n, st) for n, st in phases])
        for (file,) in conn.execute("SELECT file FROM docs").fetchall():
            if file in found:
                continue
            conn.execute("DELETE FROM phases WHERE doc = ?", (file,))
            if conn.execute("SELECT 1 FROM plans WHERE doc = ?", (file,)).fetchone():
                conn.execute("UPDATE docs SET status = 'unknown' WHERE file = ?", (file,))
            else:
                conn.execute("DELETE FROM docs WHERE file = ?", (file,))


def store_sync(snap):
    """Pass: bring the store up to date with the snapshot. Changes only the
    store, so it never returns actions."""
    if snap.store is not None:
        ingest(snap.store, snap)
        sync_states(snap, time.time())
        sync_docs(snap.store)
    return []


def finish(snap, path, live, publish=None):
    """Close the store; a live tick publishes it. Returns True when it pushed."""
    if snap.store is None:
        return False
    snap.store.close()
    if not live:
        return False
    if publish is None:
        from factory_state_branch import publish
    return publish(path)
