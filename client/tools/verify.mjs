// The play critic's verify pass (docs/design/play-critic-agent.md section 5): replays a
// finding's `repro` on a fresh debug-mode session and keeps it only if its `expect`s hold.
// Node only; never shipped to the browser.
import { parse, createSession } from '../test/dsl.mjs';

const no = (reason) => ({ reproduced: false, reason });

/** Replay `finding.repro` (`.input` text). Returns `{ reproduced, reason }`; never throws. */
export function verifyFinding(finding) {
  // A perf-pass finding is advisory: timing can't be replayed deterministically, so keep it as is.
  if (finding?.advisory === true) return { reproduced: true, reason: 'advisory finding, replay skipped' };
  let cases;
  try {
    cases = parse(String(finding?.repro ?? ''));
  } catch (e) {
    return no(`repro does not parse: ${e.message}`);
  }
  if (cases.length !== 1) return no(`repro must hold exactly one case, found ${cases.length}`);
  let steps = cases[0].steps;
  let startArgs = [];
  if (steps[0]?.op === 'start') {
    startArgs = steps[0].args;
    steps = steps.slice(1);
  }
  if (!steps.some((s) => s.op === 'expect')) return no('repro has no expect line');
  let session;
  try {
    session = createSession(startArgs.includes('debug') ? startArgs : [...(startArgs.length ? startArgs : ['walk']), 'debug']);
  } catch (e) {
    return no(`line ${cases[0].steps[0].line}: ${e.message}`);
  }
  try {
    for (const { line, op, args } of steps) {
      const r = session.execStep({ op, args });
      if (!r.ok) return no(`line ${line} (${op} ${args.join(' ')}): ${r.error}`);
    }
    return { reproduced: true, reason: 'every line replayed and every expect held' };
  } finally {
    session.close();
  }
}

/** Verify each finding: `{ kept, dropped: [{ finding, reason }] }`. */
export function verifyAll(findings) {
  const kept = [];
  const dropped = [];
  for (const finding of findings) {
    const { reproduced, reason } = verifyFinding(finding);
    if (reproduced) kept.push(finding);
    else dropped.push({ finding, reason });
  }
  return { kept, dropped };
}
