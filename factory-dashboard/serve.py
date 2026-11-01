#!/usr/bin/env python3
"""Mince Raft factory dashboard: a small local page instead of a GitHub tab.

    python3 serve.py --repo ~/code/minceraft          # opens http://127.0.0.1:8765
    python3 serve.py --repo ~/code/minceraft --read-only
    python3 serve.py --repo . --port 9000 --every 300 --no-open

Needs only Python 3.9+ and an authenticated `gh` CLI (`gh auth status`).
Every refresh runs collect.py in a fresh process that reuses the repo's own
factory scripts, so the page judges state exactly the way the tick does.
That process exits when it's done; the server itself holds one JSON blob.

Buttons post comments, close issues and add labels as you, through `gh`.
The server listens on 127.0.0.1 only, and every write needs a per-launch
token the page gets from the server, so another site can't make your
browser post on your behalf.
"""
import argparse
import json
import os
import re
import secrets
import shlex
import subprocess
import tempfile
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
LABELS = {"factory", "local", "hold"}  # the only labels the page may add
TOKEN = secrets.token_urlsafe(24)
PENDING_MAX = 900  # seconds a local write is shown before GitHub must confirm it


class State:
    def __init__(self, repo_dir, repo, every):
        self.repo_dir, self.repo, self.every = repo_dir, repo, every
        self.data, self.error, self.at = None, None, 0.0
        self.busy = False
        self.prev_runner = None  # FACTORY_RUNNER before the page cleared it, for "switch back"
        self.pending = []  # writes GitHub hasn't shown back yet: {"action", "number", "label", "at"}
        self.lock = threading.Lock()
        self.wake = threading.Event()

    def refresh(self):
        with self.lock:
            if self.busy:
                return
            self.busy = True
        started = time.time()
        try:
            cache = os.path.join(os.path.expanduser("~"), ".cache", "minceraft-dashboard", "http.json")
            env = {**os.environ, "GITHUB_REPOSITORY": self.repo, "FACTORY_HTTP_CACHE": cache}
            p = subprocess.run([sys.executable, "-B", os.path.join(HERE, "collect.py"), self.repo_dir],
                               capture_output=True, text=True, env=env, timeout=600)
            if p.returncode:
                tail = [ln for ln in p.stderr.strip().splitlines() if ln.strip()][-3:]
                self.error = "collect.py failed: " + " / ".join(tail)
            else:
                self.data, self.error = json.loads(p.stdout), None
                self.settle(started)
        except Exception as e:  # keep serving the last good data
            self.error = f"refresh failed: {e}"
        finally:
            self.at = time.time()
            self.busy = False

    def loop(self):
        while True:
            self.refresh()
            self.wake.wait(self.every)
            self.wake.clear()

    def note(self, body):
        """Remember a write that changes what the page lists, until a read shows it."""
        if body.get("action") in ("close", "label"):
            with self.lock:
                self.pending.append({"action": body["action"], "number": body["number"],
                                     "label": body.get("label"), "at": time.time()})

    def settle(self, started):
        """Drop writes that a read begun after them confirms, or that are too old."""
        ny = (self.data or {}).get("needs_you") or []
        st = (self.data or {}).get("started") or []

        def confirmed(w):
            if w["at"] > started:
                return False  # this read began before the write
            here = [i for i in ny if i["number"] == w["number"]]
            if w["action"] == "close":
                return not here and all(t["number"] != w["number"] for t in st)
            return all(w["label"] in i.get("labels", []) for i in here)

        with self.lock:
            now = time.time()
            self.pending = [w for w in self.pending
                            if not confirmed(w) and now - w["at"] < PENDING_MAX]

    def overlaid(self):
        """The last read with local writes applied, so a closed item disappears at once."""
        with self.lock:
            pending = list(self.pending)
        if not self.data or not pending:
            return self.data
        closed = {w["number"] for w in pending if w["action"] == "close"}
        labels = {}
        for w in pending:
            if w["action"] == "label":
                labels.setdefault(w["number"], set()).add(w["label"])
        d = dict(self.data)
        ny = []
        for i in d.get("needs_you") or []:
            add = labels.get(i["number"], set())
            if i["number"] in closed or (i["type"] == "unapproved" and "factory" in add):
                continue
            ny.append({**i, "labels": sorted(set(i.get("labels", [])) | add)} if add else i)
        d["needs_you"] = ny
        if d.get("started") is not None:
            d["started"] = [t for t in d["started"] if t["number"] not in closed]
        return d

    def payload(self):
        with self.lock:
            n = len(self.pending)
        return {"data": self.overlaid(), "error": self.error, "refreshed_at": self.at,
                "busy": self.busy, "every": self.every, "pending": n,
                "prev_runner": self.prev_runner}


