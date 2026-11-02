// The wasm Wind export matches the native game::wind_field::Wind (1e-9).
// Needs ./build.sh first. Run: node --test client/test/wind.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, Wind } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// [seed, x, z, ticks, dx, dz, strength], computed natively (roughness drag and gusts).
const NATIVE = [
  [12345, 0.3, 0.7, 0.0, 11.999685995736163, -0.08681015916018835, 1.0],
  [12345, 12.5, -4.2, 100.25, 11.754682965734945, -2.4140067056784855, 0.9999999999999999],
  [12345, -80.1, 33.3, 5000.75, 2.4963352737188287, 4.810521174851562, 0.45163901954520136],
  [12345, 1000.9, 250.4, 123456.5, -2.8308923824461205, 7.944749262410563, 0.7028364779569832],
  [7, 0.3, 0.7, 0.0, 0.09031252571453631, -11.999660147174962, 0.9999999999999999],
  [7, 12.5, -4.2, 100.25, 0.05281668132550016, -11.999883766027642, 0.9999999999999999],
  [7, -80.1, 33.3, 5000.75, -11.048408601361318, -4.68323257775708, 1.0],
  [7, 1000.9, 250.4, 123456.5, -10.398889115566545, -5.988581231155814, 1.0],
  [987654321, 0.3, 0.7, 0.0, -0.09655660473123817, -11.999611527965513, 1.0],
  [987654321, 12.5, -4.2, 100.25, 0.27483950346677627, -11.996852222451277, 1.0],
  [987654321, -80.1, 33.3, 5000.75, -2.0304957487344897, -5.090115461812848, 0.45668026439773196],
  [987654321, 1000.9, 250.4, 123456.5, -2.564929053490637, 7.783832718306296, 0.6829618544944916],
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
