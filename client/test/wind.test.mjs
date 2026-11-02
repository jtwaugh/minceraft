// The wasm Wind export matches the native game::wind_field::Wind (1e-9).
// Needs ./build.sh first. Run: node --test client/test/wind.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, Wind } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// [seed, x, z, ticks, dx, dz, strength], computed natively (with roughness drag).
const NATIVE = [
  [12345, 0.3, 0.7, 0, 11.999999999986608, 1.792882591637787e-5, 1.0],
  [12345, 12.5, -4.2, 100.25, 11.999997617104894, 0.007562372437640152, 1.0],
  [12345, -80.1, 33.3, 5000.75, 2.276261757575475, 3.612810789893513, 0.3558418167662773],
  [12345, 1000.9, 250.4, 123456.5, -1.4935627913549656, 6.077382761470932, 0.5215182898754479],
  [7, 0.3, 0.7, 0, -1.792877499235555e-5, -11.999999999986606, 1.0],
  [7, 12.5, -4.2, 100.25, -0.007558055509921686, -11.999997619824637, 1.0],
  [7, -80.1, 33.3, 5000.75, -10.633383151899247, -5.561579159277082, 1.0],
  [7, 1000.9, 250.4, 123456.5, -10.802246976374647, -5.2260367642607335, 1.0],
  [987654321, 0.3, 0.7, 0, -1.792877499235555e-5, -11.999999999986608, 1.0],
  [987654321, 12.5, -4.2, 100.25, -0.007558055509921687, -11.999997619824635, 1.0],
  [987654321, -80.1, 33.3, 5000.75, -1.944967518400649, -3.9630126140016224, 0.36788026081081465],
  [987654321, 1000.9, 250.4, 123456.5, -2.1549223960024095, 7.265092481218927, 0.6314954654399555],
];

const near = (a, b, what) => assert.ok(Math.abs(a - b) <= 1e-9, `${what}: ${a} vs ${b}`);

test('wasm wind matches native values', () => {
  for (const [seed, x, z, t, dx, dz, s] of NATIVE) {
    const w = new Wind(BigInt(seed));
    const [gx, gz] = w.at(x, z, t);
    near(gx, dx, `dx seed ${seed} at ${x},${z},${t}`);
    near(gz, dz, `dz seed ${seed} at ${x},${z},${t}`);
    near(w.strength(gx, gz), s, `strength seed ${seed}`);
  }
});

test('direction is a unit vector, zero for zero wind', () => {
  const [x, z] = Wind.direction(3, 4);
  near(x, 0.6, 'x');
  near(z, 0.8, 'z');
  assert.deepEqual([...Wind.direction(0, 0)], [0, 0]);
});
