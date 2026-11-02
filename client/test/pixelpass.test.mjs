import test from 'node:test';
import assert from 'node:assert/strict';
import { buildReport, pageQuery, parseArgs } from '../tools/pixelpass.mjs';
import { MAX_CROPS } from '../pixelcheck.js';

const clean = { seams: 0, offPalette: 0, nearBlack: 0, crops: [] };

test('parseArgs defaults', () => {
  assert.deepEqual(parseArgs([], {}), { seed: '12345', pose: null, view: null, crops: null, chrome: 'google-chrome' });
  assert.equal(parseArgs([], { CHROME: '/x/chrome' }).chrome, '/x/chrome');
});

test('parseArgs takes seed, pose, view and --crops', () => {
  const a = parseArgs(['7', '1,2,3,0,-0.3', '320x200', '--crops', 'out']);
  assert.deepEqual([a.seed, a.pose, a.view, a.crops], ['7', '1,2,3,0,-0.3', '320x200', 'out']);
  assert.equal(pageQuery(a), 'client/index.html?seed=7&pixel=1&pose=1%2C2%2C3%2C0%2C-0.3&view=320x200');
});

test('parseArgs rejects bad arguments', () => {
  for (const argv of [[' '], ['1', 'x'], ['1', '1,2,3'], ['1', '1,2,3,0,0', '0x5'], ['1', '1,2,3,0,0', '5000x5'], ['1', 'a', 'b', 'c'], ['--crops'], ['--nope']]) {
    assert.ok(parseArgs(argv).error, argv.join(' '));
  }
});

test('buildReport for a clean result', () => {
  assert.equal(buildReport(clean, true), 'render.seams 0\noff-palette: 0\nnear-black: 0\ncrops: 0\nrepeatable: yes');
});

test('buildReport for a seamed result lists crops and repeatable: no', () => {
  const r = { seams: 5, offPalette: 3, nearBlack: 2, crops: [{ x: 1, y: 2, w: 3, h: 4 }] };
  const text = buildReport(r, false);
  assert.match(text, /^render\.seams 5\n/);
  assert.match(text, /off-palette: 3\nnear-black: 2\ncrops: 1\n {2}1 2 3 4\nrepeatable: no$/);
  assert.doesNotMatch(buildReport(r), /repeatable/);
});

test('buildReport caps the crop list at MAX_CROPS', () => {
  const crops = Array.from({ length: MAX_CROPS + 3 }, (_, i) => ({ x: i, y: 0, w: 1, h: 1 }));
  const lines = buildReport({ seams: 9, offPalette: 9, nearBlack: 0, crops }).split('\n');
  assert.equal(lines.filter((l) => l.startsWith('  ')).length, MAX_CROPS);
  assert.ok(lines.includes(`crops: ${MAX_CROPS}`));
});
