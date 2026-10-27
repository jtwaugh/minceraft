"""Ticket and plan states and the transition log (design section 3).

Every state change is a row with a timestamp and a reason, so "why is this
stuck" is a query. `rerun` and `fix_attempt` rows share the log but leave the
state alone. The log table is created on first use.

record_transition(store, subject, to_state, reason, at) -> bool (False if the
subject is already in that state); record_event(store, subject, kind, cause,
at); history(store, subject); time_in_state(store, subject, now);
why_stuck(store, subject). A subject is a ticket or plan id (never both).
"""
TICKET_STATES = ("proposed", "approved", "waiting", "running", "in_review",
                 "merged", "done", "blocked_on_question", "failed")
PLAN_STATES = ("open", "built", "phase_done")
EVENT_KINDS = ("rerun", "fix_attempt")

_TABLE = """
CREATE TABLE IF NOT EXISTS transitions (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    subject INTEGER NOT NULL,
    kind    TEXT NOT NULL,
    state   TEXT NOT NULL,
    reason  TEXT NOT NULL,
    at      REAL NOT NULL
)
"""


def _subject(store, subject):
    """(table, valid states, current state) for a ticket or plan id."""
    store.execute(_TABLE)
    for table, states in (("tickets", TICKET_STATES), ("plans", PLAN_STATES)):
        row = store.execute(f"SELECT state FROM {table} WHERE id = ?", (subject,)).fetchone()
        if row:
            return table, states, row[0]
    raise ValueError(f"unknown subject {subject}")


def record_transition(store, subject, to_state, reason, at):
    table, states, current = _subject(store, subject)
    if to_state not in states:
        raise ValueError(f"unknown state {to_state!r} for {table[:-1]} {subject}")
    if to_state == current:
        return False
    with store:
        store.execute("INSERT INTO transitions (subject, kind, state, reason, at) VALUES (?, 'transition', ?, ?, ?)",
                      (subject, to_state, reason, at))
        store.execute(f"UPDATE {table} SET state = ? WHERE id = ?", (to_state, subject))
    return True


def record_event(store, subject, kind, cause, at):
    """Log a rerun or fix_attempt (with its cause) without changing the state."""
    if kind not in EVENT_KINDS:
        raise ValueError(f"unknown event kind {kind!r}")
    _, _, current = _subject(store, subject)
    with store:
        store.execute("INSERT INTO transitions (subject, kind, state, reason, at) VALUES (?, ?, ?, ?, ?)",
                      (subject, kind, current, cause, at))


def history(store, subject):
    """Rows (kind, state, reason, at) in the order recorded."""
    _subject(store, subject)
    return store.execute("SELECT kind, state, reason, at FROM transitions WHERE subject = ? ORDER BY seq",
                         (subject,)).fetchall()


def _latest(store, subject):
    _subject(store, subject)
    return store.execute("SELECT state, reason, at FROM transitions WHERE subject = ? AND kind = 'transition' "
                         "ORDER BY seq DESC LIMIT 1", (subject,)).fetchone()


def why_stuck(store, subject):
    """(state, reason) of the latest transition, or None if it never moved."""
    row = _latest(store, subject)
    return row[:2] if row else None


def time_in_state(store, subject, now):
    """Seconds since the latest transition, or None if it never moved."""
    row = _latest(store, subject)
    return now - row[2] if row else None
