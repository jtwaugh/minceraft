// Short-lived particles, apart from WebGL so they run in Node. The controller
// spawns a burst on each `forage_missed` event (docs/design/picking-plants.md
// note 11) and ages the list every frame; scene.js draws what `list` holds.
//
//   const p = createParticles();
//   p.spawnMisses(events)  burst at the position of each `forage_missed position=x,y,z` event
//   p.update(dt)           age by dt seconds, drop the expired
//   p.list                 live particles: { pos: [x, y, z], age, life, size }
//   p.count                how many are live

const BURST = 8;
const LIFE = 0.6; // seconds
const RISE = 0.8; // blocks per second
const SPREAD = 0.35; // blocks, around the cell's centre
const SIZE = 0.06;

export function createParticles() {
  const list = [];
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
          list.push({ pos: [x + 0.5 + r * Math.cos(a), y + 0.3, z + 0.5 + r * Math.sin(a)], base: y + 0.3, age: 0, life: LIFE, size: SIZE });
        }
      }
    },
    update(dt) {
      for (let i = list.length - 1; i >= 0; i--) {
        const p = list[i];
        p.age += dt;
        if (p.age >= p.life) list.splice(i, 1);
        else p.pos[1] = p.base + RISE * p.age;
      }
    },
  };
}
