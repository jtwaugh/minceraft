// Run: node --test client/test/consolelogic.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { History, complete, nameLists } from '../consolelogic.js';

const names = {
  commands: ['help', 'give', 'spawn', 'clear'],
  tools: ['axe', 'hoe'],
  items: ['seeds', 'stick'],
  species: ['cow', 'chicken'],
};

test('Up/Down walk the history and return to the unfinished line', () => {
  const h = new History();
  h.add('one');
  h.add('two');
  assert.equal(h.up('draft'), 'two');
  assert.equal(h.up('two'), 'one');
  assert.equal(h.up('one'), 'one');
  assert.equal(h.down('one'), 'two');
  assert.equal(h.down('two'), 'draft');
  assert.equal(h.down('draft'), 'draft');
});

test('empty history leaves the line alone', () => {
  const h = new History();
  assert.equal(h.up('x'), 'x');
  assert.equal(h.down('x'), 'x');
});

test('blank and consecutive duplicate lines are not recorded', () => {
  const h = new History();
  h.add('a');
  h.add('a');
  h.add('   ');
  h.add('');
  assert.deepEqual(h.lines, ['a']);
  h.add('b');
  h.add('a');
  assert.deepEqual(h.lines, ['a', 'b', 'a']);
});

test('submitting resets the cursor to the end', () => {
  const h = new History();
  h.add('a');
  h.add('b');
  h.up('');
  h.up('');
  h.add('c');
  assert.equal(h.up(''), 'c');
});

test('history is capped', () => {
  const h = new History(null, { max: 2 });
  for (const l of ['a', 'b', 'c']) h.add(l);
  assert.deepEqual(h.lines, ['b', 'c']);
});

test('history persists through storage and reloads', () => {
  const store = new Map();
  const storage = { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, v) };
  new History(storage).add('give tool axe');
  assert.deepEqual(new History(storage).lines, ['give tool axe']);
});

test('storage that throws or holds junk is swallowed', () => {
  const angry = {
    getItem() { throw new Error('denied'); },
    setItem() { throw new Error('full'); },
  };
  const h = new History(angry);
  h.add('a');
  assert.equal(h.up(''), 'a');
  const junk = { getItem: () => '{not json', setItem() {} };
  assert.deepEqual(new History(junk).lines, []);
  const wrong = { getItem: () => '{"a":1}', setItem() {} };
  assert.deepEqual(new History(wrong).lines, []);
});

test('first word completes a command name', () => {
  assert.equal(complete('sp', names).line, 'spawn ');
  assert.equal(complete('', names).matches.length, 4);
  assert.equal(complete('zz', names).line, 'zz');
});

test('give completes kind, then tool or item names by kind', () => {
  assert.equal(complete('give t', names).line, 'give tool ');
  assert.equal(complete('give tool a', names).line, 'give tool axe ');
  assert.equal(complete('give item st', names).line, 'give item stick ');
  assert.equal(complete('give tool st', names).line, 'give tool st');
});

test('spawn completes item and species names', () => {
  assert.equal(complete('spawn co', names).line, 'spawn cow ');
  assert.equal(complete('spawn see', names).line, 'spawn seeds ');
});

test('ambiguous prefixes extend to the common prefix', () => {
  const r = complete('spawn c', names);
  assert.equal(r.line, 'spawn c');
  assert.deepEqual(r.matches, ['chicken', 'cow']);
  const s = complete('spawn s', names);
  assert.equal(s.line, 'spawn s');
  assert.equal(complete('spawn ch', names).line, 'spawn chicken ');
  assert.equal(complete('give item s', { ...names, items: ['seeds', 'seedling'] }).line, 'give item seed');
});

test('no completion past the last argument or for unknown commands', () => {
  assert.equal(complete('give tool axe ', names).line, 'give tool axe ');
  assert.equal(complete('nope a', names).line, 'nope a');
});

test('nameLists reads the wasm exports', () => {
  const wasm = {
    command_names: () => ['a'], tool_names: () => ['b'],
    item_names: () => ['c'], species_names: () => ['d'],
  };
  assert.deepEqual(nameLists(wasm), { commands: ['a'], tools: ['b'], items: ['c'], species: ['d'] });
});

test('the real wasm exports the registry and data names', async () => {
  const { readFileSync } = await import('node:fs');
  const wasm = await import('../pkg/game_web.js');
  wasm.initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });
  const lists = nameLists(wasm);
  for (const c of ['help', 'give', 'spawn', 'clear']) assert.ok(lists.commands.includes(c), c);
  assert.ok(lists.tools.length > 0 && lists.items.length > 0 && lists.species.includes('cow'));
  assert.equal(complete('sp', lists).line, 'spawn ');
});