def gh_write(args, dry):
    if dry:
        print("[dry write] gh " + " ".join(args), flush=True)
        return True, "dry run: nothing sent"
    p = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60)
    return p.returncode == 0, (p.stderr or p.stdout).strip()[:500]


TICK_WORKFLOW = "factory-tick.yml"
NEVER_LIVE_TICK = ("Never run .github/scripts/factory_tick.py without --dry-run: run bare, it ticks live "
                   "and writes to GitHub as me. `python3 .github/scripts/factory_health.py --repro` runs it dry.")


def investigate_prompt(state, topic):
    """What a local Claude is asked to look into. Built here from the last read, never from the request."""
    d = state.data or {}
    h, runs = d.get("health") or {}, d.get("agent_runs") or []
    rules = ("Investigate only: don't push, comment, re-run workflows or change repo settings without "
             f"asking me first. {NEVER_LIVE_TICK} End with the cause and the fix you propose.")
    if topic == "runner":
        lost = [x for x in runs if re.search("lost communication", x.get("error") or "", re.I)]
        names = sorted({x["runner"] for x in lost if x.get("runner")}) or ["the self-hosted runner"]
        cur = (d.get("runner") or {}).get("value") or "unset (ubuntu-latest)"
        return (f"The factory's self-hosted runner ({', '.join(names)}) lost communication with GitHub "
                f"mid-run on {len(lost)} recent agent runs:\n"
                + "".join(f"- {x['title']}: {x['url']}\n" for x in lost)
                + f"vars.FACTORY_RUNNER is {cur}. Find out why it drops. If the runner is on this machine, "
                  "check its service and _diag logs, sleep and power settings, network, disk and memory; "
                  f"if it isn't, say what to check on the machine it's on. {rules}")
    f = h.get("first_failure") or {}
    lines = [f"The Mince Raft factory tick ({TICK_WORKFLOW}) needs a look."]
    if h.get("workflow_state") not in (None, "active"):
        lines.append(f"The workflow is {h['workflow_state']}.")
    if h.get("failing"):
        lines.append(f"It has failed {h['failing']} time(s) in a row since {f.get('at')}; first failed run: "
                     f"{f.get('url')}. The error: {' / '.join(f.get('why') or []) or 'see the log'}.")
    elif h.get("latest"):
        lines.append(f"The last tick ran at {h['latest']['at']}.")
    for c in (d.get("next_tick") or {}).get("crashed") or []:
        lines.append(f"In a dry run the {c['pass']} pass crashes: {c['error']}")
    lines.append(f"Find the cause, with `gh run view <id> --log-failed` and the scripts in .github/scripts. {rules}")
    return "\n".join(lines)


