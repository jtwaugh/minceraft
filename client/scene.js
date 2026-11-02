// Debug scene: generated terrain around a fixed seed, drawn with the viewer's
// mesher and tileset, flown with the wasm input layer and fly camera
// (crates/game-web). The sim steps at 20 Hz and rendering interpolates between ticks at display rate; look is
// applied every frame, as docs/design/basic-gameplay.md section 2.2 asks.
//
//   ?seed=S      world seed: an integer, or any text (hashed as the viewer does; default 12345)
//   ?scene=raft  the Phase 3 test scene: a flat sea, a raft rocking on the scripted swell,
//                and the player swimming in front of it (default: generated terrain)
//   ?mode=fly    start in the debug fly camera (default walk); F toggles while playing
//   ?blend=R     biome colour blend radius, 0 to 7 (default: the legend's)
//   ?plants=D    plant draw distance in blocks (default 48)
//   ?debug=1     debug mode on (also on whenever ?scene= is set); see debug.js
//   ?log=N      console.log the frame-time summary every N frames, for headless runs
//
// window.frameStats() returns { frames, p50, p99, max, hitches, records } at any time:
// max is the slowest frame in the window, hitches the frames over HITCH_MS, records
// the work done in the last hitch frames ({ ms, meshed, remeshed, verts, ticks, chunks }).
//
// window.perfSpans() returns { tick, mesh, upload }, each { count, total, max } in ms
// since the last call (then resets). The spans are performance.mark/measure pairs
// (perfspans.js); mesh includes the uploads made inside it.

import initTerrain, * as terrain from '../viewer/pkg/terrain_web.js';
import initGame, * as game from './pkg/game_web.js';
import { VERTEX_BYTES, AO_SHADE, POS_SCALE, UV_SCALE, SHADE_SCALE, B, legendContext, variantTable, quadIndices, columnTints, ramp3 } from '../viewer/mesh.js';
import { meshBlocksWasm } from '../viewer/meshwasm.js';
import { attachInput } from './input.js';
import { createModelDrawer } from './modelgl.js';
import { animalBoxes, animalPose } from './models.js';
import { fishBoxes, fishDraw, createTailTracker } from './fishmodels.js';
import { raftBoxes, raftMatrix } from './raftmodel.js';
import { itemBoxes, itemDraws } from './itemmodel.js';
import { heldBoxes, heldDraw } from './helditem.js';
import { SCENE_TEXTURES } from './scenetextures.js';
import { loadTiles, frontLayers, NAMES as LAYER_NAMES, ANIM_FIRST, ANIM_END } from './tiles.js';
import { VARIANTS } from '../viewer/tilelist.js';
import { waterFrame } from './waterframe.js';
import { PLANT_FADE_RANGE } from './plantfade.js';
import { SWAY_AMPLITUDE, SWAY_WAVE, TRUNK_SWAY_AMPLITUDE, swayPhase } from './sway.js';
import { mountHud } from './hud.js';
import { mountInventory } from './inventoryui.js';
import { mountDeath } from './deathui.js';
import { mountHint, hintScale } from './hintui.js';
import { mountCue } from './cueui.js';
import { waitingForClick } from './waitingcue.js';
import { mountCornerButton, CORNER_MARGIN } from './cornerbutton.js';
import { formatTime, formatAim, formatBelt, spawnTestItems, spawnTestAnimals, readAnimals, readFish, readItems } from './overlay.js';
import { createSampler } from './samplers.js';
import { createController } from './controller.js';
import { createSpans } from './perfspans.js';
import { meshRegionCpu } from './regionmesh.js';
import { REGION_SIZE } from './stream.js';
import { NEAR, farPlane, look, skyInputs } from './view.js';
import { sampleSky, easeSky, SKY_SAMPLE_RADIUS } from './skyblend.js';
import { FOG_GLSL } from '../viewer/fog.js';
import { SNAP_GLSL, SNAP_SAMPLE_GLSL } from './texsnap.js';
import { SKY_GLSL } from '../viewer/sky.js';
import { WATER_GLSL, WATER_DEPTH_GLSL } from '../viewer/water.js';
import { FrameTimes, FRAME_BUDGET_MS } from './frametimes.js';
import { debugEnabled } from './debug.js';
import { parseBlend } from './blend.js';
import { sceneById } from './scenes.js';
import { parseSeed } from './seed.js';
import { History, complete, nameLists } from './consolelogic.js';
import { OutputBuffer, createConsoleUI } from './consoleui.js';
import { mountPanelStyle } from './uistyle.js';
import { snowOnFoliage } from './particles.js';

