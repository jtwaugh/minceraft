"""Build the texture sheet page from every batches_*.json file.

Run from the art/ directory: python3 tools/build_sheet.py [OUT.html]
The default output is build/sheet.html.
"""
import glob
import json
import os
import re
import sys


def first_batch_number(path):
    return int(re.search(r"batches_(\d+)", path).group(1))


data = {}
for f in sorted(glob.glob("batches_*.json"), key=first_batch_number):
    data.update(json.load(open(f)))

TEMPLATE = r"""<title>Mince Raft Textures</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Silkscreen&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Layout: a texture atlas sheet; batches stacked, tiles in a wrapping grid, models as preview + face strip */
:root {
  --bg: #e9eef2; --panel: #f7f9fb; --fg: #1c2530; --muted: #5d6b78; --line: #c9d3dc; --accent: #2b4fa8;
  --chk-a: #d7dee5; --chk-b: #eef2f5; --night: #1d2a44;
  --display: "Silkscreen", "Courier New", monospace;
  --mono: "IBM Plex Mono", ui-monospace, Menlo, monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #12171d; --panel: #1a2129; --fg: #e3e9ef; --muted: #8b99a6; --line: #2c3742; --accent: #7fa6ef;
  --chk-a: #232c36; --chk-b: #1b222a; --night: #0d1424; color-scheme: dark } }
:root[data-theme="dark"] {
  --bg: #12171d; --panel: #1a2129; --fg: #e3e9ef; --muted: #8b99a6; --line: #2c3742; --accent: #7fa6ef;
  --chk-a: #232c36; --chk-b: #1b222a; --night: #0d1424; color-scheme: dark }
body { background: var(--bg); color: var(--fg); font-family: var(--mono); font-size: 13px;
  padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); padding-block: 28px 48px; }
.wrap { max-width: 1100px; margin: 0 auto; display: flex; flex-direction: column; gap: 32px; }
header { display: flex; flex-wrap: wrap; align-items: end; justify-content: space-between; gap: 16px; }
h1 { font-family: var(--display); font-weight: 400; font-size: 32px; margin: 0; letter-spacing: .02em; }
h1 small { display: block; font-family: var(--mono); font-size: 12px; color: var(--muted); letter-spacing: .04em; margin-top: 4px; }
.controls { display: flex; gap: 8px; flex-wrap: wrap; }
.controls button { font: inherit; font-size: 12px; padding: 6px 10px; border: 1px solid var(--line); background: var(--panel); color: var(--fg); cursor: pointer; }
.controls button[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); }
.controls button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.palette { display: flex; flex-wrap: wrap; gap: 6px; }
.sw { display: flex; align-items: center; gap: 6px; padding: 3px 8px 3px 3px; border: 1px solid var(--line); background: var(--panel); font-size: 11px; }
.sw i { width: 16px; height: 16px; display: block; }
nav { display: flex; flex-wrap: wrap; gap: 6px 14px; font-size: 12px; }
nav a { color: var(--accent); text-decoration: none; }
nav a:hover { text-decoration: underline; }
section h2 { font-family: var(--display); font-weight: 400; font-size: 16px; margin: 0 0 12px; display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }
section h2 span { font-family: var(--mono); font-size: 12px; color: var(--muted); }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 10px; }
.tile { background: var(--panel); border: 1px solid var(--line); padding: 10px 8px 8px; display: flex; flex-direction: column; align-items: center; justify-content: end; gap: 6px; min-width: 0; }
.tile.wide { grid-column: span 2; }
canvas.px { image-rendering: pixelated; max-width: 100%;
  background: repeating-conic-gradient(var(--chk-a) 0 25%, var(--chk-b) 0 50%) 0 0 / 10px 10px; }
canvas.night { background: var(--night); }
.name { font-size: 11px; color: var(--muted); text-align: center; overflow-wrap: anywhere; }
.name b { font-weight: 500; color: var(--fg); }
.err { color: #c0392b; font-size: 10px; }
.model { background: var(--panel); border: 1px solid var(--line); padding: 14px; display: grid; grid-template-columns: minmax(180px, 260px) 1fr; gap: 16px; align-items: start; }
.model + .model { margin-top: 10px; }
.model .preview { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; }
.model .preview canvas { image-rendering: pixelated; max-width: 100%; height: auto; }
.model h3 { font-family: var(--display); font-weight: 400; font-size: 14px; margin: 0; }
.faces { display: flex; flex-wrap: wrap; gap: 10px 12px; align-items: end; min-width: 0; }
.face { display: flex; flex-direction: column; align-items: center; gap: 4px; }
.face .name { font-size: 10px; }
@media (max-width: 600px) { .model { grid-template-columns: 1fr; } }
</style>

<div class="wrap">
  <header>
    <h1>Mince Raft<small>pixel textures · batches 1–12 · 16-color palette</small></h1>
    <div class="controls">
      <button id="tileBtn" aria-pressed="false">Tile terrain 3×3</button>
      <button id="animBtn" aria-pressed="true">Animate</button>
      <button id="spinBtn" aria-pressed="true">Turn models</button>
    </div>
  </header>
  <div class="palette" id="palette"></div>
  <nav id="nav"></nav>
  <div id="sheet" style="display:flex;flex-direction:column;gap:32px"></div>
</div>

<script>
const PAL = {"0":"#1a1a1a","1":"#4a4a4a","2":"#7f7f7f","3":"#b0b0b0","4":"#5c3b1e","5":"#8b5a2b","6":"#b8864b","7":"#e3d39b","8":"#2f5e1e","9":"#5c9e31","a":"#2b4fa8","b":"#4a7fd6","c":"#c0392b","d":"#f08a5d","e":"#f4f1e6","f":"#e8c33a"};
const NAMES = {"0":"near black","1":"dark gray","2":"stone gray","3":"light gray","4":"dark brown","5":"brown","6":"tan","7":"sand","8":"dark green","9":"grass green","a":"deep water","b":"water","c":"red","d":"salmon","e":"off-white","f":"yellow"};
const DATA = __DATA__;

const pal = document.getElementById('palette');
for (const k in PAL) pal.insertAdjacentHTML('beforeend', `<span class="sw"><i style="background:${PAL[k]}"></i>${k} ${NAMES[k]}</span>`);

let tiled = false, animate = true, spin = true;
if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
  animate = spin = false;
  document.getElementById('animBtn').setAttribute('aria-pressed', 'false');
  document.getElementById('spinBtn').setAttribute('aria-pressed', 'false');
}

function validate(rows) {
  const w = rows[0] ? rows[0].length : 0;
  for (const r of rows) { if (r.length !== w) return `ragged row`; for (const ch of r) if (ch !== '.' && !PAL[ch]) return `bad '${ch}'`; }
  return null;
}
// Paint rows onto a canvas, repeated reps×reps.
function paint(cv, rows, reps = 1) {
  const w = rows[0].length, h = rows.length;
  cv.width = w * reps; cv.height = h * reps;
  const ctx = cv.getContext('2d'); ctx.clearRect(0, 0, cv.width, cv.height);
  for (let ty = 0; ty < reps; ty++) for (let tx = 0; tx < reps; tx++)
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const ch = rows[y][x]; if (ch === '.') continue;
      ctx.fillStyle = PAL[ch]; ctx.fillRect(tx * w + x, ty * h + y, 1, 1);
    }
}
const FACE_CACHE = new Map();
function faceCanvas(rows) { const k = rows.join('/'); let c = FACE_CACHE.get(k); if (!c) { c = document.createElement('canvas'); paint(c, rows); FACE_CACHE.set(k, c); } return c; }
function scaleFor(w, h) { const m = Math.max(w, h); return m <= 4 ? 12 : m <= 10 ? 10 : m <= 12 ? 8 : 6; }

const sheet = document.getElementById('sheet');
const nav = document.getElementById('nav');
const tileable = [];   // {cv, rows, tile}
const animators = [];  // {cv, frames, i, reps?}

function makeTile(grid, name, rows, opts = {}) {
  const t = document.createElement('div'); t.className = 'tile' + (opts.wide ? ' wide' : '');
  const cv = document.createElement('canvas'); cv.className = 'px' + (opts.night ? ' night' : '');
  paint(cv, rows);
  const s = opts.scale || scaleFor(rows[0].length, rows.length);
  cv.style.width = rows[0].length * s + 'px';
  const err = validate(rows);
  t.append(cv);
  t.insertAdjacentHTML('beforeend', `<div class="name">${name}</div>` + (err ? `<div class="err">${err}</div>` : ''));
  grid.append(t);
  return { cv, tile: t, s };
}

// Animated sequences shown as an extra tile after their frames.
const SEQUENCES = {
  water_3: ['water (animated)', ['water_0', 'water_1', 'water_2', 'water_3'], 350, true],
  water_flow_3: ['water_flow (animated)', ['water_flow_0', 'water_flow_1', 'water_flow_2', 'water_flow_3'], 180, true],
  splash_3: ['splash (animated)', ['splash_0', 'splash_1', 'splash_2', 'splash_3'], 160],
  puff_7: ['puff (animated)', ['puff_0','puff_1','puff_2','puff_3','puff_4','puff_5','puff_6','puff_7'], 110],
  moon_7: ['moon (cycling)', ['moon_0','moon_1','moon_2','moon_3','moon_4','moon_5','moon_6','moon_7'], 600],
};

function buildSheet() {
for (const [label, batch] of Object.entries(DATA)) {
  const id = label.toLowerCase().replace(' ', '-');
  const sec = document.createElement('section'); sec.id = id;
  const count = Object.keys(batch.textures || batch.models).length;
  sec.innerHTML = `<h2>${batch.title}<span>${label} · ${count} ${batch.models ? 'models' : 'textures'}</span></h2>`;
  nav.insertAdjacentHTML('beforeend', `<a href="#${id}">${label.replace('Batch ', '')} ${batch.title}</a>`);
  if (batch.textures) {
    const grid = document.createElement('div'); grid.className = 'grid'; sec.append(grid);
    const night = batch.background === 'night';
    for (const [name, rows] of Object.entries(batch.textures)) {
      const o = makeTile(grid, name, rows, { night });
      if (batch.tiles) tileable.push({ ...o, rows });
      const seq = SEQUENCES[name];
      if (seq) {
        const frames = seq[1].map(n => batch.textures[n]);
        const a = makeTile(grid, `<b>${seq[0]}</b>`, frames[0], { night });
        const anim = { cv: a.cv, frames, i: 0, ms: seq[2], t: 0, tiles: !!seq[3] };
        animators.push(anim);
        if (seq[3]) tileable.push({ ...a, rows: frames[0], anim });
      }
    }
    if (label === 'Batch 10') inventoryMock(grid, batch.textures);
  } else {
    for (const [name, faces] of Object.entries(batch.models)) sec.append(modelCard(name, faces));
  }
  sheet.append(sec);
}
}

// ---------------------------------------------------------------- inventory mock
function allTextures() { const o = {}; for (const b of Object.values(DATA)) Object.assign(o, b.textures || {}); return o; }
function inventoryMock(grid, T) {
  const ALL = allTextures();
  const cols = 9, rowsN = 3, W = 4 + 3 + cols * 10 + 3 + 4, H = 4 + 3 + rowsN * 10 + 3 + 4;
  const cv = document.createElement('canvas'); cv.width = W; cv.height = H; cv.className = 'px';
  const ctx = cv.getContext('2d');
  const blit = (rows, x, y) => { for (let j = 0; j < rows.length; j++) for (let i = 0; i < rows[j].length; i++) { const ch = rows[j][i]; if (ch === '.') continue; ctx.fillStyle = PAL[ch]; ctx.fillRect(x + i, y + j, 1, 1); } };
  // nine-slice panel
  for (let y = 0; y < H; y += 4) for (let x = 0; x < W; x += 4) {
    const top = y === 0, bot = y >= H - 4, left = x === 0, right = x >= W - 4;
    const k = (top ? 't' : bot ? 'b' : '') + (left ? 'l' : right ? 'r' : '');
    blit(T['panel_' + (k || 'c')], Math.min(x, W - 4), Math.min(y, H - 4));
  }
  const items = [['raw_salmon', 12], ['raw_tuna', 3], ['raw_mackerel', 7], ['raw_squid', 2], ['rice_grains', 64], ['nori', 16], ['cucumber', 5], ['knife', 1], ['fishing_rod', 1],
                 ['salmon_nigiri', 4], ['tuna_nigiri', 2], ['cucumber_maki', 8], ['salmon_sashimi', 3], null, ['seeds', 23], ['wheat', 9], ['bread', 2], ['bucket_water', 1],
                 ['seaweed', 31], null, null, ['rice_bowl', 1], null, ['hoe', 1], null, ['bucket', 1], null];
  items.forEach((it, i) => {
    const sx = 7 + (i % cols) * 10, sy = 7 + Math.floor(i / cols) * 10;
    blit(i === 0 ? T.slot_selected : T.slot, sx, sy);
    if (!it) return;
    blit(ALL[it[0]], sx + 1, sy + 1);
    if (it[1] > 1) {
      const s = String(it[1]);
      let dx = sx + 10 - s.length * 4 + 1;
      for (const ch of s) { blit(T['digit_' + ch], dx, sy + 5); dx += 4; }
    }
  });
  const t = document.createElement('div'); t.className = 'tile wide'; t.style.gridColumn = '1 / -1';
  cv.style.width = Math.min(W * 5, 600) + 'px';
  t.append(cv);
  t.insertAdjacentHTML('beforeend', `<div class="name"><b>inventory mock-up</b>: panel, slots and digits with items from batches 4–6</div>`);
  grid.append(t);
}

// ---------------------------------------------------------------- models
// Boxes: [x, y, z, W, H, D, {front, back, left, right, top, bottom}] in model units.
// Creature faces +z; its left side is +x. Planes: {plane:'x', x, y, z, w, h, tex} drawn in the x = const plane, texture x runs toward -z.
function fishRig(f, W, H, D) {
  const [tw, th] = [f.tail[0].length, f.tail.length], [dw, dh] = [f.dorsal[0].length, f.dorsal.length];
  return [
    { box: [0, 0, 0, W, H, D], tex: { front: f.body_front, back: f.body_back, left: f.body_left, right: mirror(f.body_left), top: f.body_top, bottom: f.body_bottom } },
    { plane: [W / 2, H / 2 + th / 2, 0], tex: f.tail, w: tw, h: th },
    { plane: [W / 2, H + dh, D * 0.7], tex: f.dorsal, w: dw, h: dh },
  ];
}
function mirror(rows) { return rows.map(r => [...r].reverse().join('')); }
const RIGS = {
  salmon: f => ({ parts: fishRig(f, 2, 4, 8), scale: 16 }),
  tuna: f => ({ parts: fishRig(f, 3, 5, 10), scale: 13 }),
  mackerel: f => ({ parts: fishRig(f, 2, 3, 7), scale: 18 }),
  squid: f => {
    const parts = [];
    const T = { front: f.tentacle_side, back: f.tentacle_side, left: f.tentacle_side, right: f.tentacle_side, top: f.tentacle_end, bottom: f.tentacle_end };
    const spots = [[1, 1], [5, 0], [9, 1], [10, 5], [9, 9], [5, 10], [1, 9], [0, 5]];
    for (const [x, z] of spots) parts.push({ box: [x, 0, z, 2, 18, 2], tex: T });
    parts.push({ box: [0, 18, 0, 12, 16, 12], tex: { front: f.body_front, back: f.body_back, left: f.body_left, right: f.body_right, top: f.body_top, bottom: f.body_bottom } });
    return { parts, scale: 5 };
  },
  player: f => {
    const limb = (p, outerIsLeft) => ({ front: f[p + '_front'], back: f[p + '_back'], left: outerIsLeft ? f[p + '_outer'] : f[p + '_inner'], right: outerIsLeft ? f[p + '_inner'] : f[p + '_outer'], top: f[p + '_top'], bottom: f[p + '_bottom'] });
    const pick = p => ({ front: f[p + '_front'], back: f[p + '_back'], left: f[p + '_left'], right: f[p + '_right'], top: f[p + '_top'], bottom: f[p + '_bottom'] });
    return { scale: 6, parts: [
      { box: [-4, 12, 0, 4, 12, 4], tex: limb('arm', false) },
      { box: [0, 0, 0, 4, 12, 4], tex: limb('leg', false) },
      { box: [4, 0, 0, 4, 12, 4], tex: limb('leg', true) },
      { box: [0, 12, 0, 8, 12, 4], tex: pick('body') },
      { box: [8, 12, 0, 4, 12, 4], tex: limb('arm', true) },
      { box: [0, 24, -2, 8, 8, 8], tex: pick('head') },
    ] };
  },
  villager: f => {
    const pick = p => ({ front: f[p + '_front'], back: f[p + '_back'], left: f[p + '_left'], right: f[p + '_right'], top: f[p + '_top'], bottom: f[p + '_bottom'] });
    const upper = left => ({ front: f.upper_front, back: f.upper_back, left: left ? f.upper_outer : f.upper_inner, right: left ? f.upper_inner : f.upper_outer, top: f.upper_top, bottom: f.upper_bottom });
    const leg = left => ({ front: f.leg_front, back: f.leg_back, left: left ? f.leg_outer : f.leg_inner, right: left ? f.leg_inner : f.leg_outer, top: f.leg_top, bottom: f.leg_bottom });
    const robeTop = f.body_top;
    return { scale: 5, parts: [
      { box: [0, 0, 1, 4, 12, 4], tex: leg(false) },
      { box: [4, 0, 1, 4, 12, 4], tex: leg(true) },
      { box: [-0.5, 5.5, -0.5, 9, 18.5, 7], tex: { front: f.robe_front, back: f.robe_back, left: f.robe_left, right: f.robe_right, top: robeTop, bottom: f.robe_bottom } },
      { box: [-4, 15, 6.5, 4, 8, 4], tex: upper(false) },
      { box: [0, 15, 6.5, 8, 4, 4], tex: { front: f.cross_front, back: f.cross_back, left: f.cross_end, right: f.cross_end, top: f.cross_top, bottom: f.cross_bottom } },
      { box: [8, 15, 6.5, 4, 8, 4], tex: upper(true) },
      { box: [0, 24, -1, 8, 10, 8], tex: pick('head') },
      { box: [3, 25, 7, 2, 4, 2], tex: { front: f.nose_front, back: f.nose_front, left: f.nose_side, right: f.nose_side, top: f.nose_bottom, bottom: f.nose_bottom } },
    ] };
  },
};

// Rotate the model about the vertical axis by angle a, then project orthographically with a fixed tilt.
function renderModel(cv, rig, angle) {
  const s = rig.scale, ctx = cv.getContext('2d');
  ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, cv.width, cv.height);
  ctx.imageSmoothingEnabled = false;
  // model bounds for centring
  let cx = 0, cz = 0, maxY = 0, n = 0;
  for (const p of rig.parts) if (p.box) { const [x, y, z, W, H, D] = p.box; cx += x + W / 2; cz += z + D / 2; n++; maxY = Math.max(maxY, y + H); }
  cx /= n; cz /= n;
  const ca = Math.cos(angle), sa = Math.sin(angle), tilt = 0.5;
  const P = (x, y, z) => {
    const X = x - cx, Z = z - cz;
    const rx = X * ca + Z * sa, rz = -X * sa + Z * ca;   // rz > 0 is toward the viewer
    return { x: cv.width / 2 + rx * s, y: cv.height / 2 + (-(y - maxY / 2) * Math.cos(tilt) + rz * Math.sin(tilt)) * s, d: rz * Math.cos(tilt) + (y) * Math.sin(tilt) * 0.01 };
  };
  const quads = [];
  const add = (tex, o, u, v, w, h) => {
    // o: origin (texture top-left) in model space; u, v: unit steps along texture x and y
    const O = P(...o), U = P(o[0] + u[0], o[1] + u[1], o[2] + u[2]), V = P(o[0] + v[0], o[1] + v[1], o[2] + v[2]);
    const ux = U.x - O.x, uy = U.y - O.y, vx = V.x - O.x, vy = V.y - O.y;
    const cross = ux * vy - uy * vx;
    if (cross <= 0) return; // back-facing
    const tw = tex[0].length, th = tex.length;
    const sx = w / tw, sy = h / th;
    const C = P(o[0] + (u[0] * w + v[0] * h) / 2, o[1] + (u[1] * w + v[1] * h) / 2, o[2] + (u[2] * w + v[2] * h) / 2);
    quads.push({ d: C.d, draw: () => { ctx.setTransform(ux * sx, uy * sx, vx * sy, vy * sy, O.x, O.y); ctx.drawImage(faceCanvas(tex), 0, 0); } });
  };
  for (const p of rig.parts) {
    if (p.box) {
      const [x, y, z, W, H, D] = p.box, t = p.tex;
      add(t.front, [x, y + H, z + D], [1, 0, 0], [0, -1, 0], W, H);
      add(t.back, [x + W, y + H, z], [-1, 0, 0], [0, -1, 0], W, H);
      add(t.left, [x + W, y + H, z + D], [0, 0, -1], [0, -1, 0], D, H);
      add(t.right, [x, y + H, z], [0, 0, 1], [0, -1, 0], D, H);
      add(t.top, [x, y + H, z], [1, 0, 0], [0, 0, 1], W, D);
      add(t.bottom, [x, y, z + D], [1, 0, 0], [0, 0, -1], W, D);
    } else {
      const [x, y, z] = p.plane;
      // two-sided: left view (+x) and mirrored right view (-x)
      add(p.tex, [x, y, z], [0, 0, -1], [0, -1, 0], p.w, p.h);
      add(mirror(p.tex), [x, y, z - p.w], [0, 0, 1], [0, -1, 0], p.w, p.h);
    }
  }
  quads.sort((a, b) => a.d - b.d).forEach(q => q.draw());
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

const spinners = [];
function modelCard(name, faces) {
  const card = document.createElement('div'); card.className = 'model';
  const prev = document.createElement('div'); prev.className = 'preview';
  const rig = RIGS[name](faces);
  const cv = document.createElement('canvas'); cv.width = 240; cv.height = 240;
  prev.append(cv);
  prev.insertAdjacentHTML('beforeend', `<h3>${name}</h3>`);
  const fl = document.createElement('div'); fl.className = 'faces';
  for (const [fname, rows] of Object.entries(faces)) {
    const f = document.createElement('div'); f.className = 'face';
    const c = document.createElement('canvas'); c.className = 'px'; paint(c, rows);
    const s = Math.max(rows[0].length, rows.length) > 12 ? 5 : 7;
    c.style.width = rows[0].length * s + 'px';
    f.append(c); f.insertAdjacentHTML('beforeend', `<div class="name">${fname}</div>`);
    fl.append(f);
  }
  card.append(prev, fl);
  const sp = { cv, rig, angle: -0.6 };
  spinners.push(sp); renderModel(cv, rig, sp.angle);
  return card;
}

// ---------------------------------------------------------------- controls & loop
function redrawTiles() {
  for (const o of tileable) {
    const rows = o.anim ? o.anim.frames[o.anim.i] : o.rows;
    paint(o.cv, rows, tiled ? 3 : 1);
    o.cv.style.width = (tiled ? 96 : 80) + 'px';
  }
}
buildSheet();
let last = performance.now();
function loop(now) {
  const dt = now - last; last = now;
  if (animate) for (const a of animators) {
    a.t += dt; if (a.t < a.ms) continue; a.t = 0;
    a.i = (a.i + 1) % a.frames.length;
    paint(a.cv, a.frames[a.i], a.tiles && tiled ? 3 : 1);
  }
  if (spin) for (const sp of spinners) { sp.angle += dt * 0.0006; renderModel(sp.cv, sp.rig, sp.angle); }
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);

document.getElementById('tileBtn').onclick = e => { tiled = !tiled; e.currentTarget.setAttribute('aria-pressed', tiled); redrawTiles(); };
document.getElementById('animBtn').onclick = e => { animate = !animate; e.currentTarget.setAttribute('aria-pressed', animate); };
document.getElementById('spinBtn').onclick = e => { spin = !spin; e.currentTarget.setAttribute('aria-pressed', spin); };
</script>
"""

out = sys.argv[1] if len(sys.argv) > 1 else "build/sheet.html"
os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
open(out, "w").write(TEMPLATE.replace("__DATA__", json.dumps(data, separators=(",", ":"))))
print("wrote", out)
