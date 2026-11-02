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
 * centre, then move to pos), tint, tail: the same for the tail plate alone,
 * additionally yawed `swing` degrees about the hinge z (see tailHinge) }.
 */
export function fishPose(f, swing = 0, hinge = 0) {
  const a = f.heading * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
  const model = new Float32Array([c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, f.pos[0], f.pos[1], f.pos[2], 1]);
  const b = swing * Math.PI / 180, cb = Math.cos(b), sb = Math.sin(b);
  // model * T(0,0,hinge) * Ry(swing) * T(0,0,-hinge), column-major
  const [m0, m1, m2, , , , , , m8, m9, m10] = model;
  const tail = new Float32Array(model);
  tail[0] = m0 * cb - m8 * sb; tail[1] = m1 * cb - m9 * sb; tail[2] = m2 * cb - m10 * sb;
  tail[8] = m0 * sb + m8 * cb; tail[9] = m1 * sb + m9 * cb; tail[10] = m2 * sb + m10 * cb;
  tail[12] = model[12] + m8 * hinge - tail[8] * hinge;
  tail[13] = model[13] + m9 * hinge - tail[9] * hinge;
  tail[14] = model[14] + m10 * hinge - tail[10] * hinge;
  return { model, tail, tint: [1, 1, 1] };
}

export const TAIL_AMPLITUDE = 25; // largest tail swing either side of straight, degrees
const TAIL_FULL_SPEED = 2; // blocks per second at which the swing reaches full amplitude
const TAIL_CYCLES_PER_BLOCK = 1.5; // tail beats per block swum

/** The z of the edge where the tail plate meets the body, in the fishBoxes frame. */
export function tailHinge(species, bodyHeight) {
  const [body] = fishBoxes(species, bodyHeight);
  return body.lo[2] + body.size[2];
}

/**
 * Tracks fish between frames to swing their tails. `update(fish, t)` takes the
 * frame's fish ({ id, pos }) and the time in seconds, and returns a Map from id
 * to tail swing angle in degrees (0 when the fish is not moving). Fish missing
 * from the list are forgotten. Client-only; no RNG.
 */
export function createTailTracker() {
  const state = new Map(); // id -> { x, z, t, phase }
  return {
    update(fish, t) {
      const next = new Map();
      const angles = new Map();
      for (const f of fish) {
        const prev = state.get(f.id);
        let phase = 0, speed = 0;
        if (prev) {
          phase = prev.phase;
          const dt = t - prev.t;
          if (dt > 0) {
            speed = Math.hypot(f.pos[0] - prev.x, f.pos[2] - prev.z) / dt;
            phase += 2 * Math.PI * TAIL_CYCLES_PER_BLOCK * speed * dt;
          }
        }
        next.set(f.id, { x: f.pos[0], z: f.pos[2], t, phase });
        angles.set(f.id, TAIL_AMPLITUDE * Math.min(1, speed / TAIL_FULL_SPEED) * Math.sin(phase));
      }
      state.clear();
      for (const [k, v] of next) state.set(k, v);
      return angles;
    },
    size: () => state.size,
  };
}

/**
 * What to draw for the frame's `fish` ({ id, species, pos, heading, size }) given
 * the tail `swings` (Map id -> degrees, from createTailTracker): two draws per
 * fish, { species, height, part: 'body' | 'tail', model, tint }. The body part
 * is the body and dorsal fin (fishBoxes 0 and 1), the tail part box 2.
 */
export function fishDraw(fish, swings) {
  const out = [];
  for (const f of fish) {
    const height = f.size[1];
    const p = fishPose(f, swings.get(f.id) ?? 0, tailHinge(f.species, height));
    out.push({ species: f.species, height, part: 'body', model: p.model, tint: p.tint });
    out.push({ species: f.species, height, part: 'tail', model: p.tail, tint: p.tint });
  }
  return out;
}
