// The critic's prompt (.github/prompts/critic.md) must not hand the critic the controls
// or debug-only state, and must carry the clean-room rules and the finding schema
// (docs/design/play-critic-agent.md section 4, reconciliation notes 13 and 16).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PATHS } from '../tools/observe.mjs';

const prompt = readFileSync(new URL('../../.github/prompts/critic.md', import.meta.url), 'utf8');
const has = (word) => new RegExp(`(?<![\\w.])${word.replace(/\./g, '\\.')}(?![\\w])`).test(prompt);

test('prompt names no key or control', () => {
  const keys = /\b(Key[A-Z]|Digit\d|Backquote|Escape|Enter|Space|Arrow\w+|Shift\w*|Control\w*|Mouse\d?|Tab|F\d{1,2})\b/;
  assert.equal(prompt.match(keys), null, `key name: ${prompt.match(keys)?.[0]}`);
});

test('prompt names no debug-only path', () => {
  const debug = Object.keys(PATHS).filter((p) => PATHS[p] === 'debug');
  assert.ok(debug.length > 0);
  assert.deepEqual(debug.filter(has), []);
});

test('prompt states the clean-room rules', () => {
  for (const s of ['Minecraft', 'SPEC.md', 'work/', '*.jar', 'Never edit code']) {
    assert.ok(prompt.includes(s), s);
  }
});

test('prompt gives the finding schema fields and layout sweep', () => {
  for (const s of ['kind', 'severity', 'goal', 'observation', 'suggestion', 'repro', 'control-flow', 'layout', 'perf', 'render', 'low', 'medium', 'high']) {
    assert.ok(has(s) || prompt.includes(`\`${s}\``), s);
  }
  for (const s of ['1366', '1920', 'ultrawide', 'portrait', 'advisory', 'by design']) {
    assert.ok(prompt.includes(s), s);
  }
});

test('prompt says a repro needs an expect line that holds', () => {
  assert.match(prompt, /at least one `expect` line/);
  assert.match(prompt, /without one is always dropped/);
});

test('prompt lists perf and render in the intro and kind line, and the streamed-world goal', () => {
  assert.match(prompt, /control flow, layout, performance and rendering/);
  assert.match(prompt, /- `kind`: `control-flow`, `layout`, `perf` or `render`/);
  assert.match(prompt, /region borders in a streamed\s+world/);
  assert.ok(prompt.includes('start walk stream'));
});
