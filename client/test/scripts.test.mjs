// Runs every client/test/scripts/*.input file through the input DSL (dsl.mjs),
// one node:test per case. Needs ./build.sh first.
// Run: node --test client/test/scripts.test.mjs
import test from 'node:test';
import { readdirSync, readFileSync } from 'node:fs';
import { parse, run } from './dsl.mjs';

const dir = new URL('./scripts/', import.meta.url);
for (const file of readdirSync(dir).filter((f) => f.endsWith('.input')).sort()) {
  for (const c of parse(readFileSync(new URL(file, dir), 'utf8'))) {
    test(`${file}:${c.line} ${c.name}`, () => run(c));
  }
}
