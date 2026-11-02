// The scheduled perf-pass workflow: triggers, permissions, command. Run: node --test client/test/perfpassworkflow.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const text = readFileSync(new URL('../../.github/workflows/claude-perfpass.yml', import.meta.url), 'utf8');
const code = text.replace(/^\s*#.*$/gm, ''); // comments may mention secrets and pushes

test('runs on a schedule and on demand, not on pushes', () => {
  assert.match(code, /^on:\n {2}schedule:\n {4}- cron: '[^']+'/m);
  assert.match(code, /^ {2}workflow_dispatch:/m);
  assert.doesNotMatch(code, /^ {2}(push|pull_request):/m);
});

test('grants only contents: read and uses no secrets but the default token', () => {
  const block = code.match(/^permissions:\n((?: {2}.+\n)+)/m);
  assert.ok(block, 'top-level permissions block');
  assert.deepEqual(block[1].trim().split('\n').map((l) => l.trim()), ['contents: read']);
  assert.doesNotMatch(code, /^[ \t]+permissions:/m, 'no job-level override');
  assert.doesNotMatch(code, /secrets\./);
});

test('builds, installs Chrome, then runs perfpass.mjs with CHROME set; serialised runs', () => {
  const build = code.indexOf('run: ./build.sh');
  const npm = code.indexOf('run: npm install --prefix client');
  const chrome = code.indexOf('browser-actions/setup-chrome');
  const run = code.indexOf('run: node client/tools/perfpass.mjs 12345');
  assert.ok(build > 0 && npm > build && chrome > npm && run > chrome);
  assert.match(code, /CHROME: \$\{\{ steps\.chrome\.outputs\.chrome-path \}\}/);
  assert.match(code, /^concurrency:\n {2}group: \S+/m);
});

test('never labels, commits or pushes', () => {
  assert.doesNotMatch(code, /--add-label|gh issue|git (commit|push|add)/);
});

test('toolchain versions match ci.yml', () => {
  const ci = readFileSync(new URL('../../.github/workflows/ci.yml', import.meta.url), 'utf8');
  for (const key of ['RUST_TOOLCHAIN', 'BINARYEN_VERSION']) {
    const re = new RegExp(`^ +${key}: (\\S+)`, 'm');
    assert.equal(code.match(re)[1], ci.match(re)[1]);
  }
});
