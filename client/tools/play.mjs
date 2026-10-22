// A player's tool: drives a createSession() world one JSON line at a time
// (docs/design/play-critic-agent.md section 1). Node only; never shipped to the browser.
//
// Requests, one JSON object per stdin line; each gets exactly one JSON line on stdout:
//   {"start": ["walk", "raft"]}   create the session (once); the words after `start` in a script.
//                                 Add "debug": true for verify mode (replaying repros, never playing):
//                                 debug paths and {"observe": "debug"} work. The default, player mode,
//                                 refuses them; the mode is fixed for the session.
//   {"exec": "press KeyW"}        run one DSL line; the response carries the player observation
//   {"observe": "player"}         the player observation, without doing anything
//   {"transcript": true}          the session so far as a valid `.input` script
// Responses: { ok, error?, observation?, transcript? }. Bad lines, failed `expect`s,
// malformed JSON and unknown requests are `ok: false` results; the process keeps going.
//
//   node client/tools/play.mjs        (needs ./build.sh first)

import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { createSession } from '../test/dsl.mjs';
import { PATHS, playerObservation, debugObservation } from './observe.mjs';

const fail = (error) => ({ ok: false, error });

/** The debug-classified path a DSL line reads (`expect`, `until`, `mark`, `expect event`), or null. */
function debugRead(line) {
  const [op, ...args] = line.replace(/#.*/, '').trim().split(/\s+/);
  if (op === 'mark') return 'mark';
  if (op === 'expect' && (args[0] === 'event' || (args[0] === 'no' && args[1] === 'event'))) return 'event';
  if (op === 'expect' || op === 'until') return PATHS[args[0]] === 'debug' ? args[0] : null;
  return null;
}

/** The state `handle` works on: holds the session once `start` has made it. */
export const newState = () => ({ session: null, debug: false });

/** Answer one parsed request. Never throws. `state` is a `newState()` object. */
export function handle(state, request) {
  try {
    if (request === null || typeof request !== 'object' || Array.isArray(request)) {
      return fail('request must be a JSON object');
    }
    if ('start' in request) {
      if (state.session) return fail('start is only allowed once');
      const args = request.start;
      if (!Array.isArray(args) || !args.every((a) => typeof a === 'string')) {
        return fail('start takes an array of strings');
      }
      if ('debug' in request && typeof request.debug !== 'boolean') return fail('debug takes true or false');
      state.session = createSession(args);
      state.debug = request.debug === true;
      return { ok: true, observation: playerObservation(state.session) };
    }
    const known = ['exec', 'observe', 'transcript'].find((k) => k in request);
    if (!known) return fail(`unknown request (expected start, exec, observe or transcript)`);
    if (!state.session) return fail('send start first');
    const s = state.session;
    if (known === 'exec') {
      if (typeof request.exec !== 'string') return fail('exec takes a DSL line (string)');
      if (!state.debug) {
        for (const line of request.exec.split('\n')) {
          const path = debugRead(line);
          if (path) return fail(`${path} is a debug path; it is not available in player mode`);
        }
      }
      const r = s.exec(request.exec);
      return { ...r, observation: playerObservation(s) };
    }
    if (known === 'observe') {
      if (request.observe === 'debug' && state.debug) return { ok: true, observation: debugObservation(s) };
      if (request.observe !== 'player') return fail(`unknown observation: ${JSON.stringify(request.observe)}`);
      return { ok: true, observation: playerObservation(s) };
    }
    return { ok: true, transcript: s.transcript() };
  } catch (e) {
    return fail(e.message);
  }
}

/** Handle one raw stdin line: parse it, then `handle`. Returns the response object. */
export function handleLine(state, text) {
  let request;
  try {
    request = JSON.parse(text);
  } catch (e) {
    return fail(`malformed JSON: ${e.message}`);
  }
  return handle(state, request);
}

/** Wire `handle` to stdin/stdout, one JSON line each way. */
export async function main(input = process.stdin, output = process.stdout) {
  const state = newState();
  try {
    for await (const text of createInterface({ input })) {
      if (!text.trim()) continue;
      output.write(`${JSON.stringify(handleLine(state, text))}\n`);
    }
  } finally {
    state.session?.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
