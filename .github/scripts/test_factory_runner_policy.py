"""Policy for vars.FACTORY_RUNNER. python3 -I test_factory_runner_policy.py (no network).

The variable can point agent jobs at a self-hosted runner, which keeps state
between runs. Untrusted code must never reach one, so a job that uses it must:

- not be in ci.yml, or in a workflow triggered by pull_request or
  pull_request_target (they run fork code);
- have an `if` whose first top-level `&&` term limits the run to the
  maintainer (or the tick's github-actions[bot]), with no top-level `||`
  that could bypass it;
- default to ubuntu-latest when the variable is unset.

The tick's choice arrives as the `runner` dispatch input (factory_runner.py).
A job whose runs-on takes it must also be reachable only by workflow_dispatch
(the only event with inputs), through a top-level `&&` term or a workflow
with no other trigger.

Only the three agent jobs named in EXPECTED may use the variable, and only
the two the tick dispatches (DISPATCHED) the input. The tick reads the
variable as data (READERS) and stays on ubuntu-latest. The workflows are
read with a small indentation parser, so this needs no PyYAML.
"""
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
WORKFLOWS = os.path.join(HERE, "..", "workflows")
VAR = "vars.FACTORY_RUNNER"
RUNS_ON = "${{ fromJSON(vars.FACTORY_RUNNER || '\"ubuntu-latest\"') }}"
INPUT = "inputs.runner"
RUNS_ON_INPUT = "${{ fromJSON(inputs.runner || vars.FACTORY_RUNNER || '\"ubuntu-latest\"') }}"
DISPATCH = "github.event_name == 'workflow_dispatch'"
EXPECTED = {("claude.yml", "implement"), ("claude.yml", "start"), ("claude-fix.yml", "fix")}
DISPATCHED = {("claude.yml", "start"), ("claude-fix.yml", "fix")}
READERS = {("factory-tick.yml", "tick")}
MAINTAINERS = {"jtwaugh", "github-actions[bot]"}
FORK_EVENTS = {"pull_request", "pull_request_target"}


def strip_comment(line):
    # Good enough for workflow files: a '#' after whitespace outside quotes.
    out, q = [], None
    for i, c in enumerate(line):
        if q:
            q = None if c == q else q
        elif c in "'\"":
            q = c
        elif c == "#" and (i == 0 or line[i - 1].isspace()):
            break
        out.append(c)
    return "".join(out).rstrip()


def indent(line):
    return len(line) - len(line.lstrip(" "))


def block(lines, start):
    """Lines more indented than lines[start], up to the next sibling."""
    base, out = indent(lines[start]), []
    for ln in lines[start + 1:]:
        if ln.strip() and indent(ln) <= base:
            break
        out.append(ln)
    return out


def key_value(line):
    m = re.match(r"^\s*([A-Za-z0-9_-]+):(?:\s+(.*))?$", line)
    return (m.group(1), (m.group(2) or "").strip()) if m else (None, None)


def children(lines):
    """(key, inline value, body lines) for the least-indented keys."""
    lines = [ln for ln in lines if ln.strip()]
    if not lines:
        return []
    base = min(indent(ln) for ln in lines)
    out = []
    for i, ln in enumerate(lines):
        if indent(ln) == base:
            k, v = key_value(ln)
            if k:
                out.append((k, v, block(lines, i)))
    return out


def parse(text):
    """{"on": set of events, "jobs": {name: {"if": str, "runs-on": str, "text": str}}}"""
    lines = [strip_comment(ln) for ln in text.splitlines()]
    top = {k: (v, body) for k, v, body in children(lines)}
    events = set()
    if "on" in top:
        v, body = top["on"]
        if v:
            events = {e.strip() for e in v.strip("[]").split(",") if e.strip()}
        else:
            events = {k for k, _, _ in children(body)}
    jobs = {}
    for name, _, body in children(top.get("jobs", ("", []))[1]):
        fields = {k: (v, b) for k, v, b in children(body)}
        cond = ""
        if "if" in fields:
            v, b = fields["if"]
            cond = " ".join(ln.strip() for ln in b) if v in ("|", ">", "|-", ">-") else v
        jobs[name] = {"if": cond, "runs-on": fields.get("runs-on", ("", []))[0],
                      "text": "\n".join(body)}
    return {"on": events, "jobs": jobs}


def top_level_split(expr, op):
    parts, depth, q, cur, i = [], 0, None, "", 0
    while i < len(expr):
        c = expr[i]
        if q:
            q = None if c == q else q
        elif c in "'\"":
            q = c
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        elif depth == 0 and expr.startswith(op, i):
            parts.append(cur.strip())
            cur, i = "", i + len(op)
            continue
        cur += c
        i += 1
    parts.append(cur.strip())
    return parts


