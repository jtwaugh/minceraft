// Debug scene: generated terrain around a fixed seed, drawn with the viewer's
// mesher and tileset, flown with the wasm input layer and fly camera
// (crates/game-web). The sim steps at 20 Hz and rendering interpolates between ticks at display rate; look is
// applied every frame, as docs/design/basic-gameplay.md section 2.2 asks.
//
//   ?seed=N      world seed (default 12345)
//   ?radius=R    chunks of terrain each way from the spawn chunk (default 1)
//   ?scene=raft  the Phase 3 test scene: a flat sea, a raft rocking on the scripted swell,
//                and the player swimming in front of it (default: generated terrain)
//   ?mode=fly    start in the debug fly camera (default walk); F toggles while playing
//   ?plants=D    plant draw distance in blocks (default 48)
//   ?tool=T     what the walking player holds until the belt exists: empty (default), hoe, sword,
//                or item:<name>; the overlay shows which interaction rule a press would match
//   ?log=N       console.log the frame-time summary every N frames, for headless runs
//
// window.frameStats() returns { frames, p50, p99 } at any time.

import initTerrain, * as terrain from '../viewer/pkg/terrain_web.js';
import initGame, { InputSampler, Walk, Scene, SimClock, FlyCamera, view_matrix } from './pkg/game_web.js';
import { VERTEX_BYTES, POS_SCALE, UV_SCALE, SHADE_SCALE, B, legendContext, quadIndices, columnTints, meshBlocks, ramp3 } from '../viewer/mesh.js';
import { attachInput } from './input.js';
import { loadTiles } from './tiles.js';
import { PLANT_FADE_RANGE } from './plantfade.js';
import { formatAim, heldFromQuery } from './overlay.js';
import { createSampler } from './samplers.js';
import { createController } from './controller.js';
import { FrameTimes, FRAME_BUDGET_MS } from './frametimes.js';

const CHUNK = 64;
const qs = new URLSearchParams(location.search);
const seed = BigInt(qs.get('seed') ?? 12345);
const radius = Math.max(0, Math.min(3, +(qs.get('radius') ?? 1)));
const flying = qs.get('mode') === 'fly';
const raftScene = qs.get('scene') === 'raft';
const plantDistance = Math.max(0, +(qs.get('plants') ?? 48)); // blocks; beyond it plants shrink away over PLANT_FADE_RANGE
const held = heldFromQuery(qs.get('tool'));
const logEvery = +(qs.get('log') ?? 0);

const canvas = document.getElementById('gl');
const overlay = document.getElementById('overlay');
const hint = document.getElementById('hint');
const gl = canvas.getContext('webgl2', { antialias: true });
if (!gl) throw new Error('WebGL2 is not available');

