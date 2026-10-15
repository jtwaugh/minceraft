// Background worker for the viewer (see pool.js). Each worker loads its own
// copy of the wasm. Messages are { id, type, args }; replies { id, result }
// or { id, error }, with typed-array buffers transferred.
//
//   init   { faceLayer, module }         once, first: the page's face-to-layer table (mesh.js)
//                                        and its compiled wasm module (shared, not recompiled)
//   tile   { seed, x, z, size }          decorated map tile: heightmap, biome, temperature, humidity
//   coarse { seed, x0, z0, step, nx, nz } coarse_heightmap survey (the map's quick first draft)
//   plan   { seed, cx, cz, bounds }      flyover plan data (flyover/plan.js planData)
//   chunk  { seed, cx, cz, fast }        decorated 64x64 chunk (plus a one-block ring), meshed
//                                        (fast: fast leaves, for chunks seen only from afar)
import init, * as wasm from './pkg/terrain_web.js';
import { legendContext, columnTints, meshBlocks } from './mesh.js';
import { planData, CHUNK } from './flyover/plan.js';

let faceLayer, ready;

const tasks = {
  tile({ seed, x, z, size }) {
    const r = {
      hm: wasm.heightmap_decorated(seed, x, z, size, size),
      biome: wasm.biome_map(seed, x, z, size, size),
      temp: wasm.temperature_map(seed, x, z, size, size),
      hum: wasm.humidity_map(seed, x, z, size, size),
    };
    return [r, Object.values(r).map((a) => a.buffer)];
  },
  coarse({ seed, x0, z0, step, nx, nz }) {
    const raw = wasm.coarse_heightmap(seed, x0, z0, step, nx, nz);
    return [raw, [raw.buffer]];
  },
  plan({ seed, cx, cz, bounds }) {
    return [planData(wasm, seed, cx, cz, { bounds }), []];
  },
  chunk({ seed, cx, cz, fast = false }, ctx) {
    const x = cx * CHUNK, z = cz * CHUNK;
    const blocks = wasm.generate_region_decorated(seed, x - 1, z - 1, CHUNK + 2, CHUNK + 2);
    const tints = columnTints(wasm, ctx, seed, x, z, CHUNK);
    const m = meshBlocks(ctx, blocks, tints, CHUNK, 1, { fast });
    const parts = { opaque: m.opaque, cutout: m.cutout, water: m.water, ice: m.ice };
    return [{ cx, cz, fast, ...parts, meanY: m.meanY, minY: m.minY }, Object.values(parts).map((q) => q.verts)];
  },
};

self.onmessage = async ({ data: { id, type, args } }) => {
  if (type === 'init') {
    faceLayer = args.faceLayer;
    ready = init(args.module ? { module_or_path: args.module } : undefined).then(() => legendContext(wasm.legend()));
    return;
  }
  try {
    const ctx = { ...(await ready), faceLayer };
    const t0 = performance.now();
    const [result, transfer] = tasks[type](args, ctx);
    self.postMessage({ id, result, ms: performance.now() - t0 }, transfer);
  } catch (e) {
    self.postMessage({ id, error: String(e && e.stack || e) });
  }
};
