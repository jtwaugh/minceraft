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
//   readBelt(walk) -> { slots, selected, hands, held, swing, use }
//     slots    tool name per belt slot, or null for an empty slot
//     hands    the carried item's name, or null
//     held     what interaction rules see: empty | tool:<name> | item:<name>
//     swing, use  hand animation phases: 0 idle, 1 mid-animation, back to 0
//   formatBelt(belt) -> overlay lines (a minimum readout; the real HUD is Phase 8)
//   spawnTestItems(walk) puts a few loose items on the raft scene's islet
//   readAnimals(walk) -> [{ id, species, pos, facing, size, hurt, dying }]
//     pos     centre of the bottom face; facing in degrees (0 looks along -z);
//     size    [width, height] of the box
//   readItems(walk) -> [{ id, name, pos, size }]
//     pos     centre of the item's box; size the box's edge length
//   spawnTestAnimals(walk, kinds) puts animals on the raft scene's islet
//   faceAnimal(walk, look) -> [dx, dy] mouse movement that turns `look` ({ yaw, pitch })
//     to the first animal's middle

//   formatTime(fraction, frozen) -> "HH:MM (0.xxx)", plus " frozen" when the clock is held
//     fraction 0..1 with 0 midnight; the clock never prints 24:00

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
  return { slots, selected: walk.selected_slot(), hands: walk.hands() || null, held: walk.held(), swing: walk.swing_phase(), use: walk.use_phase() };
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

export function readAnimals(walk) {
  const rows = Array.from(walk.animals()), species = walk.animal_species();
  return species.map((name, i) => {
    const [id, x, y, z, facing, w, h, hurt, dying] = rows.slice(i * 9, i * 9 + 9);
    return { id, species: name, pos: [x, y, z], facing, size: [w, h], hurt: hurt === 1, dying: dying === 1 };
  });
}

export function readItems(walk) {
  const rows = Array.from(walk.items()), names = walk.item_names();
  return names.map((name, i) => {
    const [id, x, y, z, size] = rows.slice(i * 5, i * 5 + 5);
    return { id, name, pos: [x, y, z], size };
  });
}

// Animals stand on the islet's top row, clear of the seeds, a block apart.
export function spawnTestAnimals(walk, kinds = ['cow', 'pig', 'chicken']) {
  const [fx, , fz] = walk.feet();
  const x0 = Math.floor(fx) + 1, z = Math.floor(fz) - 0.5;
  kinds.forEach((kind, i) => walk.spawn_animal(kind, x0 + 0.5 + i, 64.05, z, 0));
}

// Same look convention as the sim: yaw 0 looks along -z and grows turning
// right, pitch grows looking up.
export function faceAnimal(walk, { yaw, pitch }, sensitivity = 0.0025) {
  const a = readAnimals(walk)[0];
  if (!a) throw new Error('no animal to face');
  const eye = walk.eye(1);
  const dx = a.pos[0] - eye[0], dy = a.pos[1] + a.size[1] / 2 - eye[1], dz = a.pos[2] - eye[2];
  const want = [Math.atan2(dx, -dz), Math.atan2(dy, Math.hypot(dx, dz))];
  let dyaw = want[0] - yaw;
  dyaw -= 2 * Math.PI * Math.round(dyaw / (2 * Math.PI));
  return [Math.round(dyaw / sensitivity), Math.round(-(want[1] - pitch) / sensitivity)];
}

export function formatTime(fraction, frozen) {
  const minutes = Math.min(1439, Math.floor(fraction * 1440));
  const hh = String(Math.floor(minutes / 60)).padStart(2, '0');
  const mm = String(minutes % 60).padStart(2, '0');
  return `time ${hh}:${mm} (${fraction.toFixed(3)})${frozen ? ' frozen' : ''}`;
}
