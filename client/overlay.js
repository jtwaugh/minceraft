// What the player is aiming at and which interaction rule a press would run,
// as plain data (readAim) and as debug-overlay text (formatAim). No DOM or
// WebGL, so both run in Node. See docs/design/basic-gameplay.md sections 5
// and 7.
//
//   readAim(walk, alpha) -> { kind, pos, face, id, box, primary, secondary, last }
//     kind     'none' | 'block' | 'entity'
//     box      [minx, miny, minz, maxx, maxy, maxz] to outline, or null
//     primary, secondary, last   { index, action } of the matched rule, or null
//   formatAim(aim) -> overlay lines
//   readBelt(walk) -> { slots, selected, hands, held }
//     slots    tool name per belt slot, or null for an empty slot
//     hands    the carried item's name, or null
//     held     what interaction rules see: empty | tool:<name> | item:<name>
//   formatBelt(belt) -> overlay lines (a minimum readout; the real HUD is Phase 8)
//   spawnTestItems(walk) puts a few loose items on the raft scene's islet

export const FACES = ['bottom', 'top', 'north', 'south', 'west', 'east'];
const KINDS = ['none', 'block', 'entity'];

function rule(walk, index) {
  return index < 0 ? null : { index, action: walk.rule_action(index) };
}

export function readAim(walk, alpha) {
  const [kind, x, y, z, face] = Array.from(walk.target());
  const box = Array.from(walk.target_box(alpha));
  return {
    kind: KINDS[kind] ?? 'none',
    pos: kind === 1 ? [x, y, z] : null,
    face: kind === 1 ? FACES[face] : null,
    id: kind === 2 ? x : null,
    box: box.length ? box : null,
    primary: rule(walk, walk.preview_rule(false)),
    secondary: rule(walk, walk.preview_rule(true)),
    last: rule(walk, walk.last_rule()),
  };
}

const showRule = (r) => (r ? `#${r.index} ${r.action}` : 'none');

export function formatAim(aim) {
  let target = 'none';
  if (aim.kind === 'block') target = `block ${aim.pos.join(' ')} ${aim.face}`;
  else if (aim.kind === 'entity') target = `entity ${aim.id}`;
  return [
    `target ${target}`,
    `rule primary ${showRule(aim.primary)} | secondary ${showRule(aim.secondary)}`,
    `last press ${showRule(aim.last)}`,
  ].join('\n');
}

export function readBelt(walk) {
  const slots = [];
  for (let i = 0; i < walk.belt_len(); i++) slots.push(walk.belt_slot(i) || null);
  return { slots, selected: walk.selected_slot(), hands: walk.hands() || null, held: walk.held() };
}

export function formatBelt(belt) {
  const cells = belt.slots.map((t, i) => `${i === belt.selected ? '>' : ' '}${i + 1}:${t ?? '-'}`);
  return `belt ${cells.join(' ')}\nhands ${belt.hands ?? 'empty'} | held ${belt.held}`;
}

// The islet is the 3x3 patch one block east of the spawn (crates/game/src/scene.rs);
// its middle row is clear of the plants and the stub animal.
const TEST_ITEMS = ['seeds', 'seeds', 'seeds'];

export function spawnTestItems(walk) {
  const [fx, , fz] = walk.feet();
  const x0 = Math.floor(fx) + 1, z = Math.floor(fz);
  TEST_ITEMS.forEach((name, i) => walk.spawn_item(name, x0 + 0.5 + i, 64.2, z));
}
