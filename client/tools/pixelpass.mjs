// Pixel pass: renders one pose in headless Chrome and checks the canvas pixels against the palette
// (docs/design/play-critic-perf-render.md section 5, Phase F). Local only, advisory: prints a
// render.seams count, the crop list, and whether a second render of the same pose was byte-identical.
// Exit 2 when Chrome, puppeteer-core or the arguments are bad.
//
//   ./build.sh                                 # client/pkg and viewer/pkg must be current
//   npm install --prefix client                # puppeteer-core, a devDependency only
//   CHROME=/path/to/chrome node client/tools/pixelpass.mjs [seed] [x,y,z,yaw,pitch] [WxH] [--crops <dir>]
//
// The browser is never part of client/index.html's load path. The page is loaded with ?pixel=1 (fixed
// render, pixelmode.js); the pixels are read from the canvas, not from a screenshot. Software GL:
// SwiftShader. Pointer lock does not exist headless, so the page gets a stand-in.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { MAX_CROPS, pixelCheck } from '../pixelcheck.js';
import { PIXEL_UNIFORMS, parsePixelMode } from '../pixelmode.js';

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
/** Canvas is read every POLL_MS; the frame counts as settled once it is unchanged for STABLE_POLLS reads. */
export const POLL_MS = 500;
export const STABLE_POLLS = 4;
export const SETTLE_TIMEOUT_MS = 120000;
/** The sky pass's fixed base colour as bytes (pixel mode forces it); those pixels are not solid geometry. */
export const SKY = PIXEL_UNIFORMS.fogColor.map((v) => Math.round(v * 255));

/** `[seed] [pose] [view] [--crops dir]` plus the environment to `{ seed, pose, view, crops, chrome }`, or `{ error }`. */
export function parseArgs(argv, env = {}) {
  const positional = [];
  let crops = null;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--crops') {
      crops = argv[++i];
      if (!crops) return { error: 'usage: pixelpass.mjs [seed] [pose] [view] [--crops dir] (--crops needs a directory)' };
    } else if (argv[i].startsWith('--')) {
      return { error: `usage: pixelpass.mjs [seed] [pose] [view] [--crops dir] (unknown option "${argv[i]}")` };
    } else positional.push(argv[i]);
  }
  const [seed = '12345', pose = null, view = null, ...rest] = positional;
  if (rest.length) return { error: `usage: pixelpass.mjs [seed] [pose] [view] [--crops dir] (got extra argument "${rest[0]}")` };
  if (seed.trim() === '') return { error: 'usage: pixelpass.mjs [seed] [pose] [view] (the seed is blank)' };
  const query = new URLSearchParams({ pixel: '1' });
  if (pose !== null) query.set('pose', pose);
  if (view !== null) query.set('view', view);
  try {
    parsePixelMode(`?${query}`);
  } catch (e) {
    return { error: e.message };
  }
  return { seed, pose, view, crops, chrome: env.CHROME || 'google-chrome' };
}

/** The page URL path and query for a parsed argument set. */
export function pageQuery({ seed, pose, view }) {
  const q = new URLSearchParams({ seed, pixel: '1' });
  if (pose !== null) q.set('pose', pose);
  if (view !== null) q.set('view', view);
  return `client/index.html?${q}`;
}

/** A `pixelCheck` result and `repeatable` (true, false, or null when only one render ran) to the printed report. */
export function buildReport(result, repeatable = null) {
  const crops = result.crops.slice(0, MAX_CROPS);
  const out = [`render.seams ${result.seams}`, `off-palette: ${result.offPalette}`, `near-black: ${result.nearBlack}`, `crops: ${crops.length}`];
  for (const c of crops) out.push(`  ${c.x} ${c.y} ${c.w} ${c.h}`);
  if (repeatable !== null) out.push(`repeatable: ${repeatable ? 'yes' : 'no'}`);
  return out.join('\n');
}

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.png': 'image/png', '.css': 'text/css' };