// ---------------------------------------------------------------- shaders
// The viewer's shader, minus water animation, reading the split colour/tint
// textures from tiles.js. `cut` (leaves, plants) outputs coverage as alpha for
// alpha-to-coverage and discards below `cutoff`; `solid` does neither.
function program(cut) {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, `#version 300 es
    layout(location=0) in vec3 pos;
    layout(location=1) in vec2 uv;
    layout(location=2) in float layer;
    layout(location=3) in float shade;
    layout(location=4) in vec3 tint;
    layout(location=5) in float top;
    uniform mat4 mvp;
    uniform vec3 unpack;
    uniform vec3 offset;
    uniform vec3 eye; uniform vec2 plantFade; // start, range (blocks)
    out vec3 vuv; out float vshade; out float vdist; out vec3 vtint;
    void main() {
      vec3 w = pos * unpack.x + offset;
      ${cut ? `// mirrors plantScale in plantfade.js: top vertices sink one block to the base
      float s = 1.0 - clamp((distance(w, eye) - plantFade.x) / plantFade.y, 0.0, 1.0);
      w.y -= top * (1.0 - s);` : ''}
      gl_Position = mvp * vec4(w, 1.0);
      vuv = vec3(uv * unpack.y, layer);
      vshade = shade * unpack.z;
      vtint = tint;
      vdist = gl_Position.w;
    }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    precision mediump sampler2DArray;
    in vec3 vuv; in float vshade; in float vdist; in vec3 vtint;
    uniform sampler2DArray tex; uniform sampler2DArray tintTex;
    uniform float alpha; uniform float fogFar; uniform vec3 fogColor; uniform float cutoff;
    out vec4 o;
    void main() {
      vec4 t = texture(tex, vuv);
      ${cut ? 'if (t.a < cutoff) discard;' : ''}
      float g = texture(tintTex, vuv).r;
      vec3 c = (t.rgb + g * vtint) / max(t.a, 1.0 / 255.0) * vshade;
      float f = clamp((vdist - fogFar * 0.55) / (fogFar * 0.6), 0.0, 1.0);
      o = vec4(mix(c, fogColor, f), ${cut ? 't.a * alpha' : 'alpha'});
    }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  p.u = (name) => gl.getUniformLocation(p, name);
  return p;
}

// Selection outline and entity highlight: a unit cube drawn as lines or as a
// translucent fill, placed and scaled by uniforms.
const boxProg = (() => {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, `#version 300 es
    layout(location=0) in vec3 pos;
    uniform mat4 mvp; uniform vec3 lo; uniform vec3 size;
    void main() { gl_Position = mvp * vec4(lo + pos * size, 1.0); }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    uniform vec4 color; out vec4 o;
    void main() { o = color; }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  p.u = (name) => gl.getUniformLocation(p, name);
  return p;
})();
const unitBox = (() => {
  const corners = [];
  for (let i = 0; i < 8; i++) corners.push(i & 1, (i >> 1) & 1, (i >> 2) & 1);
  const edges = [0, 1, 2, 3, 4, 5, 6, 7, 0, 2, 1, 3, 4, 6, 5, 7, 0, 4, 1, 5, 2, 6, 3, 7];
  const quads = [0, 1, 3, 0, 3, 2, 4, 6, 7, 4, 7, 5, 0, 4, 5, 0, 5, 1, 2, 3, 7, 2, 7, 6, 0, 2, 6, 0, 6, 4, 1, 5, 7, 1, 7, 3];
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(corners), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
  const ib = (data) => {
    const b = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, b);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint8Array(data), gl.STATIC_DRAW);
    return b;
  };
  const edgeIB = ib(edges), quadIB = ib(quads);
  gl.bindVertexArray(null);
  return { vao, edgeIB, quadIB, edges: edges.length, quads: quads.length };
})();

// `box` is [minx, miny, minz, maxx, maxy, maxz]; entities also get a faint fill.
function drawBox(mvp, box, fill) {
  const grow = 0.003; // keeps the lines off the block's own faces
  const lo = [0, 1, 2].map((i) => box[i] - grow);
  const size = [0, 1, 2].map((i) => box[i + 3] - box[i] + 2 * grow);
  gl.useProgram(boxProg);
  gl.uniformMatrix4fv(boxProg.u('mvp'), false, mvp);
  gl.uniform3fv(boxProg.u('lo'), lo);
  gl.uniform3fv(boxProg.u('size'), size);
  gl.bindVertexArray(unitBox.vao);
  if (fill) {
    gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false); gl.disable(gl.CULL_FACE);
    gl.uniform4f(boxProg.u('color'), 1, 1, 1, 0.22);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, unitBox.quadIB);
    gl.drawElements(gl.TRIANGLES, unitBox.quads, gl.UNSIGNED_BYTE, 0);
    gl.depthMask(true); gl.enable(gl.CULL_FACE); gl.disable(gl.BLEND);
  }
  gl.uniform4f(boxProg.u('color'), 0, 0, 0, 1);
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, unitBox.edgeIB);
  gl.drawElements(gl.LINES, unitBox.edges, gl.UNSIGNED_BYTE, 0);
  gl.bindVertexArray(null);
}

// ---------------------------------------------------------------- meshes
let quadIB = null, quadIBSize = 0;
function quadIndexBuffer(quads) {
  if (quads > quadIBSize) {
    quadIBSize = Math.max(quads, quadIBSize * 2, 1 << 16);
    quadIB = gl.createBuffer();
    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, quadIB);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, quadIndices(quadIBSize), gl.STATIC_DRAW);
  }
  return quadIB;
}

function upload({ verts, faces }, offset) {
  if (!faces) return null;
  const ib = quadIndexBuffer(faces);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
  const attr = (loc, size, type, normalized, off) => {
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, size, type, normalized, VERTEX_BYTES, off);
  };
  attr(0, 3, gl.SHORT, false, 0); attr(1, 2, gl.UNSIGNED_BYTE, false, 6); attr(2, 1, gl.UNSIGNED_BYTE, false, 8);
  attr(3, 1, gl.UNSIGNED_BYTE, false, 9); attr(4, 3, gl.UNSIGNED_BYTE, true, 10);
  attr(5, 1, gl.UNSIGNED_BYTE, true, 14);
  gl.bindVertexArray(null);
  return { vao, n: faces * 6, offset };
}