const { InputSampler, Walk, Scene, SimClock, FlyCamera, view_matrix } = game;

const CHUNK = 64;
const qs = new URLSearchParams(location.search);
const seed = parseSeed(qs.get('seed'));
const flying = qs.get('mode') === 'fly';
const raftScene = sceneById(qs.get('scene'))?.id === 'raft';
const plantDistance = Math.max(0, +(qs.get('plants') ?? 48)); // blocks; beyond it plants shrink away over PLANT_FADE_RANGE
const blendRadius = parseBlend(qs.get('blend')); // undefined: the legend default
const logEvery = +(qs.get('log') ?? 0);
const debug = debugEnabled(location.search); // ?debug=1 or any ?scene=; console keys are gated on it
window.debugMode = debug;

const canvas = document.getElementById('gl');
const overlay = document.getElementById('overlay');
const gl = canvas.getContext('webgl2', { antialias: true });
if (!gl) throw new Error('WebGL2 is not available');

// ---------------------------------------------------------------- shaders
// The viewer's shader, including water animation (`waterFrame` offsets the
// water layers; the frame comes from waterframe.js), reading the split colour/tint
// textures from tiles.js. `cut` (leaves, plants) outputs coverage as alpha for
// alpha-to-coverage and discards below 0.5 without MSAA; `solid` does neither. Fog
// is the radial curve of viewer/fog.js when `fogFar` > 0, else a linear ramp from
// `fogStart` to `fogEnd` (under water); both measure distance from the eye. `tint` multiplies the
// lit colour, both set per frame by view.js `look`, so the same shader draws
// above and under water.
// GLSL for swayOffset in sway.js at `amplitude`: moves `w.xz` by weight `top` along the shared wave.
const swayGlsl = (amplitude) => `float k = ${SWAY_WAVE.toFixed(3)};
      w.xz += ${amplitude.toFixed(3)} * top * vec2(sin(swayPhase + (w.x + 0.6 * w.z) * k), cos(swayPhase + (w.z - 0.6 * w.x) * k));`;
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
    layout(location=6) in float ao; // AO level 0-3 (mesh.js byte 13)
    layout(location=7) in float depth; // water depth 0-255 (mesh.js byte 15); 0 off water
    const float AO_SHADE[4] = float[4](${AO_SHADE.map((v) => v.toFixed(3)).join(', ')});
    uniform mat4 mvp;
    uniform vec3 unpack;
    uniform vec3 offset;
    uniform vec3 eye; uniform vec2 plantFade; // start, range (blocks)
    uniform float waterFrame;
    uniform float swayPhase; // swayPhase in sway.js
    out vec3 vuv; out float vshade; out float vdist; out vec3 vtint; out vec3 vdir; out float vdepth;
    void main() {
      vec3 w = pos * unpack.x + offset;
      ${cut ? `// mirrors plantScale in plantfade.js: top vertices sink one block to the base
      float s = 1.0 - clamp((distance(w, eye) - plantFade.x) / plantFade.y, 0.0, 1.0);
      // weights below 0.5 are leaves (LEAF_WEIGHT in mesh.js): they sway but never sink
      w.y -= step(0.5, top) * top * (1.0 - s);
      // mirrors swayOffset in sway.js: tops move sideways, bases stay put
      ${swayGlsl(SWAY_AMPLITUDE)}` : `// mirrors trunkLean in sway.js: logs lean with height, weight 0 stays put
      ${swayGlsl(TRUNK_SWAY_AMPLITUDE)}`}
      gl_Position = mvp * vec4(w, 1.0);
      bool anim = layer > ${(ANIM_FIRST - 0.5).toFixed(1)} && layer < ${(ANIM_END - 0.5).toFixed(1)};
      vuv = vec3(uv * unpack.y, anim ? layer + waterFrame : layer);
      vshade = shade * unpack.z * AO_SHADE[int(ao)];
      vtint = tint;
      vdepth = depth;
      vdist = distance(w, eye);
      vdir = w - eye;
    }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    precision mediump sampler2DArray;
    in vec3 vuv; in float vshade; in float vdist; in vec3 vtint; in vec3 vdir; in float vdepth;
    uniform sampler2DArray tex; uniform sampler2DArray tintTex;
    uniform float alpha; uniform float fogStart; uniform float fogEnd; uniform float fogFar; uniform vec3 fogColor; uniform vec3 tint; uniform vec3 sunDir;
    uniform float waterFresnel; // 1 while drawing water from above the surface
    out vec4 o;
    ${FOG_GLSL}
    ${SNAP_GLSL}
    ${SKY_GLSL}
    ${WATER_GLSL}
    ${WATER_DEPTH_GLSL}
    void main() {
      // snap UVs to texel centres; the unsnapped derivatives keep mip selection
      ${SNAP_SAMPLE_GLSL}
      vec4 t = textureGrad(tex, vec3(snapped, vuv.z), dFdx(vuv.xy), dFdy(vuv.xy));
      ${cut && !msaa ? 'if (t.a < 0.5) discard;' : ''}
      ${cut && msaa ? 'float ca = clamp((t.a - 0.5) / max(fwidth(t.a), 1e-4) + 0.5, 0.0, 1.0);' : ''}
      float g = texture(tintTex, vuv).r;
      // depth shading is for water seen from above; under water the look is unchanged
      float wd = vdepth * waterFresnel;
      vec3 c = (t.rgb + g * vtint) / max(t.a, 1.0 / 255.0) * vshade * tint * waterColorFactor(wd);
      vec3 vd = normalize(vdir);
      // water reflects the sky: blend toward it at low view angles (as in the viewer)
      c = mix(c, skyColor(fogColor, vec3(vd.x, -vd.y, vd.z), sunDir), waterFresnel * fresnel(vd));
      float f = fogFar > 0.0 ? fogAmount(vdist, fogFar) : clamp((vdist - fogStart) / (fogEnd - fogStart), 0.0, 1.0);
      // above water fog fades toward the sky in the view direction; under water it is flat
      vec3 fc = fogFar > 0.0 ? skyColor(fogColor, vd, sunDir) : fogColor;
      o = vec4(mix(c, fc, f), ${cut ? (msaa ? 'ca * alpha' : 't.a * alpha') : 'min(alpha * waterAlphaScale(wd), 1.0)'});
    }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  p.u = (name) => gl.getUniformLocation(p, name);
  return p;
}