def open_terminal(state, prompt, dry):
    """Start an interactive Claude in a new Terminal window in the repo, with the prompt as its first message."""
    fd, path = tempfile.mkstemp(prefix="factory-investigate-", suffix=".txt")
    with os.fdopen(fd, "w") as fh:
        fh.write(prompt)
    # The prompt goes through a file, so nothing from GitHub is ever parsed by the shell.
    cmd = (f"cd {shlex.quote(state.repo_dir)} && p=$(cat {shlex.quote(path)}) && "
           f"rm -f {shlex.quote(path)} && claude \"$p\"")
    if dry:
        os.remove(path)
        print(f"[dry terminal] {cmd}", flush=True)
        return True, "dry run: no terminal opened"
    if sys.platform != "darwin":
        return False, f"Opening a terminal only works on macOS. Run this yourself: {cmd}"
    script = cmd.replace("\\", "\\\\").replace('"', '\\"')
    p = subprocess.run(["osascript", "-e", f'tell application "Terminal" to do script "{script}"',
                        "-e", 'tell application "Terminal" to activate'], capture_output=True, text=True)
    return p.returncode == 0, (p.stderr.strip() or "Claude is starting in a new Terminal window.")[:500]


def act_ops(state, body, dry):
    """Remediation for the alerts. Every target is checked against the last read."""
    kind, d, R = body.get("action"), state.data or {}, ["-R", state.repo]
    h = d.get("health") or {}
    if kind == "investigate":
        if body.get("topic") not in ("runner", "tick"):
            raise ValueError("unknown topic")
        return open_terminal(state, investigate_prompt(state, body["topic"]), dry)
    if kind == "rerun":
        known = {x["id"] for x in d.get("agent_runs") or [] if x.get("conclusion") not in (None, "success")}
        known |= {t["id"] for t in (h.get("latest"), h.get("first_failure")) if t and t.get("id")}
        ids = body.get("runs")
        if not isinstance(ids, list) or not ids or not all(isinstance(i, int) and i in known for i in ids):
            raise ValueError("runs must be failed runs the dashboard listed")
        for i in ids:
            ok, msg = gh_write(["run", "rerun", str(i), "--failed", *R], dry)
            if not ok:
                return ok, f"run {i}: {msg}"
        return True, f"re-running {len(ids)} run(s)"
    if kind in ("tick-run", "tick-enable"):
        return gh_write(["workflow", kind[5:], TICK_WORKFLOW, *R], dry)
    if kind == "runner":
        if body.get("to") == "hosted":
            cur = (d.get("runner") or {}).get("value")
            ok, msg = gh_write(["variable", "delete", "FACTORY_RUNNER", *R], dry)
            if ok and cur:
                state.prev_runner = cur
            return ok, msg
        if body.get("to") == "back" and state.prev_runner:
            ok, msg = gh_write(["variable", "set", "FACTORY_RUNNER", "--body", state.prev_runner, *R], dry)
            if ok:
                state.prev_runner = None
            return ok, msg
        raise ValueError("runner: to must be hosted, or back after switching")
    return None


def act(state, body, dry):
    """One write. Returns (ok, message); raises ValueError on a bad request."""
    done = act_ops(state, body, dry)
    if done is not None:
        return done
    n = body.get("number")
    if not isinstance(n, int) or n <= 0:
        raise ValueError("bad issue number")
    base = f"repos/{state.repo}/issues/{n}"
    kind = body.get("action")
    if kind == "comment":
        text = (body.get("body") or "").strip()
        if not text:
            raise ValueError("empty comment")
        return gh_write(["api", "-X", "POST", f"{base}/comments", "-f", f"body={text}"], dry)
    if kind == "close":
        reason = body.get("reason") if body.get("reason") in ("completed", "not_planned") else "completed"
        note = (body.get("comment") or "").strip()
        if note:
            ok, msg = gh_write(["api", "-X", "POST", f"{base}/comments", "-f", f"body={note}"], dry)
            if not ok:
                return ok, msg
        return gh_write(["api", "-X", "PATCH", base, "-f", "state=closed",
                         "-f", f"state_reason={reason}"], dry)
    if kind == "label":
        label = body.get("label")
        if label not in LABELS:
            raise ValueError(f"label must be one of {sorted(LABELS)}")
        return gh_write(["api", "-X", "POST", f"{base}/labels", "-f", f"labels[]={label}"], dry)
    raise ValueError("unknown action")


