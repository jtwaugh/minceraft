// Short-lived particles, apart from WebGL so they run in Node. The controller
// spawns a burst on each `forage_missed` event (docs/design/picking-plants.md
// note 11), snow flakes off swaying snowy foliage, and ages the list every
// frame; scene.js draws what `list` holds.
//
//   const p = createParticles();
//   p.spawnMisses(events)  burst at the position of each `forage_missed position=x,y,z` event
//   p.spawnSnow(cells, eye, phase, dt)
//                          flakes off the snow layers at world `cells` ([x, y, z] of each, from
//                          snowOnFoliage) near `eye`, while the wind at sway `phase` pushes them
//   p.update(dt)           age by dt seconds, drop the expired
//   p.list                 live particles: { pos: [x, y, z], vel, age, life, size, snow }
//   p.count                how many are live

import { B, H, swaysUnder } from '../viewer/mesh.js';
import { swayOffset } from './sway.js';

const BURST = 8;
const LIFE = 0.6; // seconds
const RISE = 0.8; // blocks per second
const SPREAD = 0.35; // blocks, around the cell's centre
const SIZE = 0.06;

const SNOW_RADIUS = 24; // blocks from the eye; farther snow drops nothing
const SNOW_RATE = 0.5; // flakes per second from one cell at the peak of a gust
const SNOW_FALL = 1.1; // blocks per second
const SNOW_DRIFT = 0.25; // blocks per second, the way the foliage leans
const SNOW_LIFE = 1.6; // seconds
const SNOW_SIZE = 0.025;
const SNOW_MAX = 256; // live flakes at most

/**
 * World [x, y, z] of every snow layer resting on swaying foliage (swaysUnder) in
 * the inner `size` x `size` columns of `blocks` (meshBlocks' layout and `pad`),
 * whose first inner column is world (origin[0], origin[1]).
 */
export function snowOnFoliage(blocks, size, pad, origin) {
  const w = size + 2 * pad, out = [];
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const base = ((z + pad) * w + x + pad) * H;
    for (let y = 1; y < H; y++) {
      if (blocks[base + y] === B.snow_layer && swaysUnder(blocks[base + y - 1], false)) out.push([origin[0] + x, y, origin[1] + z]);
    }
  }
  return out;
}

export function createParticles() {
  const list = [];
  let seed = 0x9e3779b9, flakes = 0;
  const rand = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x100000000;
  };
  return {
    list,
    get count() {
      return list.length;
    },
    spawnMisses(events) {
      for (const e of events) {
        const m = /^forage_missed position=(-?\d+),(-?\d+),(-?\d+)$/.exec(e);
        if (!m) continue;
        const [x, y, z] = m.slice(1).map(Number);
        for (let i = 0; i < BURST; i++) {
          const a = (i / BURST) * 2 * Math.PI;
          const r = SPREAD * (0.5 + 0.5 * ((i * 5) % 3) / 2);
          list.push({ pos: [x + 0.5 + r * Math.cos(a), y + 0.3, z + 0.5 + r * Math.sin(a)], vel: [0, RISE, 0], age: 0, life: LIFE, size: SIZE });
        }
      }
    },
    spawnSnow(cells, eye, phase, dt) {
      for (const [x, y, z] of cells) {
        if (flakes >= SNOW_MAX) return;
        const cx = x + 0.5, cz = z + 0.5;
        if (Math.hypot(cx - eye[0], y - eye[1], cz - eye[2]) > SNOW_RADIUS) continue;
        // the gust is how far the wind leans this cell over, 0..1 (squared, so flakes come at the peaks)
        const [dx, dz] = swayOffset(1, cx, cz, phase, 1);
        const gust = (dx * dx + dz * dz) / 2;
        if (rand() >= SNOW_RATE * gust * gust * dt) continue;
        // off the edge the snow leans toward, anywhere along it
        const l = Math.hypot(dx, dz) || 1, ux = dx / l, uz = dz / l, along = rand() - 0.5;
        const pos = [cx + 0.55 * ux - along * uz, y + 0.15, cz + 0.55 * uz + along * ux];
        list.push({ pos, vel: [SNOW_DRIFT * ux, -SNOW_FALL, SNOW_DRIFT * uz], age: 0, life: SNOW_LIFE * (0.7 + 0.6 * rand()), size: SNOW_SIZE, snow: true });
        flakes++;
      }
    },
    update(dt) {
      for (let i = list.length - 1; i >= 0; i--) {
        const p = list[i];
        p.age += dt;
        if (p.age >= p.life) {
          if (p.snow) flakes--;
          list.splice(i, 1);
        } else for (let k = 0; k < 3; k++) p.pos[k] += p.vel[k] * dt;
      }
    },
  };
}
