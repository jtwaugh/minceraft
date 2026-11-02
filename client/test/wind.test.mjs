// The wasm Wind export matches the native game::wind_field::Wind (1e-9).
// Needs ./build.sh first. Run: node --test client/test/wind.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, Wind } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// [seed, x, z, ticks, dx, dz, strength], computed natively.
const NATIVE = [
  [12345, 0.3, 0.7, 0, 11.999999999986606, 1.7928825916377866e-5, 1.0],
  [12345, 12.5, -4.2, 100.25, 11.999997617104894, 0.007562372437640152, 1.0],
  [12345, -80.1, 33.3, 5000.75, 2.406309422544332, 3.8192183375476034, 0.37617181493749857],
  [12345, 1000.9, 250.4, 123456.5, -2.2415682042362164, 9.12105473029883, 0.7827048338904534],
  [7, 0.3, 0.7, 0, -1.7928774992355546e-5, -11.999999999986606, 1.0],
  [7, 12.5, -4.2, 100.25, -0.007558055509921686, -11.999997619824635, 1.0],
  [7, -80.1, 33.3, 5000.75, -10.63338315189925, -5.561579159277084, 1.0],
  [7, 1000.9, 250.4, 123456.5, -10.802246976374647, -5.226036764260734, 1.0],
  [987654321, 0.3, 0.7, 0, -1.7928774992355546e-5, -11.999999999986606, 1.0],
  [987654321, 12.5, -4.2, 100.25, -0.007558055509921686, -11.999997619824635, 1.0],
  [987654321, -80.1, 33.3, 5000.75, -2.2518398594593236, -4.588287302138198, 0.4259235318661185],
  [987654321, 1000.9, 250.4, 123456.5, -2.4878703119289156, 8.387591094218848, 0.729065150327595],
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
