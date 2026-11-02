import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, message_templates } from '../pkg/game_web.js';
import { createMessage, parseEvent } from '../message.js';
import { hudModel } from '../hud.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });
const templates = message_templates();
const { hold_ticks: hold, fade_ticks: fade } = templates;

const textOf = (line) => {
  const m = createMessage(templates);
  m.feed([line], 1);
  return m.view().text;
};

test('parseEvent splits the name and key=value fields', () => {
  assert.deepEqual(parseEvent('foraged position=1,2,3 yields=berries,seeds'), { name: 'foraged', position: '1,2,3', yields: 'berries,seeds' });
});

test('results read from the shipped templates', () => {
  assert.equal(textOf('item_picked_up item=seeds player=0'), 'Picked up seeds');
  assert.equal(textOf('block_tilled position=4,63,5 from=grass to=farmland'), 'Tilled soil');
  assert.equal(textOf('crop_planted position=4,64,5 crop=wheat'), 'Planted wheat');
  assert.equal(textOf('crop_harvested position=4,64,5 crop=wheat yields=wheat,seeds'), 'Harvested wheat');
  assert.equal(textOf('foraged position=4,64,5 yields=berries'), 'Foraged berries');
});

test('each refusal reason has its text, the drop key filled in', () => {
  const refused = (reason) => textOf(`action_refused action=use reason=${reason}`);
  assert.equal(refused('too_far'), 'Too far away');
  assert.equal(refused('hands_full'), `Hands full: drop with ${templates.drop_key}`);
  assert.equal(templates.drop_key, 'Q');
  assert.equal(refused('tools_stowed'), 'Tools stowed while carrying');
  assert.equal(refused('nothing_here'), 'Nothing to do here');
  assert.equal(refused('chest_full'), 'Chest full');
});

test('the message holds, fades over the configured ticks and clears', () => {
  const m = createMessage(templates);
  m.feed(['block_tilled position=0,63,0 from=grass to=farmland'], 1);
  assert.deepEqual(m.view(), { text: 'Tilled soil', alpha: 1 });
  m.feed([], hold);
  assert.deepEqual(m.view(), { text: 'Tilled soil', alpha: 1 });
  m.feed([], fade / 2);
  assert.equal(m.view().text, 'Tilled soil');
  assert.ok(Math.abs(m.view().alpha - 0.5) < 1e-9);
  m.feed([], fade / 2 - 1);
  assert.ok(m.view().alpha > 0 && m.view().alpha < 0.5);
  m.feed([], 1);
  assert.deepEqual(m.view(), { text: '', alpha: 0 });
  m.feed([], 5);
  assert.deepEqual(m.view(), { text: '', alpha: 0 });
});

test('a newer event replaces an older one and restarts the hold; the last of a frame wins', () => {
  const m = createMessage(templates);
  m.feed(['crop_planted position=0,64,0 crop=wheat'], 1);
  m.feed([], hold + 2);
  assert.ok(m.view().alpha < 1);
  m.feed(['item_picked_up item=seeds player=0', 'action_refused action=use reason=too_far'], 1);
  assert.deepEqual(m.view(), { text: 'Too far away', alpha: 1 });
});

test('a swing, an unrelated event and a forage_missed leave the message unchanged', () => {
  const m = createMessage(templates);
  for (const line of ['swing player=0 tool=hoe', 'entered_water player=0', 'forage_missed position=1,64,1']) {
    m.feed([line], 1);
    assert.deepEqual(m.view(), { text: '', alpha: 0 }, line);
  }
  m.feed(['item_picked_up item=seeds player=0'], 1);
  m.feed([], 3);
  m.feed(['swing player=0 tool=hoe', 'entered_water player=0', 'forage_missed position=1,64,1'], 0);
  assert.deepEqual(m.view(), { text: 'Picked up seeds', alpha: 1 });
  m.feed([], hold - 3 + fade);
  assert.equal(m.view().text, '', 'ignored events did not restart the hold');
});

test('hudModel passes the message through, empty by default', () => {
  const belt = { slots: [null, 'sword', 'hoe', null, null], selected: 1, hands: null, held: 'tool:sword' };
  assert.deepEqual(hudModel({ belt, health: 20, maxHealth: 20 }).message, { text: '', alpha: 0 });
  const message = { text: 'Tilled soil', alpha: 0.5 };
  assert.deepEqual(hudModel({ belt, health: 20, maxHealth: 20, message }).message, message);
});