def make_handler(state, port, read_only, dry):
    hosts = {f"127.0.0.1:{port}", f"localhost:{port}"}
    page = open(os.path.join(HERE, "index.html"), encoding="utf-8").read()

    class H(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def send(self, code, obj, ctype="application/json"):
            raw = obj.encode() if isinstance(obj, str) else json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("Content-Type", ctype + "; charset=utf-8")
            self.send_header("Content-Length", str(len(raw)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            self.wfile.write(raw)

        def host_ok(self):
            return self.headers.get("Host") in hosts  # blocks DNS rebinding

        def do_GET(self):
            if not self.host_ok():
                return self.send(403, {"error": "bad host"})
            if self.path in ("/", "/index.html"):
                cfg = json.dumps({"token": TOKEN, "readOnly": read_only, "repo": state.repo})
                return self.send(200, page.replace("/*CONFIG*/{}", cfg), "text/html")
            if self.path == "/api/state":
                return self.send(200, state.payload())
            self.send(404, {"error": "not found"})

        def do_POST(self):
            if not self.host_ok() or self.headers.get("X-Dashboard-Token") != TOKEN:
                return self.send(403, {"error": "forbidden"})
            origin = self.headers.get("Origin")
            if origin and re.sub(r"^https?://", "", origin) not in hosts:
                return self.send(403, {"error": "bad origin"})
            if self.path == "/api/refresh":
                state.wake.set()
                return self.send(200, {"ok": True})
            if self.path != "/api/act":
                return self.send(404, {"error": "not found"})
            if read_only:
                return self.send(403, {"ok": False, "message": "dashboard is read-only"})
            try:
                n = int(self.headers.get("Content-Length") or 0)
                body = json.loads(self.rfile.read(min(n, 65536)) or b"{}")
            except ValueError:
                return self.send(400, {"ok": False, "message": "bad JSON"})
            try:
                ok, msg = act(state, body, dry)
            except ValueError as e:  # the request itself was wrong
                return self.send(400, {"ok": False, "message": str(e)})
            if ok:
                state.note(body)  # show it now; the next read confirms it
                state.wake.set()
            self.send(200 if ok else 502, {"ok": ok, "message": msg})

    return H


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=".", help="path to your minceraft checkout (default: .)")
    ap.add_argument("--github", default="jtwaugh/minceraft", help="owner/name on GitHub")
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--every", type=int, default=300, help="seconds between refreshes (default 300)")
    ap.add_argument("--read-only", action="store_true", help="hide and refuse every write")
    ap.add_argument("--no-open", action="store_true", help="don't open a browser tab")
    ap.add_argument("--dry-writes", action="store_true", help=argparse.SUPPRESS)  # testing
    a = ap.parse_args()

    repo_dir = os.path.abspath(os.path.expanduser(a.repo))
    if not os.path.exists(os.path.join(repo_dir, ".github", "scripts", "factory_tick.py")):
        sys.exit(f"{repo_dir} doesn't look like a minceraft checkout "
                 "(no .github/scripts/factory_tick.py). Pass --repo.")
    try:  # rate_limit is free and fails the same way any read would
        if subprocess.run(["gh", "api", "rate_limit"], capture_output=True).returncode:
            sys.exit("gh can't reach GitHub: run `gh auth login` first.")
    except FileNotFoundError:
        sys.exit("The GitHub CLI isn't installed: https://cli.github.com, then `gh auth login`.")

    state = State(repo_dir, a.github, max(a.every, 60))
    threading.Thread(target=state.loop, daemon=True).start()
    server = ThreadingHTTPServer(("127.0.0.1", a.port),
                                 make_handler(state, a.port, a.read_only, a.dry_writes))
    url = f"http://127.0.0.1:{a.port}/"
    print(f"Factory dashboard on {url}  (first read takes a minute; Ctrl-C to stop)", flush=True)
    if not a.no_open:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