def unwrap(cond):
    cond = cond.strip()
    if cond.startswith("${{") and cond.endswith("}}"):
        cond = cond[3:-2].strip()
    return cond


def dispatch_only(on, cond):
    """Only workflow_dispatch reaches the job (maintainer_only has ruled out a top-level ||)."""
    return on == {"workflow_dispatch"} or DISPATCH in top_level_split(unwrap(cond), "&&")


def maintainer_only(cond):
    cond = unwrap(cond)
    if not cond or len(top_level_split(cond, "||")) > 1:
        return False
    first = top_level_split(cond, "&&")[0]
    m = re.fullmatch(r"(github\.actor|github\.event\.sender\.login|github\.triggering_actor)"
                     r"\s*==\s*'([^']+)'", first)
    if m:
        return m.group(2) in MAINTAINERS
    m = re.fullmatch(r"contains\(\s*fromJSON\(\s*'\[(.*)\]'\s*\)\s*,\s*"
                     r"(github\.actor|github\.event\.sender\.login|github\.triggering_actor)\s*\)", first)
    if m:
        logins = {s.strip().strip('"') for s in m.group(1).split(",")}
        return bool(logins) and logins <= MAINTAINERS
    return False


def violations(name, wf):
    bad = []
    for job, j in wf["jobs"].items():
        if VAR not in j["text"] and INPUT not in j["text"]:
            continue
        where = f"{name}: job {job}"
        if (name, job) in READERS:
            if VAR in j["runs-on"] or INPUT in j["text"]:
                bad.append(f"{where}: only reads {VAR}; it must not run on it")
            continue
        if name == "ci.yml":
            bad.append(f"{where}: ci.yml must not use {VAR}")
        forks = wf["on"] & FORK_EVENTS
        if forks:
            bad.append(f"{where}: workflow runs on {', '.join(sorted(forks))}, which runs fork code")
        if not maintainer_only(j["if"]):
            bad.append(f"{where}: needs an `if` whose first && term limits it to {sorted(MAINTAINERS)}")
        if j["runs-on"] == RUNS_ON_INPUT:
            if not dispatch_only(wf["on"], j["if"]):
                bad.append(f"{where}: runs-on takes {INPUT}, so the `if` needs a top-level {DISPATCH}")
        elif INPUT in j["text"]:
            bad.append(f"{where}: uses {INPUT} without runs-on {RUNS_ON_INPUT}")
        elif j["runs-on"] != RUNS_ON:
            bad.append(f"{where}: runs-on must be {RUNS_ON} so unset means ubuntu-latest")
    return bad


def repo_workflows():
    out = {}
    for f in sorted(os.listdir(WORKFLOWS)):
        if f.endswith((".yml", ".yaml")):
            with open(os.path.join(WORKFLOWS, f), encoding="utf-8") as fh:
                out[f] = parse(fh.read())
    return out


GOOD = """
on:
  workflow_dispatch:
jobs:
  work:
    if: contains(fromJSON('["jtwaugh", "github-actions[bot]"]'), github.actor)
    runs-on: ${{ fromJSON(vars.FACTORY_RUNNER || '"ubuntu-latest"') }}
    steps: []
"""


class Repo(unittest.TestCase):
    def test_repo_follows_policy(self):
        bad = [b for n, wf in repo_workflows().items() for b in violations(n, wf)]
        self.assertEqual(bad, [])

    def test_only_the_agent_jobs_use_it(self):
        users = {(n, j) for n, wf in repo_workflows().items()
                 for j, job in wf["jobs"].items() if VAR in job["text"]}
        self.assertEqual(users, EXPECTED | READERS)

    def test_only_the_dispatched_jobs_take_the_input(self):
        wfs = repo_workflows()
        users = {(n, j) for n, wf in wfs.items()
                 for j, job in wf["jobs"].items() if INPUT in job["text"]}
        self.assertEqual(users, DISPATCHED)
        for n, j in DISPATCHED:
            self.assertEqual(wfs[n]["jobs"][j]["runs-on"], RUNS_ON_INPUT)

    def test_other_jobs_keep_github_hosted(self):
        for n, wf in repo_workflows().items():
            for j, job in wf["jobs"].items():
                if (n, j) not in EXPECTED:
                    self.assertNotIn(VAR, job["runs-on"], f"{n}: {j}")
                    self.assertNotIn(INPUT, job["runs-on"], f"{n}: {j}")

    def test_parser_reads_the_real_jobs(self):
        wf = repo_workflows()["claude.yml"]
        self.assertIn("workflow_dispatch", wf["on"])
        self.assertTrue(wf["jobs"]["implement"]["if"].startswith("github.event.sender.login == 'jtwaugh' &&"))
        self.assertIn("pull_request_review_comment", wf["on"])