// Full-screen sky pass: one triangle, each pixel's direction rebuilt from the
// camera basis (view.js skyInputs), coloured by the same skyColor the fog mixes toward.
const skyProg = (() => {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, `#version 300 es
    uniform vec3 camRight; uniform vec3 camUp; uniform vec3 camFwd; uniform vec2 skyScale;
    out vec3 vdir;
    void main() {
      vec2 p = vec2((gl_VertexID & 1) * 4 - 1, (gl_VertexID >> 1) * 4 - 1);
      vdir = camFwd + camRight * p.x * skyScale.x + camUp * p.y * skyScale.y;
      gl_Position = vec4(p, 1.0, 1.0);
    }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    in vec3 vdir;
    uniform vec3 fogColor; uniform vec3 sunDir;
    out vec4 o;
    ${SKY_GLSL}
    void main() { o = vec4(skyColor(fogColor, normalize(vdir), sunDir), 1.0); }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  p.u = (name) => gl.getUniformLocation(p, name);
  return p;
})();
const skyVao = gl.createVertexArray(); // attribute-less: the vertex shader uses gl_VertexID

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
    uniform mat4 mvp; uniform mat4 model; uniform vec3 lo; uniform vec3 size;
    void main() { gl_Position = mvp * model * vec4(lo + pos * size, 1.0); }`));
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

const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

// Animals as textured boxes (batch 15): red while hurt, tipped over while dying
// (see animalPose). Unknown species draw grey.
const animalModels = {};
function drawAnimals(mvp, animals) {
  for (const a of animals) {
    const m = animalModels[a.species] ??= models.addModel(animalBoxes(a.species));
    const { model, tint } = animalPose(a);
    models.drawModel(mvp, model, m, tint);
  }
}

