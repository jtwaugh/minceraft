// The draft scheduled critic workflow: triggers, permissions, command. Run: node --test client/test/criticworkflow.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const text = readFileSync(new URL('../../.github/workflow-drafts/claude-critic.yml', import.meta.url), 'utf8');
const code = text.replace(/^\s*#.*$/gm, ''); // comments may mention labels and pushes

test('runs weekly, on demand, and on client/ pushes to master', () => {
  assert.match(code, /^on:\n {2}schedule:\n {4}- cron: '[^']+'/m);
  assert.match(code, /^ {2}workflow_dispatch:/m);
  assert.match(code, /^ {2}push:\n {4}branches: \[master\]\n {4}paths:\n {6}- 'client\/\*\*'/m);
});

test('grants only contents: read and issues: write', () => {
  const block = code.match(/^permissions:\n((?: {2}.+\n)+)/m);
  assert.ok(block, 'top-level permissions block');
  assert.deepEqual(block[1].trim().split('\n').map((l) => l.trim()).sort(), ['contents: read', 'issues: write']);
  assert.doesNotMatch(code, /^[ \t]+permissions:/m, 'no job-level override');
});

test('builds, then runs the critic with --file; serialised runs', () => {
  const build = code.indexOf('run: ./build.sh');
  const run = code.indexOf('run: node client/tools/critic.mjs --file');
  assert.ok(build > 0 && run > build);
  assert.match(code, /GH_TOKEN: \$\{\{ github\.token \}\}/);
  assert.match(code, /CLAUDE_CODE_OAUTH_TOKEN: \$\{\{ secrets\.CLAUDE_CODE_OAUTH_TOKEN \}\}/);
  assert.match(code, /^concurrency:\n {2}group: \S+/m);
});

test('never labels, commits or pushes', () => {
  assert.doesNotMatch(code, /--add-label|gh issue edit|git (commit|push|add)|\bidea\b/);
});
