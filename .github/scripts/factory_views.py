"""Views rendered from the state store: what each card and label should show.

    render(store) -> {"tickets": {"<id>": view}, "plans": {"<id>": view}}

A ticket view is {"state", "status", "waiting", "labels"}: the board Status
column (names from `project_board.py`), the "Waiting on" text, and the labels
the tick manages. A plan view is {"state", "status", "waiting", "labels"} with
no card status (the board doesn't manage plans). Nothing GitHub-specific is
read and nothing is written to the store, so deleting a view and rendering it
again loses nothing.
(docs/design/factory-orchestrator.md Phase B, reconciler note 8)

The store holds no PR or review facts, so an `in_review` ticket shows
Submitted; the finer Blocked, Nits and Ready for Merge split stays with the
board pass until it moves onto the store.
"""
import os
import sys

os.environ.setdefault("GITHUB_REPOSITORY", "")  # project_board reads it on import
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import project_board  # noqa: E402

STATUS = {
    "proposed": "Inbox",
    "approved": "Approved",
    "waiting": "Approved",
    "running": "Approved",
    "in_review": "Submitted",
    "blocked_on_question": "Blocked",
    "failed": "Blocked",
    "merged": "Done",
    "done": "Done",
}
assert set(STATUS.values()) <= set(project_board.STATUSES) | set(project_board.OPTIONAL)


def render(store):
    states = dict(store.execute("SELECT id, state FROM tickets"))
    deps = {}
    for t, d in store.execute("SELECT ticket, depends_on FROM dependencies ORDER BY ticket, depends_on"):
        deps.setdefault(t, []).append(d)
    tickets = {}
    for n in sorted(states):
        state = states[n]
        open_deps = [d for d in deps.get(n, []) if states.get(d) != "done"]
        waits = state in ("proposed", "approved", "waiting") and open_deps
        tickets[str(n)] = {
            "state": state,
            "status": STATUS[state],
            "waiting": ", ".join(f"#{d}" for d in open_deps) if waits else "",
            "labels": ["planned"] + ([] if state == "proposed" else ["factory"]),
        }
    plans = {str(n): {"state": state, "status": None, "waiting": "", "labels": []}
             for n, state in store.execute("SELECT id, state FROM plans ORDER BY id")}
    return {"tickets": tickets, "plans": plans}
