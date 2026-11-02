// Fish box models: three boxes per fish (body, dorsal fin, tail plate) from the
// batch 11 faces of the tileset export. The art draws only left faces; right
// faces reuse them mirrored (`~` prefix, see boxVertices). Front is -z at
// heading 0, as for animals.

// Fish models in the tileset export (batch 11); the client requests exactly these.
export const FISH_MODELS = ['salmon', 'tuna', 'mackerel'];

// Grid sizes from the tileset export, in art pixels: body [width, height,
// length], and fin and tail [length, height].
const SIZES = {
  salmon: { body: [2, 4, 8], tail: [4, 4], dorsal: [3, 2] },
  tuna: { body: [3, 5, 10], tail: [5, 6], dorsal: [4, 3] },
  mackerel: { body: [2, 3, 7], tail: [3, 3], dorsal: [2, 2] },
};
const THIN = 0.04; // thickness of the flat fin and tail plates, in blocks
const GREY = {};

/**
 * The boxes of a `species` fish whose body is `bodyHeight` blocks tall, centred
 * on x and z at the origin with the body's bottom at y 0 and its nose at -z.
 * The tail plate sits behind the body, the dorsal fin on top of its middle.
 * Unknown species get one grey box.
 */
export function fishBoxes(species, bodyHeight) {
  const s = SIZES[species];
  if (!s) return [{ lo: [-0.5, 0, -0.5], size: [1, bodyHeight, 1], faces: GREY }];
  const k = bodyHeight / s.body[1];
  const [w, h, l] = s.body.map((v) => v * k);
  const f = (n) => `${species}.${n}`;
  const flat = (n) => ({ left: f(n), right: `~${f(n)}` });
  return [
    {
      lo: [-w / 2, 0, -l / 2], size: [w, h, l],
      faces: {
        front: f('body_front'), back: f('body_back'), left: f('body_left'),
        right: `~${f('body_left')}`, top: f('body_top'), bottom: f('body_bottom'),
      },
    },
    { lo: [-THIN / 2, h, -s.dorsal[0] * k / 2], size: [THIN, s.dorsal[1] * k, s.dorsal[0] * k], faces: flat('dorsal') },
    { lo: [-THIN / 2, (h - s.tail[1] * k) / 2, l / 2], size: [THIN, s.tail[1] * k, s.tail[0] * k], faces: flat('tail') },
  ];
}

/**
 * Pose of fish `f` ({ pos, heading (degrees) }) for drawing the boxes of
 * fishBoxes: { model: column-major Float32Array(16) (turn about the bottom
 * centre, then move to pos), tint }.
 */
export function fishPose(f) {
  const a = f.heading * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
  const model = new Float32Array([c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, f.pos[0], f.pos[1], f.pos[2], 1]);
  return { model, tint: [1, 1, 1] };
}
