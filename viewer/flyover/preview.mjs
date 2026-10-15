// Plan a flyover in Node (no browser) and print it, optionally drawing the
// survey, island, route and features to a PPM image.
//
//   ./build.sh                      # viewer/pkg must be current
//   node viewer/flyover/preview.mjs <seed> [x0] [z0] [out.ppm]
//
// x0, z0: the map's top-left corner, as in the viewer's x and z fields. Like
// the viewer, it keeps the route on that 1024x1024-block map if it can.
import fs from 'fs';
import { fileURLToPath } from 'url';
import { planFlyover } from './plan.js';

const pkg = fileURLToPath(new URL('../pkg/', import.meta.url));
const wasm = await import(pkg + 'terrain_web.js');
await wasm.default({ module_or_path: fs.readFileSync(pkg + 'terrain_web_bg.wasm') });

const [seedArg = '12345', x0Arg = '0', z0Arg = '0', out] = process.argv.slice(2);
const seed = /^-?\d+$/.test(seedArg) ? BigInt.asUintN(64, BigInt(seedArg)) : null;
if (seed === null) { console.error('numeric seeds only here (the viewer hashes text seeds)'); process.exit(1); }
const MAP = 1024, x0 = +x0Arg, z0 = +z0Arg;
const plan = planFlyover(wasm, seed, x0 + MAP / 2, z0 + MAP / 2, { bounds: { x0, z0, x1: x0 + MAP, z1: z0 + MAP } });
if (!plan) { console.log('no island in the survey'); process.exit(0); }

const { survey: g, island, chosen, path } = plan;
console.log(`planned in ${plan.ms} ms: ${plan.islandCount} islands, ${plan.featureCount} candidate features, ${plan.schedule.length} chunks in view`);
if (plan.moved) console.log(`no route fits the map; it would move to x ${plan.mapOrigin[0]} z ${plan.mapOrigin[1]}`);
console.log(`start: ${island.label} at x ${Math.round(island.cx)} z ${Math.round(island.cz)}, ${island.blocks} m², ` +
  `top y ${island.maxH}, open water ${island.iso}+ blocks, score ${island.total.toFixed(2)}`);
chosen.forEach((f, i) => console.log(`  ${i + 1}. ${f.label.padEnd(22)} x ${f.x} z ${f.z} y ${f.y}  (${f.kind}, score ${f.score.toFixed(2)})`));
console.log(`${plan.keys.length} keyframes, ${path.duration.toFixed(1)} s`);

if (out) {
  const N = g.nx, img = Buffer.alloc(N * g.nz * 3);
  for (let c = 0; c < N * g.nz; c++) {
    const h = g.top[c];
    img.set(h >= 64 ? [50 + (h - 64) * 3, 110 + (h - 64) * 2, 40] : [0, 40, 120 + Math.max(0, h - 40) * 2], c * 3);
    if (g.gapH[c] >= 3) img.set([255, 0, 255], c * 3);
  }
  for (const c of island.cells) img.set([255, 220, 120], c * 3);
  const put = (x, z, col, r = 0) => {
    const i = Math.round((x - g.x0) / g.step), k = Math.round((z - g.z0) / g.step);
    for (let dk = -r; dk <= r; dk++) for (let di = -r; di <= r; di++) {
      if (i + di >= 0 && i + di < N && k + dk >= 0 && k + dk < g.nz) img.set(col, ((k + dk) * N + i + di) * 3);
    }
  };
  for (let t = 0; t < path.duration; t += 0.1) { const { eye } = path.at(t); put(eye[0], eye[2], [255, 255, 255]); }
  for (const f of chosen) put(f.x, f.z, [255, 40, 40], 2);
  fs.writeFileSync(out, Buffer.concat([Buffer.from(`P6 ${N} ${g.nz} 255\n`), img]));
  console.log(`wrote ${out} (${N}x${g.nz}, one pixel per ${g.step} blocks)`);
}
