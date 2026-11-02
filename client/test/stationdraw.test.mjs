import test from 'node:test';
import assert from 'node:assert/strict';
import { stationPose, stationDraws, readStations, mulMat4, STATION_ITEM_SIZE } from '../itemmodel.js';
import { raftMatrix } from '../raftmodel.js';

const centre = (m) => [m[12], m[13], m[14]];
const IDENTITY = raftMatrix([0, 0, 0], 0, [0, 0], 0);

test('station pose sits at the top-face centre of its cell, raft-local', () => {
  for (const cell of [[3, 5, 2], [0, 4, 7]]) {
    const m = stationPose(cell);
    const c = centre(m);
    assert.deepEqual([c[0], c[2]], [cell[0] + 0.5, cell[2] + 0.5]);
    // The item's bottom edge is the cell's top face.
    assert.ok(Math.abs(c[1] - STATION_ITEM_SIZE / 2 - (cell[1] + 1)) < 1e-6);
  }
});

test('station draws carry the raft transform', () => {
  const stations = [{ cell: [3, 5, 2], name: 'raw_beef' }, { cell: [0, 4, 7], name: 'wheat' }];
  const plain = stationDraws(stations, IDENTITY);
  assert.deepEqual(plain.map((d) => d.key), ['raw_beef', 'wheat']);
  assert.deepEqual(centre(plain[0].model), centre(stationPose([3, 5, 2])));
  const raft = raftMatrix([10, 1, -4], Math.PI / 2, [8, 8], 0);
  const moved = stationDraws(stations, raft);
  // Same point as the raft's own transform of the raft-local centre.
  const p = centre(stationPose([0, 4, 7]));
  const want = [0, 1, 2].map((r) => raft[r] * p[0] + raft[4 + r] * p[1] + raft[8 + r] * p[2] + raft[12 + r]);
  centre(moved[1].model).forEach((v, i) => assert.ok(Math.abs(v - want[i]) < 1e-4));
  assert.deepEqual(Array.from(moved[0].model), Array.from(mulMat4(raft, stationPose([3, 5, 2]))));
});

test('no stations, no instances; a removed item drops out next frame', () => {
  assert.deepEqual(stationDraws([], IDENTITY), []);
  let cells = [1, 4, 1], names = ['wheat'];
  const walk = { station_cells: () => Int32Array.from(cells), station_item_names: () => names };
  assert.equal(stationDraws(readStations(walk), IDENTITY).length, 1);
  cells = []; names = [];
  assert.equal(stationDraws(readStations(walk), IDENTITY).length, 0);
});
