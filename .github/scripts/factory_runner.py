"""Which runner the tick sends the jobs it dispatches to (claude.yml's start
job, claude-fix.yml).

    choose(variable, runners) -> (runs-on JSON, note)
    load(repo, get, env)      -> the same, reading $FACTORY_RUNNER and the
                                 repo's self-hosted runners through `get`

`variable` is FACTORY_RUNNER as the workflows read it: a JSON label or list
("warm", ["self-hosted", "x"]). It is used when at least one runner is online
and has every label in it; busy runners count, since the job only queues
behind the current one. Otherwise, or when the variable is unset, the jobs go
to ubuntu-latest, so an asleep warm worker doesn't leave them queued. The note
says why a set variable wasn't used ("" when it was, or was unset).
(docs/design/factory-orchestrator.md Phase D)
"""
import json
import os

FALLBACK = '"ubuntu-latest"'


def choose(variable, runners):
    """`runners` is the API's runner list, or None when it couldn't be read."""
    variable = (variable or "").strip()
    if not variable:
        return FALLBACK, ""
    try:
        want = json.loads(variable)
        want = {want} if isinstance(want, str) else set(want)
    except (ValueError, TypeError):
        return FALLBACK, f"FACTORY_RUNNER {variable} is not a JSON label or list; using ubuntu-latest"
    if runners is None:
        return FALLBACK, "couldn't read the repo's runners; using ubuntu-latest"
    for r in runners:
        if r.get("status") == "online" and want <= {lb.get("name") for lb in r.get("labels", [])}:
            return variable, ""
    return FALLBACK, f"no online runner has {', '.join(sorted(want))}; using ubuntu-latest"


def load(repo, get, env=None):
    """Listing runners needs repo admin: `get` must read with PROJECT_TOKEN."""
    variable = (env if env is not None else os.environ).get("FACTORY_RUNNER", "")
    if not variable.strip():
        return choose("", None)
    try:
        runners = get(f"repos/{repo}/actions/runners?per_page=100")["runners"]
    except (RuntimeError, KeyError, TypeError):
        runners = None
    return choose(variable, runners)
