"""Copy new plans and their tickets from a tick snapshot into the state store.

ingest(conn, snapshot) adds or updates rows and is idempotent. The store
starts with the next plan: the first ingest records the highest Plan issue
number then existing as a cutoff, and plans at or below it, with their
tickets, are never stored. A ticket whose parent is not a stored plan is not
stored. Dependencies point only at stored tickets (never a plan).

The cutoff and the predicted files live in two small tables this module
creates on first use, so the store schema (factory_store) is unchanged.
"""
import os

os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")  # tickets.py reads it at import
from tickets import deps, predicted_files  # noqa: E402

EXTRA = """
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS predicted_files (
    ticket INTEGER NOT NULL REFERENCES tickets(id),
    path   TEXT NOT NULL,
    PRIMARY KEY (ticket, path)
);
"""


def cutoff(conn, plans):
    """The stored cutoff; recorded from the snapshot's plans on first use."""
    row = conn.execute("SELECT value FROM meta WHERE key = 'plan_cutoff'").fetchone()
    if row:
        return int(row[0])
    highest = max((p["number"] for p in plans), default=0)
    conn.execute("INSERT INTO meta VALUES ('plan_cutoff', ?)", (str(highest),))
    return highest


def ingest(conn, snapshot):
    with conn:
        for stmt in EXTRA.split(";"):
            if stmt.strip():
                conn.execute(stmt)
        limit = cutoff(conn, snapshot.plans)
        for p in sorted(snapshot.plans, key=lambda p: p["number"]):
            if p["number"] <= limit:
                continue
            src = p.get("plan_source") or {}
            doc = src.get("file")
            if doc:  # placeholder until the docs mirror fills in the header
                conn.execute("INSERT OR IGNORE INTO docs (file, status) VALUES (?, 'unknown')", (doc,))
            conn.execute(
                "INSERT INTO plans (id, doc, phase) VALUES (?, ?, ?) "
                "ON CONFLICT(id) DO UPDATE SET doc = excluded.doc, phase = excluded.phase",
                (p["number"], doc, src.get("section")))
        stored_plans = {r[0] for r in conn.execute("SELECT id FROM plans")}
        mine = [i for i in snapshot.issues if i.get("kind") == "ticket"
                and i.get("parent") in stored_plans and i["number"] > limit]
        for t in mine:
            conn.execute(
                "INSERT INTO tickets (id, plan, title) VALUES (?, ?, ?) "
                "ON CONFLICT(id) DO UPDATE SET plan = excluded.plan, title = excluded.title",
                (t["number"], t["parent"], t["title"]))
        stored = {r[0] for r in conn.execute("SELECT id FROM tickets")}
        for t in mine:
            n = t["number"]
            conn.execute("DELETE FROM dependencies WHERE ticket = ?", (n,))
            conn.executemany("INSERT INTO dependencies VALUES (?, ?)",
                             [(n, d) for d in sorted(deps(t.get("body"))) if d != n and d in stored])
            conn.execute("DELETE FROM predicted_files WHERE ticket = ?", (n,))
            conn.executemany("INSERT INTO predicted_files VALUES (?, ?)",
                             [(n, f) for f in sorted(predicted_files(t.get("body")))])
