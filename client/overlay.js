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
//   heldFromQuery(tool) -> the `held` string for Walk.set_held

export const FACES = ['bottom', 'top', 'north', 'south', 'west', 'east'];
const KINDS = ['none', 'block', 'entity'];

/** `?tool=hoe|sword|empty` (or `item:<name>`) as a held string; anything else is empty-handed. */
export function heldFromQuery(tool) {
  if (!tool || tool === 'empty') return 'empty';
  return tool.includes(':') ? tool : `tool:${tool}`;
}

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
