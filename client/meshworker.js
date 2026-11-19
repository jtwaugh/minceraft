// Module worker that meshes streamed regions off the main thread
// (docs/design/stream-without-stalls.md section 3). It loads its own instance
// of the terrain wasm, as viewer/worker.js does. meshclient.js talks to it.
//
//   { type: 'init', module, ctx, seed, blendRadius }   once, first: the page's compiled
//                                        terrain module (shared, not recompiled), its mesh
//                                        ctx (legendContext) and the world's seed and blend
//   { type: 'mesh', id, rx, rz, blocks }  region (rx, rz)'s padded blocks (gatherRegion)
//
// Replies are { id, result } with meshPadded's result, every typed-array buffer
// in it transferred, or { id, error }. `createMeshHandler` is the same handler
// as a plain function, so Node can run it without a Worker.
import init, * as wasm from '../viewer/pkg/terrain_web.js';
import { meshPadded } from './regionmesh.js';

// Every ArrayBuffer in `value` (bare or behind a typed array), once each.
export function buffersIn(value, out = new Set()) {
  if (value instanceof ArrayBuffer) out.add(value);
  else if (ArrayBuffer.isView(value)) out.add(value.buffer);
  else if (value && typeof value === 'object') for (const v of Object.values(value)) buffersIn(v, out);
  return out;
}

// handle(message) -> [reply, transfer], or null for init (which has no reply).
export function createMeshHandler(terrain = wasm, load = init) {
  let env;
  return async function handle(data) {
    if (data.type === 'init') {
      const { module, ctx, seed, blendRadius } = data;
      env = load(module ? { module_or_path: module } : undefined).then(() => ({ terrain, ctx, seed, blendRadius }));
      return null;
    }
    const { id, rx, rz, blocks } = data;
    try {
      const result = meshPadded(await env, { rx, rz, blocks });
      return [{ id, result }, [...buffersIn(result)]];
    } catch (e) {
      return [{ id, error: String((e && e.stack) || e) }, []];
    }
  };
}

if (typeof WorkerGlobalScope !== 'undefined' && self instanceof WorkerGlobalScope) {
  const handle = createMeshHandler();
  self.onmessage = async ({ data }) => {
    const reply = await handle(data);
    if (reply) self.postMessage(...reply);
  };
}
