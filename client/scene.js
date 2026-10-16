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
//   ?log=N       console.log the frame-time summary every N frames, for headless runs
//
// window.frameStats() returns { frames, p50, p99 } at any time.

import initTerrain, * as terrain from '../viewer/pkg/terrain_web.js';
import initGame, { InputSampler, Walk, Scene, SimClock, FlyCamera, view_matrix } from './pkg/game_web.js';
import { VERTEX_BYTES, POS_SCALE, UV_SCALE, SHADE_SCALE, B, legendContext, quadIndices, columnTints, meshBlocks, ramp3 } from '../viewer/mesh.js';
import { attachInput } from './input.js';
import { loadTiles } from './tiles.js';
import { createController } from './controller.js';
import { FrameTimes, FRAME_BUDGET_MS } from './frametimes.js';

const CHUNK = 64;
const qs = new URLSearchParams(location.search);
const seed = BigInt(qs.get('seed') ?? 12345);
const radius = Math.max(0, Math.min(3, +(qs.get('radius') ?? 1)));
const flying = qs.get('mode') === 'fly';
const raftScene = qs.get('scene') === 'raft';
const logEvery = +(qs.get('log') ?? 0);

const canvas = document.getElementById('gl');
const overlay = document.getElementById('overlay');
const hint = document.getElementById('hint');
const gl = canvas.getContext('webgl2', { antialias: true });
if (!gl) throw new Error('WebGL2 is not available');

// ---------------------------------------------------------------- shaders
// The viewer's shader, minus water animation. `cut` discards transparent
// texels (leaves, plants); `solid` does not.
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
    uniform mat4 mvp;
    uniform vec3 unpack;
    uniform vec3 offset;
    out vec3 vuv; out float vshade; out float vdist; out vec3 vtint;
    void main() {
      gl_Position = mvp * vec4(pos * unpack.x + offset, 1.0);
      vuv = vec3(uv * unpack.y, layer);
      vshade = shade * unpack.z;
      vtint = tint;
      vdist = gl_Position.w;
    }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    precision mediump sampler2DArray;
    in vec3 vuv; in float vshade; in float vdist; in vec3 vtint;
    uniform sampler2DArray tex;
    uniform float alpha; uniform float fogFar; uniform vec3 fogColor;
    out vec4 o;
    void main() {
      vec4 t = texture(tex, vuv);
      ${cut ? 'if (t.a < 0.5) discard;' : ''}
      vec3 c = (t.a < 0.9 ? t.rgb * vtint : t.rgb) * vshade;
      float f = clamp((vdist - fogFar * 0.55) / (fogFar * 0.6), 0.0, 1.0);
      o = vec4(mix(c, fogColor, f), alpha);
    }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  p.u = (name) => gl.getUniformLocation(p, name);
  return p;
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
const input = attachInput({ sampler, canvas, document, window });

const names = [];
const legendJson = terrain.legend();
for (const b of JSON.parse(legendJson).blocks) names[b.id] = b.name;
const faceLayer = await loadTiles(gl, B, names, '../viewer/textures.json');
const ctx = legendContext(legendJson, faceLayer);

const solidProg = program(false), cutProg = program(true);
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

  const { alpha, eye, yaw, pitch, flying } = controller.frame(lastFrame === null ? 0 : (now - lastFrame) / 1000);

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
    gl.uniformMatrix4fv(prog.u('mvp'), false, mvp);
    gl.uniform3fv(prog.u('unpack'), [POS_SCALE, UV_SCALE, SHADE_SCALE]);
    gl.uniform3fv(prog.u('fogColor'), skyColor);
    gl.uniform1f(prog.u('fogFar'), fogFar);
    gl.uniform1f(prog.u('alpha'), a);
  };
  bind(solidProg, 1); drawParts(solidProg, meshes.opaque);
  bind(cutProg, 1); drawParts(cutProg, meshes.cutout);
  if (scene) {
    const [dx, dy, dz] = walk.raft_offset(flying ? 1 : alpha);
    const moved = (parts) => parts.map((p) => ({ ...p, offset: [p.offset[0] + dx, p.offset[1] + dy, p.offset[2] + dz] }));
    bind(solidProg, 1); drawParts(solidProg, moved(raftMeshes.opaque));
    bind(cutProg, 1); drawParts(cutProg, moved(raftMeshes.cutout));
  }
  gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.depthMask(false);
  bind(solidProg, 0.45); drawParts(solidProg, meshes.water);
  bind(solidProg, 0.72); drawParts(solidProg, meshes.ice);
  gl.depthMask(true);
  gl.bindVertexArray(null);

  if (lastFrame !== null) frames.push(now - lastFrame);
  lastFrame = now;
  if (now - overlayAt > 250) {
    overlayAt = now;
    const p99 = frames.percentile(99);
    overlay.textContent = `${frames.format()}\n${p99 > FRAME_BUDGET_MS ? 'over' : 'within'} the 16.7 ms p99 budget\n` +
      `${flying ? 'fly' : 'walk'} (F to switch)\npos ${eye.map((v) => v.toFixed(1)).join(' ')}`;
    hint.hidden = input.locked;
  }
  if (logEvery && frames.n % logEvery === 0 && frames.n) console.log(`[frames] ${frames.format()}`);
  requestAnimationFrame(render);
}

gl.activeTexture(gl.TEXTURE0);
for (const p of [solidProg, cutProg]) { gl.useProgram(p); gl.uniform1i(p.u('tex'), 0); }
requestAnimationFrame(render);
