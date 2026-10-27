"""The tick's store: open it, sync it from the snapshot, publish it.

`$FACTORY_STORE` names the store file; unset, every store pass is skipped and
the tick behaves as if there were no store. A dry run or a fixture tick works
on a copy of the file and never publishes. Later tickets add steps to
`store_sync`.
(docs/design/factory-orchestrator.md Phase B)
"""
import os
import shutil
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
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


def store_sync(snap):
    """Pass: bring the store up to date with the snapshot. Changes only the
    store, so it never returns actions."""
    if snap.store is not None:
        ingest(snap.store, snap)
        sync_states(snap, time.time())
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
