// Real frame timing in headless Chrome (docs/design/play-critic-perf-render.md section 3). Advisory:
// it prints hitch records and a verdict and exits 0 whatever the verdict; 2 when Chrome is missing.
//
//   ./build.sh                                 # client/pkg and viewer/pkg must be current
//   npm install --prefix client                # puppeteer-core, a devDependency only
//   CHROME=/path/to/chrome node client/tools/perfpass.mjs <seed>
//
// The browser is never part of client/index.html's load path. Pointer lock does not exist headless, so
// the page gets a stand-in (requestPointerLock sets pointerLockElement and fires pointerlockchange)
// and the walk is key events, not screenshots. Software GL: SwiftShader.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PERF_REPLAYS, perfVerdict } from '../perfcheck.js';

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
/** How long each replay walks forward over new ground. */
export const WALK_MS = 8000;

/** `[seed]` plus the environment to `{ seed, chrome, replays }`, or `{ error }`. */
export function parseArgs(argv, env = {}) {
  const [seed = '12345', ...rest] = argv;
  if (rest.length) return { error: `usage: perfpass.mjs [seed] (got extra argument "${rest[0]}")` };
  if (seed.trim() === '') return { error: 'usage: perfpass.mjs [seed] (the seed is blank)' };
  const raw = env.PERF_REPLAYS;
  const replays = raw === undefined || raw === '' ? PERF_REPLAYS : Number(raw);
  if (!Number.isInteger(replays) || replays < 1) return { error: `PERF_REPLAYS must be a positive integer, got "${raw}"` };
  return { seed, chrome: env.CHROME || 'google-chrome', replays };
}

/** Per-replay `{ stats, spans }` to the printed report: hitch records, spans and the verdict line. */
export function buildReport(runs) {
  const v = perfVerdict(runs.map((r) => r.stats));
  if (!v.verdict) return { ...v, text: 'perf: no replays ran' };
  const slowest = runs.reduce((a, b) => (b.stats.max > a.stats.max ? b : a));
  const spans = slowest.spans ?? [];
  const out = [v.text, 'spans (slowest replay):'];
  if (!spans.length) out.push('  none reported');
  else for (const s of spans) out.push(`  ${s.name}: ${Number(s.ms).toFixed(1)} ms`);
  out.push(`verdict: ${v.verdict} (advisory)`);
  return { ...v, text: out.join('\n') };
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

async function replay(browser, url) {
  const page = await browser.newPage();
  try {
    await page.evaluateOnNewDocument(fakePointerLock);
    await page.goto(url);
    await page.waitForFunction(() => window.frameStats && window.frameStats().frames > 0, { timeout: 120000 });
    await page.click('canvas');
    await page.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW', bubbles: true })));
    await new Promise((r) => setTimeout(r, WALK_MS));
    return await page.evaluate(() => ({
      stats: window.frameStats(),
      spans: typeof window.perfSpans === 'function' ? window.perfSpans() : [],
    }));
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
  const url = `http://127.0.0.1:${server.address().port}/client/index.html?seed=${encodeURIComponent(args.seed)}`;
  try {
    const runs = [];
    for (let i = 0; i < args.replays; i++) runs.push(await replay(browser, url));
    console.log(buildReport(runs).text);
  } finally {
    await browser.close();
    server.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