class Fixtures(unittest.TestCase):
    def check(self, text, name="x.yml"):
        return violations(name, parse(text))

    def test_good_fixture_passes(self):
        self.assertEqual(self.check(GOOD), [])

    def test_ci_fails(self):
        self.assertTrue(any("ci.yml" in b for b in self.check(GOOD, "ci.yml")))

    def test_pull_request_triggers_fail(self):
        for ev in ("pull_request", "pull_request_target"):
            bad = self.check(GOOD.replace("workflow_dispatch:", f"{ev}:"))
            self.assertTrue(any("fork code" in b for b in bad), ev)
        self.assertTrue(self.check(GOOD.replace("on:\n  workflow_dispatch:", "on: [push, pull_request]")))

    def test_missing_if_fails(self):
        bad = self.check(GOOD.replace("    if: contains(fromJSON('[\"jtwaugh\", \"github-actions[bot]\"]'), github.actor)\n", ""))
        self.assertTrue(any("needs an `if`" in b for b in bad))

    def test_weak_ifs_fail(self):
        for cond in ["github.event_name == 'workflow_dispatch'",
                     "github.actor == 'someone'",
                     "contains(fromJSON('[\"jtwaugh\", \"someone\"]'), github.actor)",
                     "github.actor == 'jtwaugh' || github.event_name == 'issue_comment'",
                     "github.event_name == 'issues' && github.actor == 'jtwaugh'",
                     "contains(github.event.comment.body, 'jtwaugh')"]:
            text = GOOD.replace("contains(fromJSON('[\"jtwaugh\", \"github-actions[bot]\"]'), github.actor)", cond)
            self.assertTrue(self.check(text), cond)

    def test_strong_ifs_pass(self):
        for cond in ["github.event.sender.login == 'jtwaugh' && (github.event_name == 'issues' || x)",
                     "${{ github.actor == 'jtwaugh' }}",
                     "contains(fromJSON('[\"jtwaugh\"]'), github.actor) && github.event_name == 'x'"]:
            text = GOOD.replace("contains(fromJSON('[\"jtwaugh\", \"github-actions[bot]\"]'), github.actor)", cond)
            self.assertEqual(self.check(text), [], cond)

    def test_block_scalar_if(self):
        text = GOOD.replace("    if: contains(fromJSON('[\"jtwaugh\", \"github-actions[bot]\"]'), github.actor)\n",
                            "    if: |\n      github.actor == 'jtwaugh' && (\n        a || b\n      )\n")
        self.assertEqual(self.check(text), [])

    def test_input_on_a_dispatch_only_workflow_passes(self):
        self.assertEqual(self.check(GOOD.replace(RUNS_ON, RUNS_ON_INPUT)), [])

    def test_input_reachable_by_other_events_fails(self):
        text = GOOD.replace(RUNS_ON, RUNS_ON_INPUT).replace("  workflow_dispatch:", "  workflow_dispatch:\n  issues:")
        self.assertTrue(any("top-level" in b for b in self.check(text)))
        actors = "contains(fromJSON('[\"jtwaugh\", \"github-actions[bot]\"]'), github.actor)"
        for cond in [f"github.actor == 'jtwaugh' && ({DISPATCH} || x)",
                     f"github.actor == 'jtwaugh' && !({DISPATCH})"]:
            bad = self.check(text.replace(actors, cond))
            self.assertTrue(any("top-level" in b for b in bad), cond)
        self.assertEqual(self.check(text.replace(actors, f"{actors} && {DISPATCH}")), [])

    def test_input_with_another_runs_on_fails(self):
        text = GOOD.replace("    steps: []", "    env:\n      X: ${{ inputs.runner }}\n    steps: []")
        self.assertTrue(any("without runs-on" in b for b in self.check(text)))

    def test_reader_must_not_run_on_it(self):
        reader = GOOD.replace("  work:", "  tick:")
        bad = self.check(reader, "factory-tick.yml")
        self.assertTrue(any("must not run on it" in b for b in bad))
        hosted = reader.replace(RUNS_ON, "ubuntu-latest").replace(
            "    steps: []", "    env:\n      R: ${{ vars.FACTORY_RUNNER }}\n    steps: []")
        self.assertEqual(self.check(hosted, "factory-tick.yml"), [])

    def test_bare_variable_fails(self):
        bad = self.check(GOOD.replace(RUNS_ON, "${{ vars.FACTORY_RUNNER }}"))
        self.assertTrue(any("runs-on must be" in b for b in bad))


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False).result.wasSuccessful() else 1)