// ---------------------------------------------------------------- matrices (column-major)
function perspective(fovY, aspect, near, far) {
  const f = 1 / Math.tan(fovY / 2);
  return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) / (near - far), -1, 0, 0, 2 * far * near / (near - far), 0];
}
function mul(a, b) {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return o;
}

// ---------------------------------------------------------------- setup
await Promise.all([initTerrain(), initGame()]);
const sampler = new InputSampler();
// heightmap_decorated column: [top_block, top_y, ...]; spawn just above the top block.
const surfaceY = terrain.heightmap_decorated(seed, CHUNK / 2, CHUNK / 2, 1, 1)[1] + 1;
const scene = raftScene ? new Scene() : null;
const walk = scene ? Walk.in_scene(scene) : new Walk(CHUNK / 2, surfaceY + 0.5, CHUNK / 2);
walk.set_held(held);
const input = attachInput({ sampler, canvas, document, window });

const names = [];
const legendJson = terrain.legend();
for (const b of JSON.parse(legendJson).blocks) names[b.id] = b.name;
const faceLayer = await loadTiles(gl, B, names, '../viewer/textures.json');
const ctx = legendContext(legendJson, faceLayer);

const solidProg = program(false), cutProg = program(true);
// With MSAA, cutout edges use alpha-to-coverage so distant leaves fade to
// partial coverage instead of popping in and out at a hard alpha threshold.
const msaa = gl.getParameter(gl.SAMPLES) > 0;
const cut = (draw) => {
  if (msaa) gl.enable(gl.SAMPLE_ALPHA_TO_COVERAGE);
  draw();
  if (msaa) gl.disable(gl.SAMPLE_ALPHA_TO_COVERAGE);
};
let skyColor = [0.61, 0.76, 0.9];
const meshes = { opaque: [], cutout: [], water: [], ice: [] };

// One chunk per frame, spawn chunk outwards, so the page is flying at once.
const queue = [];
const raftMeshes = { opaque: [], cutout: [] }; // moved each frame by the sim's raft offset
for (let r = 0; r <= radius && !scene; r++) {
  for (let cz = -r; cz <= r; cz++) for (let cx = -r; cx <= r; cx++) {
    if (Math.max(Math.abs(cx), Math.abs(cz)) === r) queue.push([cx, cz]);
  }
}
function meshNextChunk() {
  const [cx, cz] = queue.shift();
  const x = cx * CHUNK, z = cz * CHUNK;
  const blocks = terrain.generate_region_decorated(seed, x - 1, z - 1, CHUNK + 2, CHUNK + 2);
  const tints = columnTints(terrain, ctx, seed, x, z, CHUNK);
  if (cx === 0 && cz === 0) skyColor = ramp3(tints.centre.biome.sky, tints.centre.t).map((v) => v / 255);
  walk.add_region(x - 1, z - 1, CHUNK + 2, CHUNK + 2, blocks);
  const m = meshBlocks(ctx, blocks, tints, CHUNK, 1);
  const offset = [x, 0, z];
  for (const k of Object.keys(meshes)) {
    const part = upload(m[k], offset);
    if (part) meshes[k].push(part);
  }
}

// The test scene is one sea region plus the raft's blocks meshed apart, so the
// raft can move over the water. Biome tints come from the terrain under it.
if (scene) {
  const { min, size } = scene;
  const tints = columnTints(terrain, ctx, seed, min, min, size);
  skyColor = ramp3(tints.centre.biome.sky, tints.centre.t).map((v) => v / 255);
  const offset = [min, 0, min];
  for (const [blocks, into] of [[scene.sea_blocks(), true], [scene.raft_blocks(), false]]) {
    const m = meshBlocks(ctx, blocks, tints, size, 0);
    for (const k of Object.keys(meshes)) {
      const part = upload(m[k], offset);
      if (!part) continue;
      if (into) meshes[k].push(part);
      else if (raftMeshes[k]) raftMeshes[k].push(part);
    }
  }
}

// ---------------------------------------------------------------- loop
const clock = new SimClock();
const fly = new FlyCamera(CHUNK / 2, surfaceY + 24, CHUNK / 2);
if (scene) {
  const [x, y, z] = walk.eye(1);
  fly.teleport(x, y + 24, z);
}
const controller = createController({ input, walk, fly, clock, document, flying });
const frames = new FrameTimes(300);
let lastFrame = null, overlayAt = 0;
window.frameStats = () => frames.summary();