// Fish as textured boxes, drawn opaque before the water pass so the water
// blends over them. One model per species and body height for the body and
// dorsal fin, one for the tail plate, which swings on its own (fishDraw).
const fishModels = {};
const tailTracker = createTailTracker();
function drawFish(mvp, fish, timeSeconds) {
  const swings = tailTracker.update(fish, timeSeconds);
  for (const { species, height, part, model, tint } of fishDraw(fish, swings)) {
    const m = fishModels[`${species}/${height}/${part}`] ??= models.addModel(
      part === 'tail' ? fishBoxes(species, height).slice(2) : fishBoxes(species, height).slice(0, 2));
    models.drawModel(mvp, model, m, tint);
  }
}

// Loose items as camera-facing icon slabs, one model per item name.
const itemModels = {};
function drawItems(mvp, items, timeSeconds, camera) {
  for (const { key, model } of itemDraws(items, timeSeconds, camera)) {
    models.drawModel(mvp, model, itemModels[key] ??= models.addModel(itemBoxes(key)));
  }
}

// The held item, in camera space after everything else: the depth buffer is
// cleared first, so it never clips into blocks in front of the player.
const heldModels = {};
function drawHeld(proj) {
  const d = heldDraw(controller.state.belt.held, controller.state.belt.swing, controller.state.belt.use);
  if (!d) return;
  gl.clear(gl.DEPTH_BUFFER_BIT);
  gl.disable(gl.BLEND); gl.enable(gl.DEPTH_TEST); gl.enable(gl.CULL_FACE); gl.depthMask(true);
  models.drawModel(proj, d.model, heldModels[d.key] ??= models.addModel(heldBoxes(d.key)));
}

// `box` is [minx, miny, minz, maxx, maxy, maxz]; entities also get a faint fill.
// `fill` 'flake' is a snow flake: a solid white fill and no outline.
function drawBox(mvp, box, fill) {
  const grow = 0.003; // keeps the lines off the block's own faces
  const lo = [0, 1, 2].map((i) => box[i] - grow);
  const size = [0, 1, 2].map((i) => box[i + 3] - box[i] + 2 * grow);
  gl.useProgram(boxProg);
  gl.uniformMatrix4fv(boxProg.u('mvp'), false, mvp);
  gl.uniform3fv(boxProg.u('lo'), lo);
  gl.uniform3fv(boxProg.u('size'), size);
  gl.uniformMatrix4fv(boxProg.u('model'), false, IDENTITY);
  gl.bindVertexArray(unitBox.vao);
  if (fill) {
    gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false); gl.disable(gl.CULL_FACE);
    gl.uniform4f(boxProg.u('color'), 1, 1, 1, fill === 'flake' ? 0.9 : 0.22);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, unitBox.quadIB);
    gl.drawElements(gl.TRIANGLES, unitBox.quads, gl.UNSIGNED_BYTE, 0);
    gl.depthMask(true); gl.enable(gl.CULL_FACE); gl.disable(gl.BLEND);
    if (fill === 'flake') { gl.bindVertexArray(null); return; }
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