function serve() {
  const server = http.createServer((req, res) => {
    const file = path.join(ROOT, path.normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)));
    if (!file.startsWith(ROOT) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404).end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

// Runs in the page before its scripts: a headless page cannot lock the pointer.
function fakePointerLock() {
  let locked = null;
  Object.defineProperty(document, 'pointerLockElement', { get: () => locked });
  Element.prototype.requestPointerLock = function () {
    locked = this;
    document.dispatchEvent(new Event('pointerlockchange'));
  };
  document.exitPointerLock = () => {
    locked = null;
    document.dispatchEvent(new Event('pointerlockchange'));
  };
}

// Runs in the page: the canvas as base64 RGBA, top row first.
function readCanvas() {
  const canvas = document.querySelector('canvas');
  const copy = document.createElement('canvas');
  copy.width = canvas.width;
  copy.height = canvas.height;
  const ctx = copy.getContext('2d');
  ctx.drawImage(canvas, 0, 0);
  const bytes = ctx.getImageData(0, 0, copy.width, copy.height).data;
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return { width: copy.width, height: copy.height, b64: btoa(s) };
}

/** Loads the page, waits until the canvas stops changing, and returns `{ data, width, height }`. */
async function render(browser, url) {
  const page = await browser.newPage();
  try {
    await page.evaluateOnNewDocument(fakePointerLock);
    await page.goto(url);
    await page.waitForFunction(() => window.frameStats && window.frameStats().frames > 0, { timeout: SETTLE_TIMEOUT_MS });
    // Regions mesh over several frames and the page reports no "done"; settled means unchanged for a while.
    const deadline = Date.now() + SETTLE_TIMEOUT_MS;
    let last = null, same = 0, shot;
    while (same < STABLE_POLLS) {
      if (Date.now() > deadline) throw new Error('the frame never settled; regions may still be meshing');
      await new Promise((r) => setTimeout(r, POLL_MS));
      shot = await page.evaluate(readCanvas);
      same = shot.b64 === last ? same + 1 : 0;
      last = shot.b64;
    }
    return { width: shot.width, height: shot.height, data: Buffer.from(shot.b64, 'base64') };
  } finally {
    await page.close();
  }
}

/** Writes each crop of `image` as a PNG (via the browser's canvas) into `dir`, named `crop-<n>-<x>_<y>_<w>x<h>.png`. */
async function writeCrops(browser, image, crops, dir) {
  fs.mkdirSync(dir, { recursive: true });
  const page = await browser.newPage();
  try {
    const pngs = await page.evaluate((b64, width, height, list) => {
      const src = document.createElement('canvas');
      src.width = width;
      src.height = height;
      const bin = atob(b64), px = new Uint8ClampedArray(bin.length);
      for (let i = 0; i < bin.length; i++) px[i] = bin.charCodeAt(i);
      src.getContext('2d').putImageData(new ImageData(px, width, height), 0, 0);
      return list.map((c) => {
        const out = document.createElement('canvas');
        out.width = c.w;
        out.height = c.h;
        out.getContext('2d').drawImage(src, c.x, c.y, c.w, c.h, 0, 0, c.w, c.h);
        return out.toDataURL('image/png').split(',')[1];
      });
    }, image.data.toString('base64'), image.width, image.height, crops);
    crops.forEach((c, i) => fs.writeFileSync(path.join(dir, `crop-${i}-${c.x}_${c.y}_${c.w}x${c.h}.png`), Buffer.from(pngs[i], 'base64')));
  } finally {
    await page.close();
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2), process.env);
  if (args.error) { console.error(args.error); process.exit(2); }
  let puppeteer;
  try {
    puppeteer = (await import('puppeteer-core')).default;
  } catch {
    console.error('puppeteer-core is not installed: run `npm install --prefix client`');
    process.exit(2);
  }
  let browser;
  try {
    browser = await puppeteer.launch({
      executablePath: args.chrome,
      headless: true,
      args: ['--no-sandbox', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
    });
  } catch (e) {
    console.error(`Chrome not found or failed to start (CHROME=${args.chrome}): ${e.message}`);
    process.exit(2);
  }
  const server = await serve();
  const url = `http://127.0.0.1:${server.address().port}/${pageQuery(args)}`;
  try {
    const { palette } = JSON.parse(fs.readFileSync(path.join(ROOT, 'viewer/textures.json'), 'utf8'));
    const first = await render(browser, url);
    const second = await render(browser, url);
    const result = pixelCheck(first, { palette, sky: SKY });
    console.log(buildReport(result, first.data.equals(second.data)));
    if (args.crops) await writeCrops(browser, first, result.crops.slice(0, MAX_CROPS), args.crops);
  } finally {
    await browser.close();
    server.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
