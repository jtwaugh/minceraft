# Factory dashboard

A local page for the Mince Raft software factory, in place of a GitHub
project tab. It runs on your machine, reads GitHub through your `gh` login,
and opens in your browser at http://127.0.0.1:8765.

## Run it

```sh
gh auth status                                  # must be signed in
python3 serve.py --repo ~/path/to/minceraft     # opens the page
```

Keep the minceraft checkout reasonably up to date (`git pull`). The
dashboard reuses its `.github/scripts`, so it judges state exactly the way
the factory tick does, and a pull keeps the two in step.

Options:

| Flag | Default | |
|---|---|---|
| `--repo PATH` | `.` | your minceraft checkout |
| `--port N` | 8765 | |
| `--every SECONDS` | 300 | how often it re-reads GitHub (minimum 60) |
| `--read-only` | off | hides and refuses every write |
| `--no-open` | off | don't open a browser tab |
| `--github OWNER/NAME` | jtwaugh/minceraft | |

Needs Python 3.9+ and the GitHub CLI, with nothing to install.

## What it shows

- **Banners:** the tick failing, stale or disabled (with the error from the
  first failed run), the self-hosted runner dropping agent runs, a tick pass
  crashing.
- **Overview:** last tick, items waiting on you, active and failed agent
  runs, open and blocked PRs, ticks in the last 48 h.
- **Waiting on you:** the needs-you queue, by the needs-you pass's own rules,
  most-blocking first. The tick's own tracker issues (`Factory: stalled work`) show
  apart as reports and don't count as waiting.
- **Started, no PR yet:** each started ticket's state and its start count
  (n of 3).
- **Blocked PRs** and what Claude is doing about each.
- **Open PRs** with CI and reviewer state.
- **Recent agent runs** (implementer, fix, review), with the error and runner
  for the newest failures.
- **Next tick:** a dry run of every tick pass, listing what it would do.
  Nothing is written.

## Buttons

Every write opens a dialog showing exactly what will be posted, and goes
out as you through `gh`:

- **No-PR question:** *Retry #N with @claude* comments on the ticket (which
  starts the implementer) and closes the question. *Build it myself* labels
  the ticket `local` and closes the question.
- **Spec question:** one `/decide X` button per option.
- **Unapproved ticket:** *Approve* adds `factory`; *Close* closes it as not
  planned.
- **PR waiting on review:** *Review the diff* opens the PR's files on
  GitHub; *Ask @claude to review* comments asking Claude for a review;
  *Close PR* closes it unmerged. Merging stays on GitHub.
- **Comment** on any waiting issue.

Closing a no-PR question by itself doesn't restart its ticket; use Retry.

A close or label shows on the page as soon as GitHub accepts it. The server
holds the change over the last read until a read begun after it agrees, or
for 15 minutes, whichever comes first. The header counts changes not yet
confirmed. Restarting the server forgets them.

## Cost

- **RAM:** the server idles at about 20 MB. Each refresh runs a separate
  reader process (about 90 MB at peak, for about a minute) that exits when
  done. The page is plain HTML and JS with no framework.
- **API:** about 165 reads per refresh, mostly conditional requests that
  come back 304 and don't count against the limit. ETags are kept in
  `~/.cache/minceraft-dashboard/http.json`.

## Safety

The server listens on 127.0.0.1 only and checks the Host header. Writes
need a token that changes every launch, and it only allows comments,
closing issues, and the `factory`, `local` and `hold` labels. Don't expose
the port or run it on a shared machine: anything that can reach the port
and read the page can post as you.