// Work done this frame, kept with the frame time when the frame is a hitch.
let work = { meshed: 0, remeshed: 0, verts: 0, ticks: 0, chunks: 0 };
const spans = createSpans();
window.perfSpans = spans.take;
const upload = (buf, offset) => spans.span('upload', () => uploadBuffers(buf, offset));
function uploadBuffers({ verts, faces }, offset) {
  if (!faces) return null;
  work.verts += verts.byteLength / VERTEX_BYTES;
  const ib = quadIndexBuffer(faces);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
  const attr = (loc, size, type, normalized, off) => {
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, size, type, normalized, VERTEX_BYTES, off);
  };
  attr(0, 3, gl.SHORT, false, 0); attr(1, 2, gl.UNSIGNED_BYTE, false, 6); attr(2, 1, gl.UNSIGNED_BYTE, false, 8);
  attr(3, 1, gl.UNSIGNED_BYTE, false, 9); attr(4, 3, gl.UNSIGNED_BYTE, true, 10);
  attr(5, 1, gl.UNSIGNED_BYTE, true, 14); attr(6, 1, gl.UNSIGNED_BYTE, false, 13); attr(7, 1, gl.UNSIGNED_BYTE, false, 15);
  gl.bindVertexArray(null);
  return { vao, buf, n: faces * 6, offset };
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
const scene = raftScene ? new Scene() : null;
// Terrain streams around the player (data/streaming.toml); the raft scene is one fixed sea.
const walk = scene ? Walk.in_scene(scene) : Walk.streamed(seed, CHUNK / 2, 127, CHUNK / 2);
if (!scene) { // stand on the highest block of the spawn column
  let top = 127;
  while (top > 0 && !walk.block_at(CHUNK / 2, top, CHUNK / 2)) top--;
  if (top > 0) walk.teleport(CHUNK / 2, top + 1.5, CHUNK / 2); // 0: region not loaded yet, keep the start
}
const surfaceY = walk.feet()[1];
const loadRadius = Walk.load_radius();
if (scene) { spawnTestItems(walk); spawnTestAnimals(walk); }
const input = attachInput({ sampler, canvas, document, window, debug });

const names = [];
const legendJson = terrain.legend();
for (const b of JSON.parse(legendJson).blocks) names[b.id] = b.name;
const faceLayer = await loadTiles(gl, B, names, '../viewer/textures.json');
const ctx = legendContext(legendJson, faceLayer, variantTable(LAYER_NAMES, VARIANTS), frontLayers(B));
// Textured-model drawer (modelgl.js); nothing is drawn with it yet. Without the
// export it still builds, with every face grey.
const tileset = await fetch('../viewer/textures.json').then((r) => r.json()).catch(() => ({ palette: {}, batches: {} }));
const models = createModelDrawer(gl, tileset, SCENE_TEXTURES);
mountPanelStyle(document, tileset, '#overlay, #console-input');
const hud = mountHud(document, tileset);
const inventoryView = mountInventory(document, tileset);

// With MSAA, cutout edges use alpha-to-coverage on a sharpened alpha so distant
// leaves fade to partial coverage instead of popping in and out; without it the
// cutout shader falls back to a hard discard.
const msaa = gl.getParameter(gl.SAMPLES) > 0;
const solidProg = program(false), cutProg = program(true);
const cut = (draw) => {
  if (msaa) gl.enable(gl.SAMPLE_ALPHA_TO_COVERAGE);
  draw();
  if (msaa) gl.disable(gl.SAMPLE_ALPHA_TO_COVERAGE);
};
let skyColor = [0.61, 0.76, 0.9];
let skyTarget = skyColor, skySampleAt = 0, skyReady = false;
// The sky the player stands under: the mean over a small window of columns around the eye.
function sampleSkyAt(eye) {
  const n = 2 * SKY_SAMPLE_RADIUS + 1;
  const x0 = Math.floor(eye[0]) - SKY_SAMPLE_RADIUS, z0 = Math.floor(eye[2]) - SKY_SAMPLE_RADIUS;
  return sampleSky(terrain.biome_map(seed, x0, z0, n, n), terrain.temperature_map(seed, x0, z0, n, n), ctx.BIOMES, ramp3);
}
const meshes = { opaque: [], cutout: [], water: [], ice: [] };

