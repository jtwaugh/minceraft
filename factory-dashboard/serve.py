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
import subprocess
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
                "busy": self.busy, "every": self.every, "pending": n}


def gh_write(args, dry):
    if dry:
        print("[dry write] gh " + " ".join(args), flush=True)
        return True, "dry run: nothing sent"
    p = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60)
    return p.returncode == 0, (p.stderr or p.stdout).strip()[:500]


def act(state, body, dry):
    """One write. Returns (ok, message); raises ValueError on a bad request."""
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
