// Run: node --test client/test/bobbermodel.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { bobberBoxes, bobberCentre, bobberDraw, lineMatrix, DIP_DEPTH, LINE_THICKNESS } from '../bobbermodel.js';

const at = (m, p) => [0, 1, 2].map((i) => m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i]);
const near = (a, b) => a.forEach((v, i) => assert.ok(Math.abs(v - b[i]) < 1e-5, `${a} vs ${b}`));

test('no bobber draws nothing', () => {
  assert.equal(bobberDraw(null, [0, 0, 0]), null);
});

test('a dipping bobber sits lower than a floating one, by DIP_DEPTH', () => {
  const float = bobberCentre({ state: 'floating', dipping: false, pos: [1, 64, 2] });
  const dip = bobberCentre({ state: 'floating', dipping: true, pos: [1, 64, 2] });
  assert.deepEqual(float, [1, 64, 2]);
  near(dip, [1, 64 - DIP_DEPTH, 2]);
});

test('the bobber is moved to its centre and uses the bobber tile', () => {
  const d = bobberDraw({ state: 'flying', dipping: false, pos: [3, 70, 4] }, [0, 0, 0]);
  near(at(d.bobber, [0, 0, 0]), [3, 70, 4]);
  for (const f of ['front', 'back', 'left', 'right', 'top', 'bottom']) assert.equal(bobberBoxes()[0].faces[f], 'bobber');
});

test('the line runs from the rod tip to the drawn bobber centre', () => {
  const tip = [0, 65, 0];
  const d = bobberDraw({ state: 'floating', dipping: true, pos: [4, 63, 3] }, tip);
  near(at(d.line, [0, 0, 0]), tip);
  near(at(d.line, [0, 0, 1]), bobberCentre({ dipping: true, pos: [4, 63, 3] }));
});

test('the line is thin and handles straight-down and zero-length segments', () => {
  const m = lineMatrix([0, 10, 0], [0, 5, 0]);
  const side = at(m, [0.5, 0, 0]);
  assert.ok(Math.abs(Math.hypot(side[0], side[2]) - LINE_THICKNESS / 2) < 1e-5);
  near(at(m, [0, 0, 1]), [0, 5, 0]);
  assert.equal(lineMatrix([1, 2, 3], [1, 2, 3]), null);
});