let raftModel = null; // the raft's boxes; moved each frame by the sim's raft offset
// Meshes of each streamed region, by "rx,rz", so they can be freed when it unloads.
const regionParts = new Map();
// World cells of the snow on swaying foliage in each region, by "rx,rz", which shake off flakes.
const regionSnow = new Map();
let snowCells = [];
const setSnow = (key, cells) => {
  if (cells?.length) regionSnow.set(key, cells); else regionSnow.delete(key);
  snowCells = [...regionSnow.values()].flat();
};
const freeParts = (m) => { gl.deleteVertexArray(m.vao); gl.deleteBuffer(m.buf); };
const meshRegion = (r, remesh) => spans.span('mesh', () => meshRegionNow(r, remesh));
function meshRegionNow({ rx, rz }, remesh = false) {
  work[remesh ? 'remeshed' : 'meshed']++;
  const { mesh: m, blocks, tints, origin: [x, z] } = meshRegionCpu({ walk, terrain, ctx, seed, blendRadius }, { rx, rz });
  if (rx === 0 && rz === 0 && !skyReady) skyColor = skyTarget = ramp3(tints.centre.biome.sky, tints.centre.t).map((v) => v / 255);
  const parts = {};
  for (const k of Object.keys(meshes)) {
    const part = upload(m[k], [x, 0, z]);
    parts[k] = part ? [part] : [];
    if (part) meshes[k].push(part);
  }
  regionParts.set(`${rx},${rz}`, parts);
  setSnow(`${rx},${rz}`, snowOnFoliage(blocks, REGION_SIZE, 1, [x, z]));
}
function freeRegion({ rx, rz }) {
  const parts = regionParts.get(`${rx},${rz}`);
  regionParts.delete(`${rx},${rz}`);
  setSnow(`${rx},${rz}`, null);
  for (const k of Object.keys(parts ?? {})) {
    for (const p of parts[k]) freeParts(p);
    meshes[k] = meshes[k].filter((m) => !parts[k].includes(m));
  }
}

// The test scene is one sea region plus the raft's blocks meshed apart, so the
// raft can move over the water. Biome tints come from the terrain under it.
let remeshFarm = () => {};
if (scene) {
  const { min, size } = scene;
  const tints = columnTints(terrain, ctx, seed, min, min, size, blendRadius);
  skyColor = ramp3(tints.centre.biome.sky, tints.centre.t).map((v) => v / 255);
  const offset = [min, 0, min];
  const seaBlocks = scene.sea_blocks();
  // The sea with the farm overlay applied; meshed again whenever a tile changes.
  const meshSea = () => {
    const blocks = seaBlocks.slice();
    const t = walk.farm_tiles();
    for (let i = 0; i < t.length; i += 4) blocks[((t[i + 2] - min) * size + t[i] - min) * 128 + t[i + 1]] = t[i + 3];
    const m = meshBlocksWasm(terrain, ctx, blocks, tints, size, 0, { origin: [min, min] });
    for (const k of Object.keys(meshes)) {
      for (const old of seaParts[k]) { gl.deleteVertexArray(old.vao); gl.deleteBuffer(old.buf); }
      meshes[k] = seaParts[k] = [upload(m[k], offset)].filter(Boolean);
    }
  };
  const seaParts = { opaque: [], cutout: [], water: [], ice: [] };
  let farmSig = null;
  remeshFarm = () => {
    const sig = walk.farm_tiles().join();
    if (sig === farmSig) return;
    farmSig = sig;
    meshSea();
  };
  raftModel = models.addModel(raftBoxes(scene.raft_blocks(), size));
  remeshFarm();
}

// ---------------------------------------------------------------- loop
const clock = new SimClock();
const fly = new FlyCamera(CHUNK / 2, surfaceY + 24, CHUNK / 2);
if (scene) {
  const [x, y, z] = walk.eye(1);
  fly.teleport(x, y + 24, z);
}
const controller = createController({ input, walk, fly, clock, document, flying, stream: scene ? null : { budget: 1 },
  sprintFov: { base: sampler.fov_degrees, extra: sampler.sprint_fov_extra_degrees, seconds: sampler.sprint_fov_ease_seconds } });
