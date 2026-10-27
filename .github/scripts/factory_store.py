"""The factory's state store: a SQLite file (stdlib only) holding docs, plans,
tickets, dependencies, decisions, runs and attempts.

The schema enforces the invariants of design rule 3 that a database can: a
ticket belongs to exactly one existing plan, and a dependency points at a
ticket, never a plan (an id is a plan or a ticket, not both).

open(path) -> connection; schema_version(conn) -> int; dump(conn) -> str.
"""
import sqlite3

SCHEMA_VERSION = 1

SCHEMA = """
CREATE TABLE docs (
    file   TEXT PRIMARY KEY,
    title  TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL
);
CREATE TABLE phases (
    doc   TEXT NOT NULL REFERENCES docs(file),
    name  TEXT NOT NULL,
    state TEXT NOT NULL,
    PRIMARY KEY (doc, name)
);
CREATE TABLE plans (
    id    INTEGER PRIMARY KEY,
    doc   TEXT REFERENCES docs(file),
    phase TEXT,
    state TEXT NOT NULL DEFAULT 'open'
);
CREATE TABLE tickets (
    id    INTEGER PRIMARY KEY,
    plan  INTEGER NOT NULL REFERENCES plans(id),
    title TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'proposed'
);
CREATE TABLE dependencies (
    ticket     INTEGER NOT NULL REFERENCES tickets(id),
    depends_on INTEGER NOT NULL REFERENCES tickets(id),
    PRIMARY KEY (ticket, depends_on),
    CHECK (ticket <> depends_on)
);
CREATE TABLE decisions (
    issue  INTEGER PRIMARY KEY,
    plan   INTEGER REFERENCES plans(id),
    answer TEXT,
    ticket INTEGER REFERENCES tickets(id)
);
CREATE TABLE runs (
    id     TEXT PRIMARY KEY,
    ticket INTEGER REFERENCES tickets(id),
    role   TEXT NOT NULL
);
CREATE TABLE attempts (
    run     TEXT NOT NULL REFERENCES runs(id),
    n       INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    PRIMARY KEY (run, n)
);
CREATE TRIGGER ticket_not_plan BEFORE INSERT ON tickets
WHEN EXISTS (SELECT 1 FROM plans WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'id is already a plan'); END;
CREATE TRIGGER plan_not_ticket BEFORE INSERT ON plans
WHEN EXISTS (SELECT 1 FROM tickets WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'id is already a ticket'); END;
"""


def schema_version(conn):
    return conn.execute("PRAGMA user_version").fetchone()[0]


def open(path):  # noqa: A001 - the module's public name is factory_store.open
    """Open the store at path, creating the tables on a new file."""
    conn = sqlite3.connect(path)
    conn.execute("PRAGMA foreign_keys = ON")
    version = schema_version(conn)
    if version == 0:
        with conn:
            conn.executescript("BEGIN;" + SCHEMA + f"PRAGMA user_version = {SCHEMA_VERSION};COMMIT;")
    elif version != SCHEMA_VERSION:
        conn.close()
        raise RuntimeError(f"store schema version {version}, expected {SCHEMA_VERSION}")
    return conn


def dump(conn):
    """Deterministic text form: tables by name, rows sorted, nothing added."""
    tables = [r[0] for r in conn.execute(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
    out = [f"schema_version {schema_version(conn)}"]
    for t in tables:
        cols = [r[1] for r in conn.execute(f'PRAGMA table_info("{t}")')]
        rows = sorted(conn.execute(f'SELECT * FROM "{t}"').fetchall(),
                      key=lambda r: [(v is not None, str(type(v)), v) for v in r])
        out.append(f"[{t}] {' '.join(cols)}")
        out.extend("  " + " | ".join("NULL" if v is None else str(v) for v in r) for r in rows)
    return "\n".join(out) + "\n"