function drawParts(prog, parts) {
  for (const m of parts) {
    gl.uniform3fv(prog.u('offset'), m.offset);
    gl.bindVertexArray(m.vao);
    gl.drawElements(gl.TRIANGLES, m.n, gl.UNSIGNED_INT, 0);
  }
}

function render(now) {
  if (queue.length) meshNextChunk();

  const { alpha, eye, yaw, pitch, flying, aim } = controller.frame(lastFrame === null ? 0 : (now - lastFrame) / 1000);

  const dpr = window.devicePixelRatio || 1;
  const w = Math.round(canvas.clientWidth * dpr), h = Math.round(canvas.clientHeight * dpr);
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  gl.viewport(0, 0, w, h);
  gl.clearColor(...skyColor, 1);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  gl.enable(gl.DEPTH_TEST); gl.enable(gl.CULL_FACE); gl.disable(gl.BLEND);

  const mvp = new Float32Array(mul(
    perspective(sampler.fov_degrees * Math.PI / 180, w / h, 0.3, 1000),
    view_matrix(eye[0], eye[1], eye[2], yaw, pitch),
  ));
  const fogFar = (radius + 0.5) * CHUNK * 1.1;
  const bind = (prog, a) => {
    gl.useProgram(prog);
    const s = prog === cutProg ? cutSampler : solidSampler;
    gl.bindSampler(0, s); gl.bindSampler(1, s);
    gl.uniformMatrix4fv(prog.u('mvp'), false, mvp);
    gl.uniform3fv(prog.u('unpack'), [POS_SCALE, UV_SCALE, SHADE_SCALE]);
    gl.uniform3fv(prog.u('fogColor'), skyColor);
    gl.uniform1f(prog.u('fogFar'), fogFar);
    gl.uniform1f(prog.u('alpha'), a);
    if (prog === cutProg) { gl.uniform3fv(prog.u('eye'), eye); gl.uniform2f(prog.u('plantFade'), plantDistance, PLANT_FADE_RANGE); }
  };
  bind(solidProg, 1); drawParts(solidProg, meshes.opaque);
  bind(cutProg, 1); cut(() => drawParts(cutProg, meshes.cutout));
  if (scene) {
    const [dx, dy, dz] = walk.raft_offset(flying ? 1 : alpha);
    const moved = (parts) => parts.map((p) => ({ ...p, offset: [p.offset[0] + dx, p.offset[1] + dy, p.offset[2] + dz] }));
    bind(solidProg, 1); drawParts(solidProg, moved(raftMeshes.opaque));
    bind(cutProg, 1); cut(() => drawParts(cutProg, moved(raftMeshes.cutout)));
  }
  gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.depthMask(false);
  bind(solidProg, 0.45); drawParts(solidProg, meshes.water);
  bind(solidProg, 0.72); drawParts(solidProg, meshes.ice);
  gl.depthMask(true);
  gl.bindVertexArray(null);
  if (!flying && aim.box) drawBox(mvp, aim.box, aim.kind === 'entity');

  if (lastFrame !== null) frames.push(now - lastFrame);
  lastFrame = now;
  if (now - overlayAt > 250) {
    overlayAt = now;
    const p99 = frames.percentile(99);
    overlay.textContent = `${frames.format()}\n${p99 > FRAME_BUDGET_MS ? 'over' : 'within'} the 16.7 ms p99 budget\n` +
      `${flying ? 'fly' : 'walk'} (F to switch)\npos ${eye.map((v) => v.toFixed(1)).join(' ')}${flying ? "" : `\nheld ${held}\n${formatAim(aim)}`}`;
    hint.hidden = input.locked;
  }
  if (logEvery && frames.n % logEvery === 0 && frames.n) console.log(`[frames] ${frames.format()}`);
  requestAnimationFrame(render);
}

gl.activeTexture(gl.TEXTURE0);
// Opaque and water repeat; cutout clamps and stops at the 4x4 mip (samplers.js).
const solidSampler = createSampler(gl, 'opaque');
const cutSampler = createSampler(gl, 'cutout');
for (const p of [solidProg, cutProg]) {
  gl.useProgram(p);
  gl.uniform1i(p.u('tex'), 0); gl.uniform1i(p.u('tintTex'), 1);
  gl.uniform1f(p.u('cutoff'), msaa ? 1 / 255 : 0.5);
}
requestAnimationFrame(render);