const deathView = mountDeath(document, tileset, controller);
const hint = mountHint(document, tileset);
const cue = mountCue(document, tileset);
// The debug readout folds into a top-left button: open by default only in debug mode, and the choice is remembered.
let overlayOpen = debug;
try { overlayOpen = JSON.parse(localStorage.getItem('overlayOpen') ?? String(debug)) === true; } catch { /* blocked storage: use the default */ }
const overlayButton = mountCornerButton(document, tileset, { id: 'overlay-button', corner: 'top-left', label: '-', onClick() {
  overlayOpen = !overlayOpen;
  try { localStorage.setItem('overlayOpen', String(overlayOpen)); } catch { /* blocked storage */ }
  showOverlay();
} });
function showOverlay() {
  overlayButton.setScale(hintScale(innerWidth, innerHeight));
  overlayButton.setLabel(overlayOpen ? '-' : 'info');
  overlay.hidden = !overlayOpen;
  overlay.style.top = `${CORNER_MARGIN + overlayButton.h + 4}px`;
}
showOverlay();
addEventListener('resize', showOverlay);
const frames = new FrameTimes(300);
let lastFrame = null, overlayAt = 0;
const outputBuffer = new OutputBuffer();
let storage = null;
try { storage = localStorage; } catch { /* blocked storage: history stays in memory */ }
const consoleUI = debug && createConsoleUI({
  document,
  buffer: outputBuffer,
  history: new History(storage),
  complete,
  names: nameLists(game),
  onSubmit(line) {
    outputBuffer.echo(line, performance.now());
    controller.submitConsole(line);
  },
  onCancel: () => controller.closeConsole(),
});
if (consoleUI) window.consoleOutput = () => outputBuffer.lines; // every line this session, for scripts
window.frameStats = () => ({ ...frames.summary(), records: frames.hitchRecords() });

function drawParts(prog, parts) {
  for (const m of parts) {
    gl.uniform3fv(prog.u('offset'), m.offset);
    gl.bindVertexArray(m.vao);
    gl.drawElements(gl.TRIANGLES, m.n, gl.UNSIGNED_INT, 0);
  }
}

const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');

