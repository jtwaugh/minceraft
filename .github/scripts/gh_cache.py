#!/usr/bin/env python3
"""GitHub REST reads that cost nothing when nothing changed.

The factory tick re-reads the repo every run, and runs up to once a minute.
GitHub doesn't count a conditional request that comes back 304 Not Modified
against the rate limit, so this keeps each response's ETag and body in a JSON
file named by $FACTORY_HTTP_CACHE (factory-tick.yml keeps the file between
runs with actions/cache) and sends If-None-Match. Within one run the same path
is read from the network at most once, so the loader, `tickets.py` and
`project_board.py` can all ask for an issue's comments and pay once.

    begin()             start a run: forget this run's reads, load the file
    get(path)           one GET, cached
    get_all(path)       a list endpoint, every page of 100
    save()              write the file (does nothing without the variable)
    stats               {"requests": n, "not_modified": n} since begin()
    state               a dict the caller keeps between runs in the same file
                        (the tick's issue list and whether its last run was clean)

Without $FACTORY_HTTP_CACHE it still dedupes within the run, which is all a
one-off command needs.
"""
import json
import os
import re
import subprocess

_file = None
_etags = {}   # path -> [etag, body], kept between runs
_memo = {}    # path -> body, this run only
state = {}    # caller's own data, kept between runs
stats = {"requests": 0, "not_modified": 0}


def begin():
    """Start a run: forget this run's reads and (re)load the ETag file."""
    global _file, _etags
    _memo.clear()
    state.clear()
    stats.update(requests=0, not_modified=0)
    _file = os.environ.get("FACTORY_HTTP_CACHE") or None
    _etags = {}
    if _file and os.path.exists(_file):
        try:
            with open(_file, encoding="utf-8") as f:
                data = json.load(f)
        except (OSError, ValueError):
            data = {}  # a damaged cache only costs full reads
        if isinstance(data, dict) and "etags" in data:
            _etags, saved = data["etags"], data.get("state") or {}
            state.update(saved)
        elif isinstance(data, dict):
            _etags = data  # the first format: ETags only


def save():
    """Write the ETag file, keeping only what this run read (so it never grows
    with paths nobody asks for any more)."""
    if not _file:
        return
    keep = {p: v for p, v in _etags.items() if p in _memo}
    os.makedirs(os.path.dirname(os.path.abspath(_file)), exist_ok=True)
    tmp = _file + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump({"etags": keep, "state": state}, f)
    os.replace(tmp, _file)


def get(path, env=None):
    """GET one REST path. A 304 returns the cached body."""
    if path in _memo:
        return _memo[path]
    cached = _etags.get(path)
    cmd = ["gh", "api", "-i", "--method", "GET", path]
    if cached:
        cmd += ["-H", f"If-None-Match: {cached[0]}"]
    kw = {"env": env} if env else {}
    p = subprocess.run(cmd, capture_output=True, text=True, **kw)
    stats["requests"] += 1
    # text=True has already turned the \r\n line ends into \n
    head, _, body = (p.stdout or "").replace("\r\n", "\n").partition("\n\n")
    status = head.split("\n", 1)[0].split()
    if cached and len(status) > 1 and status[1] == "304":
        stats["not_modified"] += 1
        data = cached[1]
    else:
        if p.returncode:
            raise RuntimeError(f"gh api {path}: {(p.stderr or '').strip()}")
        data = json.loads(body)
        etag = next((h.split(":", 1)[1].strip() for h in head.split("\n")
                     if h.lower().startswith("etag:")), None)
        if etag:
            _etags[path] = [etag, data]
    _memo[path] = data
    return data


def get_all(path, env=None):
    """Every page of a list endpoint. Page 1 is requested as `path?per_page=100`
    (no page number), the same key `get` uses for that path, so the two share
    a cached read."""
    base = re.sub(r"([?&])(per_page|page)=\d+&?", r"\1", path).rstrip("?&")
    sep = "&" if "?" in base else "?"
    out, page = [], 1
    while True:
        url = f"{base}{sep}per_page=100" + (f"&page={page}" if page > 1 else "")
        chunk = get(url, env)
        out += chunk
        if len(chunk) < 100:
            return out
        page += 1