function render(now) {
  work = { meshed: 0, remeshed: 0, verts: 0, ticks: 0, chunks: 0 };
  remeshFarm();

  const snow = reducedMotion.matches ? null : { cells: snowCells, phase: swayPhase(now, false) };
  const { ticks, alpha, eye, yaw, pitch, fov, flying, aim, underwater, regions, particles } = spans.span('tick', () => controller.frame(lastFrame === null ? 0 : (now - lastFrame) / 1000, snow));
  work.ticks = ticks;
  if (regions) {
    work.chunks = regions.chunks;
    regions.free.forEach(freeRegion);
    regions.mesh.forEach((r) => meshRegion(r));
    // An edit changed these regions' blocks, or a new neighbour left them stale: mesh them again.
    for (const r of regions.remesh) {
      freeRegion(r);
      meshRegion(r, true);
    }
  }

  const dpr = window.devicePixelRatio || 1;
  const w = Math.round(canvas.clientWidth * dpr), h = Math.round(canvas.clientHeight * dpr);
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  gl.viewport(0, 0, w, h);
  const fogFar = (loadRadius + 0.5) * REGION_SIZE * 1.1;
  const dtSky = lastFrame === null ? 0 : (now - lastFrame) / 1000;
  if (now - skySampleAt > 250) { skySampleAt = now; skyTarget = sampleSkyAt(eye); if (!skyReady) skyColor = skyTarget; skyReady = true; }
  skyColor = easeSky(skyColor, skyTarget, dtSky);
  const view = look({ underwater, skyColor, fogFar, water: ctx.COLORS[B.water] });
  gl.clearColor(...view.fogColor, 1);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  const sky = skyInputs(view, yaw, pitch, fov * Math.PI / 180, w / h, controller.state.day?.sunDir);
  if (sky) {
    gl.useProgram(skyProg);
    gl.uniform3fv(skyProg.u('camRight'), sky.right);
    gl.uniform3fv(skyProg.u('camUp'), sky.up);
    gl.uniform3fv(skyProg.u('camFwd'), sky.fwd);
    gl.uniform2fv(skyProg.u('skyScale'), sky.scale);
    gl.uniform3fv(skyProg.u('fogColor'), sky.base);
    gl.uniform3fv(skyProg.u('sunDir'), sky.sunDir);
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.disable(gl.BLEND);
    gl.bindVertexArray(skyVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.bindVertexArray(null);
  }
  gl.enable(gl.DEPTH_TEST); gl.enable(gl.CULL_FACE); gl.disable(gl.BLEND);

  const proj = new Float32Array(perspective(fov * Math.PI / 180, w / h, NEAR, farPlane(fogFar)));
  const mvp = new Float32Array(mul(
    proj,
    view_matrix(eye[0], eye[1], eye[2], yaw, pitch),
  ));
  const bind = (prog, a) => {
    gl.useProgram(prog);
    const s = prog === cutProg ? cutSampler : solidSampler;
    gl.bindSampler(0, s); gl.bindSampler(1, s);
    gl.uniformMatrix4fv(prog.u('mvp'), false, mvp);
    gl.uniform3fv(prog.u('unpack'), [POS_SCALE, UV_SCALE, SHADE_SCALE]);
    gl.uniform3fv(prog.u('fogColor'), view.fogColor);
    gl.uniform3fv(prog.u('sunDir'), sky ? sky.sunDir : [0, 1, 0]);
    gl.uniform1f(prog.u('fogStart'), view.fogStart);
    gl.uniform1f(prog.u('fogEnd'), view.fogEnd);
    gl.uniform1f(prog.u('fogFar'), view.fogFar);
    gl.uniform3fv(prog.u('eye'), eye);
    gl.uniform3fv(prog.u('tint'), view.tint);
    gl.uniform1f(prog.u('alpha'), a);
    gl.uniform1f(prog.u('waterFresnel'), 0);
    gl.uniform1f(prog.u('waterFrame'), waterFrame(now, reducedMotion.matches));
    gl.uniform1f(prog.u('swayPhase'), swayPhase(now, reducedMotion.matches));
    if (prog === cutProg) gl.uniform2f(prog.u('plantFade'), plantDistance, PLANT_FADE_RANGE);
  };
  bind(solidProg, 1); drawParts(solidProg, meshes.opaque);
  bind(cutProg, 1); cut(() => drawParts(cutProg, meshes.cutout));
  if (scene) {
    const [dx, dy, dz] = walk.raft_offset(flying ? 1 : alpha);
    models.drawModel(mvp, raftMatrix([dx, dy, dz], walk.raft_heading(flying ? 1 : alpha), walk.raft_pivot(), scene.min), raftModel);
  }
  drawFish(mvp, readFish(walk), now / 1000);
  gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.depthMask(false);
  // Water faces point out of the water, so from below the surface they are
  // back faces: draw both sides while the eye is under water.
  if (underwater) gl.disable(gl.CULL_FACE);
  bind(solidProg, 0.45);
  gl.uniform1f(solidProg.u('waterFresnel'), underwater ? 0 : 1);
  drawParts(solidProg, meshes.water);
  gl.enable(gl.CULL_FACE);
  bind(solidProg, 0.72); drawParts(solidProg, meshes.ice);
  gl.depthMask(true);
  gl.bindVertexArray(null);
  if (scene) drawAnimals(mvp, readAnimals(walk));
  drawItems(mvp, readItems(walk), now / 1000, { yaw });
  if (!flying && aim.box) drawBox(mvp, aim.box, aim.kind === 'entity');
  for (const { pos, size, snow } of particles) drawBox(mvp, [...pos.map((v) => v - size), ...pos.map((v) => v + size)], snow ? 'flake' : true);
  if (!flying) drawHeld(proj);
  inventoryView.update(controller.state.inventory, controller.state.belt);
  deathView.update(controller.state.dead);
  hud.update(flying ? { ...controller.state.hud, crosshair: false } : controller.state.hud);
  hint.update(input.locked);
  cue.update(waitingForClick(input));

  if (lastFrame !== null) frames.push(now - lastFrame, work);
  lastFrame = now;
  if (consoleUI) {
    for (const text of walk.drain_command_output()) outputBuffer.output(text, now);
    consoleUI.update(input.console, now);
  }
  if (now - overlayAt > 250) {
    overlayAt = now;
    const p99 = frames.percentile(99);
    const hitch = frames.formatLastHitch();
    overlay.textContent = `${frames.format()}\n${p99 > FRAME_BUDGET_MS ? 'over' : 'within'} the 16.7 ms p99 budget\n` +
      (hitch ? `${hitch}\n` : '') +
      `${flying ? 'fly' : 'walk'} (F to switch)${underwater ? ', under water' : ''}
cheated: ${walk.cheated() ? 'yes' : 'no'}
${formatTime(walk.time_of_day(), walk.day_frozen())}
pos ${eye.map((v) => v.toFixed(1)).join(' ')}${flying ? "" : `
${formatBelt(controller.state.belt)}
${formatAim(aim)}`}`;
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
}
requestAnimationFrame(render);
